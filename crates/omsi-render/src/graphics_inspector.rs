//! Graphics diagnostics and inspection APIs.
//!
//! This module provides zero-stall GPU query interfaces for material properties,
//! texture metadata, and async texture readback. All operations use async staging
//! pools to avoid blocking Device::poll(Wait) calls.

use crate::{AlphaMode, MaterialId, TextureId};

/// PBR material parameters exposed for inspection.
#[derive(Debug, Clone, Copy)]
pub struct PBRParams {
    /// Roughness [0.0..1.0]
    pub roughness: f32,
    /// Metalness [0.0..1.0]
    pub metalness: f32,
    /// Normal map intensity [0.0..2.0]
    pub normal_intensity: f32,
    /// Environment reflection strength [0.0..3.0]
    pub env_reflection: f32,
}

impl Default for PBRParams {
    fn default() -> Self {
        Self {
            roughness: 0.5,
            metalness: 0.0,
            normal_intensity: 1.0,
            env_reflection: 1.0,
        }
    }
}

/// Material snapshot for inspector display.
///
/// Captures all material properties at a single frame boundary without
/// holding locks across frames.
#[derive(Debug, Clone)]
pub struct MaterialSnapshot {
    /// Shader variant name (enhanced.wgsl, shader.wgsl)
    pub shader_variant: String,
    /// Diffuse/albedo texture
    pub diffuse_tex: Option<TextureHandle>,
    /// Normal/bump map texture
    pub normal_tex: Option<TextureHandle>,
    /// Roughness map texture
    pub roughness_tex: Option<TextureHandle>,
    /// Metallic map texture
    pub metallic_tex: Option<TextureHandle>,
    /// Environment map texture
    pub envmap_tex: Option<TextureHandle>,
    /// PBR parameters
    pub pbr_params: PBRParams,
    /// Total VRAM usage in bytes
    pub vram_bytes: usize,
    /// Base color tint [R, G, B, A]
    pub color: [f32; 4],
    /// Emissive color [R, G, B]
    pub emissive: [f32; 3],
    /// Alpha mode (Opaque, Test, Blend)
    pub alpha_mode: AlphaMode,
    /// UV scale/bias [scale_u, scale_v, bias_u, bias_v]
    pub uv_transform: [f32; 4],
    /// Blend mode classification
    pub blend_mode: BlendMode,
    /// Double-sided rendering flag
    pub double_sided: bool,
    /// Alpha test cutoff value [0.0..1.0]
    pub alpha_cutoff: f32,
    /// Render pass classification
    pub render_pass: RenderPassType,
}

/// Blend mode classification for material rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Standard alpha blending (src_alpha, one_minus_src_alpha)
    Alpha,
    /// Additive blending (one, one)
    Additive,
    /// Multiplicative blending (dst_color, zero)
    Multiply,
    /// Opaque (no blending)
    Opaque,
}

/// Render pass type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPassType {
    /// Opaque forward pass
    Opaque,
    /// Transparent blended pass
    Transparent,
    /// Alpha-tested cutout pass
    Cutout,
    /// Shadow map pass
    Shadow,
}

/// Stable texture handle for inspector queries.
///
/// Combines texture ID with generation counter to detect texture replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    pub id: TextureId,
    pub generation: u64,
}

/// Texture metadata query result.
#[derive(Debug, Clone)]
pub struct TextureMetadata {
    pub handle: TextureHandle,
    /// Texture format (e.g., "Bc7RgbaUnorm", "Rgba8Unorm")
    pub format: String,
    /// Resolution (width, height)
    pub resolution: (u32, u32),
    /// Mipmap level count
    pub mipmap_count: u32,
    /// VRAM bytes for all mip levels
    pub vram_bytes: u64,
    /// Whether this is a dynamic texture (scripttexture/texttexture)
    pub is_dynamic: bool,
}

/// Texture query request with optional mip level isolation.
#[derive(Debug, Clone, Copy)]
pub struct TextureQuery {
    pub handle: TextureHandle,
    /// Request specific mip level (None = level 0)
    pub request_mip_level: Option<u8>,
}

/// Texture query result with metadata and optional preview thumbnail.
#[derive(Debug, Clone)]
pub struct TextureQueryResult {
    pub metadata: TextureMetadata,
    /// Thumbnail view for UI rendering (bound asynchronously, never blocks)
    pub thumbnail_ready: bool,
}

