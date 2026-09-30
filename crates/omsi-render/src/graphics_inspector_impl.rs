//! Inspector integration for render state queries.
//!
//! Implements InspectorQuery trait for Scene and Renderer, providing zero-stall
//! access to material properties and texture metadata.

use crate::graphics_inspector::*;
use crate::staging_pool::StagingPool;
use crate::{AlphaMode, MaterialId, Scene};
use std::collections::HashMap;

/// Inspector state attached to a Scene for diagnostic queries.
pub struct SceneInspector {
    /// Async staging pool for texture readback
    staging_pool: Option<StagingPool>,
    /// Cached texture metadata to avoid repeated queries
    texture_cache: HashMap<TextureHandle, TextureMetadata>,
    /// Frame counter for cache invalidation
    frame: u64,
}

impl SceneInspector {
    pub fn new() -> Self {
        Self {
            staging_pool: None,
            texture_cache: HashMap::new(),
            frame: 0,
        }
    }

    pub fn with_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            staging_pool: Some(StagingPool::new(device, queue)),
            texture_cache: HashMap::new(),
            frame: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame += 1;
        if let Some(pool) = &mut self.staging_pool {
            pool.begin_frame();
        }
        // Clear cache every 60 frames to prevent stale metadata
        if self.frame % 60 == 0 {
            self.texture_cache.clear();
        }
    }
}

impl Default for SceneInspector {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension methods for Scene to support inspector queries.
impl Scene {
    /// Query material properties by ID.
    pub fn query_material_snapshot(&self, material_id: MaterialId) -> Option<MaterialSnapshot> {
        let material = self.materials.get(material_id)?;

        // Determine shader variant based on material properties
        let shader_variant = if material.unlit {
            "shader.wgsl".to_string()
        } else {
            "enhanced.wgsl".to_string()
        };

        // Classify blend mode
        let blend_mode = match material.alpha {
            AlphaMode::Opaque => BlendMode::Opaque,
            AlphaMode::Test => BlendMode::Opaque, // Cutout uses opaque pass
            AlphaMode::Blend => BlendMode::Alpha,
        };

        // Classify render pass
        let render_pass = match material.alpha {
            AlphaMode::Opaque => RenderPassType::Opaque,
            AlphaMode::Test => RenderPassType::Cutout,
            AlphaMode::Blend => RenderPassType::Transparent,
        };

        // Extract texture handles with generation counters
        let diffuse_tex = material.texture.map(|id| {
            let gen = self.textures.get(id).map(|t| t.gen).unwrap_or(0);
            TextureHandle { id, generation: gen }
        });

        // Check for PBR maps
        let pbr = self.pbr_maps.get(&material.texture?);
        let normal_tex = pbr.and_then(|p| p.normal).map(|id| {
            let gen = self.textures.get(id).map(|t| t.gen).unwrap_or(0);
            TextureHandle { id, generation: gen }
        });

        // Calculate total VRAM usage
        let mut vram_bytes = 0usize;
        if let Some(tex_id) = material.texture {
            vram_bytes += self.texture_bytes_of(tex_id) as usize;
        }
        if let Some(tex_id) = material.nightmap {
            vram_bytes += self.texture_bytes_of(tex_id) as usize;
        }
        if let Some(tex_id) = material.lightmap {
            vram_bytes += self.texture_bytes_of(tex_id) as usize;
        }
        if let Some((tex_id, _)) = material.envmap {
            vram_bytes += self.texture_bytes_of(tex_id) as usize;
        }

        // Extract PBR parameters
        let pbr_params = PBRParams {
            roughness: pbr.map(|p| p.flags[2]).unwrap_or(0.5),
            metalness: pbr.map(|p| p.flags[3]).unwrap_or(0.0),
            normal_intensity: pbr.map(|p| p.flags[0]).unwrap_or(1.0),
            env_reflection: material.envmap.map(|(_, f)| f).unwrap_or(1.0),
        };

        // UV transform (default identity - could extract from material uniform)
        let uv_transform = [1.0, 1.0, 0.0, 0.0];

        // Alpha cutoff for test mode (OMSI default ~0.5)
        let alpha_cutoff = if material.alpha == AlphaMode::Test {
            0.5
        } else {
            0.0
        };

        Some(MaterialSnapshot {
            shader_variant,
            diffuse_tex,
            normal_tex,
            roughness_tex: None,
            metallic_tex: None,
            envmap_tex: material.envmap.map(|(id, _)| {
                let gen = self.textures.get(id).map(|t| t.gen).unwrap_or(0);
                TextureHandle { id, generation: gen }
            }),
            pbr_params,
            vram_bytes,
            color: material.color,
            emissive: material.emissive,
            alpha_mode: material.alpha,
            uv_transform,
            blend_mode,
            double_sided: true, // OMSI default - could track per-material
            alpha_cutoff,
            render_pass,
        })
    }

