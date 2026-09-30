//! Material panel UI for the visual debug inspector.
//!
//! Provides UI components for displaying material properties, texture previews,
//! and PBR parameter inspection with zero-stall GPU queries.

use crate::inspector::{InspectorHit, SelectionTarget};
use omsi_render::graphics_inspector::*;
use omsi_render::{MaterialId, Scene};

/// Material panel state for the inspector UI.
pub struct MaterialPanel {
    /// Currently selected material ID
    selected_material: Option<MaterialId>,
    /// Cached material snapshot
    material_snapshot: Option<MaterialSnapshot>,
    /// Active texture query handles
    texture_queries: Vec<(TextureHandle, TextureQuery)>,
    /// Pending readback requests
    pending_readbacks: Vec<ReadbackId>,
    /// Mipmap slider state (0 = base level)
    mipmap_level: u8,
    /// PBR parameter sandbox overrides
    pbr_overrides: Option<PBRParams>,
    /// Whether sandbox mode is active
    sandbox_active: bool,
}

impl MaterialPanel {
    pub fn new() -> Self {
        Self {
            selected_material: None,
            material_snapshot: None,
            texture_queries: Vec::new(),
            pending_readbacks: Vec::new(),
            mipmap_level: 0,
            pbr_overrides: None,
            sandbox_active: false,
        }
    }

    /// Update selection based on inspector hit.
    pub fn update_selection(&mut self, hit: Option<&InspectorHit>, scene: &Scene) {
        let material_id = match hit {
            Some(InspectorHit {
                target: SelectionTarget::Vehicle { .. },
                ..
            })
            | Some(InspectorHit {
                target: SelectionTarget::Scenery { .. },
                ..
            }) => {
                // Extract material from instance - placeholder logic
                // Real implementation would query instance materials
                Some(0)
            }
            _ => None,
        };

        if self.selected_material != material_id {
            self.selected_material = material_id;
            self.material_snapshot = material_id.and_then(|id| scene.query_material_snapshot(id));
            self.texture_queries.clear();
            self.mipmap_level = 0;
            self.sandbox_active = false;
            self.pbr_overrides = None;
        }
    }

    /// Poll pending readbacks and save completed textures.
    pub fn poll_readbacks(&mut self, scene: &mut Scene) {
        self.pending_readbacks.retain(|&id| {
            match scene.poll_readback(id) {
                Some(Some(readback)) => {
                    self.save_texture_png(&readback);
                    false // Remove completed
                }
                Some(None) => {
                    log::warn!("Texture readback failed: {:?}", id);
                    false // Remove failed
                }
                None => true, // Keep pending
            }
        });
    }

    fn save_texture_png(&self, readback: &TextureReadback) {
        let filename = format!(
            "Screenshots/inspector_dump_{}_{}.png",
            readback.handle.id, readback.mip_level
        );
        log::info!(
            "Saving texture {}x{} to {}",
            readback.width,
            readback.height,
            filename
        );
        // Placeholder - would use image crate to save PNG
    }
}

impl Default for MaterialPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(2621440), "2.50 MB");
    }

    #[test]
    fn test_material_panel_creation() {
        let panel = MaterialPanel::new();
        assert!(panel.selected_material.is_none());
        assert!(!panel.sandbox_active);
        assert_eq!(panel.mipmap_level, 0);
    }
}
