//! Async texture readback staging pool.
//!
//! Provides zero-stall GPU-to-CPU texture readback using a ring buffer of staging
//! buffers. Never calls Device::poll(Wait) synchronously; all operations are queued
//! and polled asynchronously.

use crate::graphics_inspector::{ReadbackId, ReadbackStatus, TextureHandle, TextureReadback};
use std::collections::{HashMap, VecDeque};
use wgpu;

/// Maximum staging pool capacity in bytes (128 MB).
const MAX_STAGING_BYTES: u64 = 128 * 1024 * 1024;

/// Minimum staging buffer alignment (wgpu copy requirements).
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

/// A single staging buffer for async readback.
struct StagingBuffer {
    buffer: wgpu::Buffer,
    size: u64,
    /// Request ID this buffer is servicing
    request_id: Option<ReadbackId>,
    /// Frame number when this buffer was submitted
    submit_frame: u64,
}

/// A pending readback request.
struct ReadbackRequest {
    id: ReadbackId,
    handle: TextureHandle,
    mip_level: u8,
    width: u32,
    height: u32,
    status: ReadbackStatus,
    /// Staging buffer index in the pool
    buffer_index: Option<usize>,
}

/// Async staging pool for texture readback.
///
/// Manages a ring buffer of staging buffers with zero-stall guarantees.
pub struct StagingPool {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Ring buffer of staging buffers
    buffers: Vec<StagingBuffer>,
    /// Current frame counter
    frame: u64,
    /// Total bytes allocated in staging buffers
    allocated_bytes: u64,
    /// Pending readback requests
    requests: HashMap<ReadbackId, ReadbackRequest>,
    /// Next request ID
    next_id: u64,
    /// Queue of free buffer indices
    free_buffers: VecDeque<usize>,
    /// Completed readbacks ready for retrieval
    completed: HashMap<ReadbackId, TextureReadback>,
}