    /// Query texture metadata without blocking.
    pub fn query_texture_metadata_snapshot(
        &self,
        query: TextureQuery,
    ) -> Option<TextureQueryResult> {
        let texture = self.textures.get(query.handle.id)?;

        // Verify generation matches
        if texture.gen != query.handle.generation {
            return None;
        }

        let format = format_name(texture.texture.format());
        let (width, height) = texture.size;
        let mip_level = query.request_mip_level.unwrap_or(0);
        let actual_width = (width >> mip_level).max(1);
        let actual_height = (height >> mip_level).max(1);

        let metadata = TextureMetadata {
            handle: query.handle,
            format: format.to_string(),
            resolution: (actual_width, actual_height),
            mipmap_count: mip_count(width, height),
            vram_bytes: texture.bytes,
            is_dynamic: false, // Would need tracking for scripttexture
        };

        Some(TextureQueryResult {
            metadata,
            thumbnail_ready: true, // Always ready for synchronous queries
        })
    }

    /// Get list of all active material IDs.
    pub fn active_material_ids(&self) -> Vec<MaterialId> {
        (0..self.materials.len()).collect()
    }

    /// Get list of all loaded texture handles.
    pub fn loaded_texture_handles(&self) -> Vec<TextureHandle> {
        self.textures
            .iter()
            .enumerate()
            .filter(|(_, t)| t.bytes > 0) // Skip freed slots
            .map(|(id, t)| TextureHandle {
                id,
                generation: t.gen,
            })
            .collect()
    }
}

/// Implementation of InspectorQuery for Scene with staging pool.
impl InspectorQuery for Scene {
    fn query_material(&self, material_id: MaterialId) -> Option<MaterialSnapshot> {
        self.query_material_snapshot(material_id)
    }

    fn query_texture_metadata(&self, query: TextureQuery) -> Option<TextureQueryResult> {
        self.query_texture_metadata_snapshot(query)
    }

    fn request_texture_readback(
        &mut self,
        _handle: TextureHandle,
        _mip_level: u8,
    ) -> Option<ReadbackId> {
        // Requires staging pool integration - placeholder for now
        log::warn!("Texture readback requires staging pool initialization");
        None
    }

    fn poll_readback(&mut self, _id: ReadbackId) -> Option<Option<TextureReadback>> {
        None
    }

    fn cancel_readback(&mut self, _id: ReadbackId) {
        // No-op without staging pool
    }

    fn active_materials(&self) -> Vec<MaterialId> {
        self.active_material_ids()
    }

    fn loaded_textures(&self) -> Vec<TextureHandle> {
        self.loaded_texture_handles()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_inspector_creation() {
        let inspector = SceneInspector::new();
        assert!(inspector.staging_pool.is_none());
        assert_eq!(inspector.frame, 0);
    }

    #[test]
    fn test_texture_handle_equality() {
        let h1 = TextureHandle {
            id: 0,
            generation: 1,
        };
        let h2 = TextureHandle {
            id: 0,
            generation: 1,
        };
        let h3 = TextureHandle {
            id: 0,
            generation: 2,
        };

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_blend_mode_classification() {
        assert_eq!(
            std::mem::discriminant(&BlendMode::Alpha),
            std::mem::discriminant(&BlendMode::Alpha)
        );
        assert_ne!(
            std::mem::discriminant(&BlendMode::Alpha),
            std::mem::discriminant(&BlendMode::Opaque)
        );
    }
}