/// Async texture readback request ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadbackId(pub u64);

/// Status of an async readback operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadbackStatus {
    /// Request queued, not yet started
    Pending,
    /// GPU copy in progress
    InFlight,
    /// Ready to map
    Ready,
    /// Completed and data available
    Complete,
    /// Failed or cancelled
    Failed,
}

/// Completed texture readback with pixel data.
#[derive(Debug, Clone)]
pub struct TextureReadback {
    pub handle: TextureHandle,
    pub mip_level: u8,
    pub width: u32,
    pub height: u32,
    /// Raw RGBA8 pixel data (width * height * 4 bytes)
    pub pixels: Vec<u8>,
}

/// Inspector query interface for render state.
///
/// All queries are non-blocking and use async staging pools.
pub trait InspectorQuery {
    /// Query material properties by ID.
    ///
    /// Returns None if material ID is invalid or material was freed.
    fn query_material(&self, material_id: MaterialId) -> Option<MaterialSnapshot>;

    /// Query texture metadata without blocking.
    ///
    /// Thumbnail binding happens asynchronously; `thumbnail_ready` indicates
    /// when the view is available for UI rendering.
    fn query_texture_metadata(&self, query: TextureQuery) -> Option<TextureQueryResult>;

    /// Request async texture readback to CPU.
    ///
    /// Returns immediately with a ReadbackId. Poll with `poll_readback` to
    /// retrieve pixels when ready. Readback is capped at 128MB staging pool.
    fn request_texture_readback(
        &mut self,
        handle: TextureHandle,
        mip_level: u8,
    ) -> Option<ReadbackId>;

    /// Poll readback status and retrieve completed data.
    ///
    /// Returns None if ID is invalid or readback is still in progress.
    /// Returns Some(None) if readback failed.
    /// Returns Some(Some(data)) when complete.
    fn poll_readback(&mut self, id: ReadbackId) -> Option<Option<TextureReadback>>;

    /// Cancel a pending readback and free staging resources.
    fn cancel_readback(&mut self, id: ReadbackId);

    /// Get list of all active material IDs.
    fn active_materials(&self) -> Vec<MaterialId>;

    /// Get list of all loaded texture handles.
    fn loaded_textures(&self) -> Vec<TextureHandle>;
}

/// Texture format classification for display.
pub fn format_name(format: wgpu::TextureFormat) -> &'static str {
    use wgpu::TextureFormat::*;
    match format {
        Rgba8Unorm => "RGBA8Unorm",
        Rgba8UnormSrgb => "RGBA8Srgb",
        Bgra8Unorm => "BGRA8Unorm",
        Bgra8UnormSrgb => "BGRA8Srgb",
        Bc1RgbaUnorm => "BC1 (DXT1)",
        Bc1RgbaUnormSrgb => "BC1 Srgb",
        Bc3RgbaUnorm => "BC3 (DXT5)",
        Bc3RgbaUnormSrgb => "BC3 Srgb",
        Bc7RgbaUnorm => "BC7",
        Bc7RgbaUnormSrgb => "BC7 Srgb",
        R8Unorm => "R8",
        Rg8Unorm => "RG8",
        Rgba16Float => "RGBA16F",
        Rgba32Float => "RGBA32F",
        Depth32Float => "Depth32F",
        _ => "Other",
    }
}

/// Calculate mipmap count for a texture size.
pub fn mip_count(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height);
    if max_dim == 0 {
        return 1;
    }
    (max_dim as f32).log2().floor() as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mip_count() {
        assert_eq!(mip_count(1024, 1024), 11); // 1024 -> 512 -> ... -> 1
        assert_eq!(mip_count(512, 256), 10);   // 512 -> 256 -> ... -> 1
        assert_eq!(mip_count(1, 1), 1);
        assert_eq!(mip_count(0, 0), 1);
    }

    #[test]
    fn test_format_names() {
        assert_eq!(format_name(wgpu::TextureFormat::Bc7RgbaUnorm), "BC7");
        assert_eq!(format_name(wgpu::TextureFormat::Rgba8Unorm), "RGBA8Unorm");
    }
}