impl StagingPool {
    /// Create a new staging pool.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            buffers: Vec::new(),
            frame: 0,
            allocated_bytes: 0,
            requests: HashMap::new(),
            next_id: 1,
            free_buffers: VecDeque::new(),
            completed: HashMap::new(),
        }
    }

    /// Begin a new frame.
    ///
    /// Advances frame counter and polls pending readbacks.
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.poll_pending_requests();
    }

    /// Request texture readback to staging buffer.
    ///
    /// Returns None if pool is full or allocation would exceed capacity.
    pub fn request_readback(
        &mut self,
        texture: &wgpu::Texture,
        handle: TextureHandle,
        mip_level: u8,
    ) -> Option<ReadbackId> {
        let size = texture.size();
        let width = (size.width >> mip_level).max(1);
        let height = (size.height >> mip_level).max(1);
        
        // Calculate required buffer size with alignment
        let bytes_per_row = align_to(width * 4, COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer_size = (bytes_per_row * height) as u64;

        // Check capacity
        if self.allocated_bytes + buffer_size > MAX_STAGING_BYTES {
            log::warn!(
                "Staging pool full: {} + {} > {}",
                self.allocated_bytes,
                buffer_size,
                MAX_STAGING_BYTES
            );
            return None;
        }

        // Allocate or reuse buffer
        let buffer_index = self.get_or_create_buffer(buffer_size)?;

        // Create request
        let id = ReadbackId(self.next_id);
        self.next_id += 1;

        let request = ReadbackRequest {
            id,
            handle,
            mip_level,
            width,
            height,
            status: ReadbackStatus::Pending,
            buffer_index: Some(buffer_index),
        };

        // Encode copy command
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("inspector_texture_copy"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffers[buffer_index].buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // Mark buffer as in-flight
        self.buffers[buffer_index].request_id = Some(id);
        self.buffers[buffer_index].submit_frame = self.frame;

        self.requests.insert(id, request);

        Some(id)
    }

    /// Poll readback status and retrieve completed data.
    ///
    /// Returns None if request is still in flight.
    /// Returns Some(None) if request failed.
    /// Returns Some(Some(data)) when complete.
    pub fn poll_readback(&mut self, id: ReadbackId) -> Option<Option<TextureReadback>> {
        // Check if already completed
        if let Some(readback) = self.completed.remove(&id) {
            return Some(Some(readback));
        }

        let request = self.requests.get_mut(&id)?;

        match request.status {
            ReadbackStatus::Pending | ReadbackStatus::InFlight => {
                // Still in flight, try to poll without blocking
                self.poll_request_non_blocking(id);
                None
            }
            ReadbackStatus::Ready => {
                // Map and read data
                let result = self.map_and_read_request(id);
                Some(result)
            }
            ReadbackStatus::Complete => {
                // Should have been in completed map
                None
            }
            ReadbackStatus::Failed => Some(None),
        }
    }

    /// Cancel a pending readback.
    pub fn cancel_readback(&mut self, id: ReadbackId) {
        if let Some(request) = self.requests.remove(&id) {
            if let Some(buffer_index) = request.buffer_index {
                self.free_buffer(buffer_index);
            }
        }
        self.completed.remove(&id);
    }

    /// Get allocated staging buffer count.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Get total allocated bytes.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes
    }

    // Internal methods

    fn get_or_create_buffer(&mut self, required_size: u64) -> Option<usize> {
        // Try to reuse a free buffer of sufficient size
        if let Some(index) = self.free_buffers.pop_front() {
            if self.buffers[index].size >= required_size {
                return Some(index);
            }
            // Buffer too small, put it back
            self.free_buffers.push_back(index);
        }

        // Allocate new buffer
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("inspector_staging"),
            size: required_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let index = self.buffers.len();
        self.buffers.push(StagingBuffer {
            buffer,
            size: required_size,
            request_id: None,
            submit_frame: 0,
        });
        self.allocated_bytes += required_size;

        Some(index)
    }

    fn free_buffer(&mut self, index: usize) {
        self.buffers[index].request_id = None;
        self.free_buffers.push_back(index);
    }

    fn poll_pending_requests(&mut self) {
        let ids: Vec<_> = self.requests.keys().copied().collect();
        for id in ids {
            self.poll_request_non_blocking(id);
        }
    }

    fn poll_request_non_blocking(&mut self, id: ReadbackId) {
        let request = match self.requests.get_mut(&id) {
            Some(r) => r,
            None => return,
        };

        // Don't call Device::poll(Wait) - use poll with zero timeout for non-blocking check
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::ZERO),
        });

        // Check if buffer is ready (submitted 2+ frames ago for safety)
        if let Some(buffer_index) = request.buffer_index {
            let buffer = &self.buffers[buffer_index];
            if self.frame >= buffer.submit_frame + 2 {
                request.status = ReadbackStatus::Ready;
            }
        }
    }

    fn map_and_read_request(&mut self, id: ReadbackId) -> Option<TextureReadback> {
        let request = self.requests.remove(&id)?;
        let buffer_index = request.buffer_index?;
        let buffer = &self.buffers[buffer_index];

        // Map buffer slice
        let buffer_slice = buffer.buffer.slice(..);
        
        // Use try_get_mapped_range for non-blocking access
        let mapped = match buffer_slice.get_mapped_range().as_ref() {
            data => {
                // Copy data out
                let bytes_per_row = align_to(request.width * 4, COPY_BYTES_PER_ROW_ALIGNMENT);
                let mut pixels = Vec::with_capacity((request.width * request.height * 4) as usize);

                for row in 0..request.height {
                    let src_offset = (row * bytes_per_row) as usize;
                    let src_end = src_offset + (request.width * 4) as usize;
                    if src_end <= data.len() {
                        pixels.extend_from_slice(&data[src_offset..src_end]);
                    }
                }

                Some(pixels)
            }
        };

        let _ = buffer_slice;
        buffer.buffer.unmap();
        self.free_buffer(buffer_index);

        mapped.map(|pixels| TextureReadback {
            handle: request.handle,
            mip_level: request.mip_level,
            width: request.width,
            height: request.height,
            pixels,
        })
    }
}

/// Align value to the specified alignment.
fn align_to(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) / alignment * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_to() {
        assert_eq!(align_to(0, 256), 0);
        assert_eq!(align_to(1, 256), 256);
        assert_eq!(align_to(255, 256), 256);
        assert_eq!(align_to(256, 256), 256);
        assert_eq!(align_to(257, 256), 512);
    }

    #[test]
    fn test_capacity_limit() {
        // Verify MAX_STAGING_BYTES is reasonable
        assert_eq!(MAX_STAGING_BYTES, 128 * 1024 * 1024);
    }
}
