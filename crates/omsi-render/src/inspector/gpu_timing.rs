//! GPU timestamp query pool for measuring render pass execution times.
//!
//! Uses wgpu timestamp queries to measure GPU execution time for individual render passes.
//! Results are read asynchronously (2-3 frames latency) to avoid GPU pipeline stalls.

use std::collections::VecDeque;
use wgpu;

/// Maximum number of timestamp queries per frame (start + end for each pass).
const MAX_QUERIES_PER_FRAME: usize = 64;

/// Number of frames to buffer for asynchronous readback (2-3 frames latency).
const FRAME_BUFFER_SIZE: usize = 4;

/// A single timestamp query result.
#[derive(Debug, Clone, Copy)]
pub struct TimestampResult {
    /// Pass name identifier.
    pub pass_id: u32,
    /// GPU time in nanoseconds.
    pub time_ns: u64,
}

/// Manages GPU timestamp queries for render pass timing.
///
/// Queries are submitted per-pass and read back asynchronously after 2-3 frames
/// to avoid stalling the GPU pipeline.
pub struct TimestampQueryPool {
    /// wgpu query set for timestamps.
    query_set: wgpu::QuerySet,
    /// Staging buffer for readback.
    resolve_buffer: wgpu::Buffer,
    /// CPU-readable buffer for results.
    read_buffer: wgpu::Buffer,
    /// Queue of pending frames waiting for results.
    pending_frames: VecDeque<PendingFrame>,
    /// Next available query index in the current frame.
    next_query_index: u32,
    /// Current frame being recorded.
    current_frame: Option<FrameQueries>,
    /// Whether timestamp queries are supported.
    enabled: bool,
}

/// A frame's worth of timestamp queries.
struct FrameQueries {
    /// Frame ID.
    frame_id: u64,
    /// Query ranges: (start_index, end_index, pass_name).
    ranges: Vec<(u32, u32, String)>,
}

/// A frame submitted for async readback.
struct PendingFrame {
    frame_id: u64,
    query_count: u32,
    ranges: Vec<(u32, u32, String)>,
}

impl TimestampQueryPool {
    /// Create a new timestamp query pool.
    ///
    /// Returns `None` if timestamp queries are not supported by the device.
    pub fn new(device: &wgpu::Device) -> Option<Self> {
        // Check if timestamp queries are supported
        let features = device.features();
        if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            log::warn!("Timestamp queries not supported on this device");
            return None;
        }

        let query_count = MAX_QUERIES_PER_FRAME as u32;

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Timestamp Query Set"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });

        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Timestamp Resolve Buffer"),
            size: (query_count * 8) as u64, // 8 bytes per timestamp
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let read_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Timestamp Read Buffer"),
            size: (query_count * 8) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Some(Self {
            query_set,
            resolve_buffer,
            read_buffer,
            pending_frames: VecDeque::with_capacity(FRAME_BUFFER_SIZE),
            next_query_index: 0,
            current_frame: None,
            enabled: true,
        })
    }

    /// Begin a new frame of timestamp queries.
    pub fn begin_frame(&mut self, frame_id: u64) {
        if !self.enabled {
            return;
        }

        self.next_query_index = 0;
        self.current_frame = Some(FrameQueries {
            frame_id,
            ranges: Vec::new(),
        });
    }

    /// Begin timing a render pass.
    ///
    /// Returns the query index to use with `write_timestamp`, or `None` if queries are full.
    pub fn begin_pass(&mut self, pass_name: impl Into<String>) -> Option<u32> {
        if !self.enabled {
            return None;
        }

        if self.next_query_index + 2 > MAX_QUERIES_PER_FRAME as u32 {
            log::warn!("Timestamp query pool exhausted for this frame");
            return None;
        }

        let start_index = self.next_query_index;
        self.next_query_index += 2; // Reserve start + end

        if let Some(ref mut frame) = self.current_frame {
            frame.ranges.push((start_index, start_index + 1, pass_name.into()));
        }

        Some(start_index)
    }

    /// Write a timestamp query to a render pass.
    pub fn write_timestamp(
        &self,
        pass: &mut wgpu::RenderPass,
        query_index: u32,
    ) {
        if !self.enabled {
            return;
        }

        pass.write_timestamp(&self.query_set, query_index);
    }

    /// Write a timestamp query to a compute pass.
    pub fn write_timestamp_compute(
        &self,
        pass: &mut wgpu::ComputePass,
        query_index: u32,
    ) {
        if !self.enabled {
            return;
        }

        pass.write_timestamp(&self.query_set, query_index);
    }

    /// End the current frame and submit queries for async readback.
    pub fn end_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enabled {
            return;
        }

        let frame = match self.current_frame.take() {
            Some(f) => f,
            None => return,
        };

        if frame.ranges.is_empty() {
            return;
        }

        let query_count = self.next_query_index;

        // Resolve queries to staging buffer
        encoder.resolve_query_set(
            &self.query_set,
            0..query_count,
            &self.resolve_buffer,
            0,
        );

        // Copy to CPU-readable buffer
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.read_buffer,
            0,
            (query_count * 8) as u64,
        );

        // Queue for async readback
        self.pending_frames.push_back(PendingFrame {
            frame_id: frame.frame_id,
            query_count,
            ranges: frame.ranges,
        });

        // Limit buffer depth
        if self.pending_frames.len() > FRAME_BUFFER_SIZE {
            self.pending_frames.pop_front();
        }
    }

    /// Poll for completed timestamp results.
    ///
    /// Returns timing results for the oldest completed frame, or `None` if no results ready.
    pub fn poll_results(&mut self, device: &wgpu::Device) -> Option<Vec<(String, f32)>> {
        if !self.enabled || self.pending_frames.is_empty() {
            return None;
        }

        // Try to read the oldest pending frame
        let pending = self.pending_frames.front()?;
        let buffer_slice = self.read_buffer.slice(..((pending.query_count * 8) as u64));

        // Check if buffer is ready
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });

        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        // Non-blocking check
        if receiver.try_recv().ok()?.is_err() {
            return None;
        }

        // Read timestamp data
        let data = buffer_slice.get_mapped_range();
        let timestamps: &[u64] = bytemuck::cast_slice(&data);

        let mut results = Vec::new();
        for (start_idx, end_idx, pass_name) in &pending.ranges {
            let start_ns = timestamps[*start_idx as usize];
            let end_ns = timestamps[*end_idx as usize];
            let duration_ns = end_ns.saturating_sub(start_ns);
            let duration_ms = duration_ns as f32 / 1_000_000.0;
            results.push((pass_name.clone(), duration_ms));
        }

        drop(data);
        self.read_buffer.unmap();

        self.pending_frames.pop_front();

        Some(results)
    }

    /// Check if timestamp queries are enabled and supported.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_queries_structure() {
        let frame = FrameQueries {
            frame_id: 123,
            ranges: vec![
                (0, 1, "Pass1".to_string()),
                (2, 3, "Pass2".to_string()),
            ],
        };

        assert_eq!(frame.frame_id, 123);
        assert_eq!(frame.ranges.len(), 2);
    }

    #[test]
    fn test_pending_frame_structure() {
        let pending = PendingFrame {
            frame_id: 456,
            query_count: 4,
            ranges: vec![(0, 1, "Test".to_string())],
        };

        assert_eq!(pending.frame_id, 456);
        assert_eq!(pending.query_count, 4);
    }
}
