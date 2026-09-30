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

    /// Render material properties section.
    pub fn render_properties(&self, ui: &mut egui::Ui) {
        let snapshot = match &self.material_snapshot {
            Some(s) => s,
            None => {
                ui.label("No material selected");
                return;
            }
        };

        ui.heading("Material Properties");
        ui.separator();

        egui::Grid::new("material_props")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("Shader Variant:");
                ui.label(&snapshot.shader_variant);
                ui.end_row();

                ui.label("Alpha Mode:");
                ui.label(format!("{:?}", snapshot.alpha_mode));
                ui.end_row();

                ui.label("Blend Mode:");
                ui.label(format!("{:?}", snapshot.blend_mode));
                ui.end_row();

                ui.label("Render Pass:");
                ui.label(format!("{:?}", snapshot.render_pass));
                ui.end_row();

                ui.label("Double-Sided:");
                ui.label(if snapshot.double_sided { "Yes" } else { "No" });
                ui.end_row();

                if snapshot.alpha_mode == omsi_render::AlphaMode::Test {
                    ui.label("Alpha Cutoff:");
                    ui.label(format!("{:.3}", snapshot.alpha_cutoff));
                    ui.end_row();
                }

                ui.label("VRAM Usage:");
                ui.label(format_bytes(snapshot.vram_bytes));
                ui.end_row();
            });

        ui.add_space(8.0);

        // Color properties
        ui.label("Base Color:");
        let color = snapshot.color;
        let color_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(40.0, 20.0),
        );
        ui.painter().rect_filled(
            color_rect,
            4.0,
            egui::Color32::from_rgba_premultiplied(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
                (color[3] * 255.0) as u8,
            ),
        );
        ui.add_space(24.0);

        if snapshot.emissive != [0.0, 0.0, 0.0] {
            ui.label("Emissive:");
            let em = snapshot.emissive;
            let em_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                egui::vec2(40.0, 20.0),
            );
            ui.painter().rect_filled(
                em_rect,
                4.0,
                egui::Color32::from_rgb(
                    (em[0] * 255.0) as u8,
                    (em[1] * 255.0) as u8,
                    (em[2] * 255.0) as u8,
                ),
            );
            ui.add_space(24.0);
        }
    }

    /// Render texture preview section.
    pub fn render_texture_preview(&mut self, ui: &mut egui::Ui, scene: &Scene) {
        let snapshot = match &self.material_snapshot {
            Some(s) => s,
            None => return,
        };

        ui.heading("Texture Maps");
        ui.separator();

        if let Some(handle) = snapshot.diffuse_tex {
            self.render_texture_slot(ui, scene, "Diffuse/Albedo", handle);
        }

        if let Some(handle) = snapshot.normal_tex {
            self.render_texture_slot(ui, scene, "Normal Map", handle);
        }

        if let Some(handle) = snapshot.envmap_tex {
            self.render_texture_slot(ui, scene, "Environment Map", handle);
        }

        // Mipmap isolation slider
        if snapshot.diffuse_tex.is_some() {
            ui.add_space(8.0);
            ui.label("Mipmap Level:");
            ui.add(egui::Slider::new(&mut self.mipmap_level, 0..=10).text("Level"));
        }
    }

    /// Render PBR parameter sandbox.
    pub fn render_pbr_sandbox(&mut self, ui: &mut egui::Ui) {
        let snapshot = match &self.material_snapshot {
            Some(s) => s,
            None => return,
        };

        ui.heading("PBR Parameter Sandbox");
        ui.separator();

        ui.checkbox(&mut self.sandbox_active, "Enable Sandbox Mode");

        if !self.sandbox_active {
            ui.label("Enable sandbox to adjust parameters non-destructively");
            return;
        }

        // Initialize overrides from snapshot
        let overrides = self.pbr_overrides.get_or_insert(snapshot.pbr_params);

        ui.add_space(8.0);

        ui.label("Roughness:");
        ui.add(egui::Slider::new(&mut overrides.roughness, 0.0..=1.0).text(""));

        ui.label("Metalness:");
        ui.add(egui::Slider::new(&mut overrides.metalness, 0.0..=1.0).text(""));

        ui.label("Normal Intensity:");
        ui.add(egui::Slider::new(&mut overrides.normal_intensity, 0.0..=2.0).text(""));

        ui.label("Environment Reflection:");
        ui.add(egui::Slider::new(&mut overrides.env_reflection, 0.0..=3.0).text(""));

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Reset to Original").clicked() {
                self.pbr_overrides = Some(snapshot.pbr_params);
            }

            if ui.button("Copy Configuration").clicked() {
                self.copy_config_to_clipboard(overrides);
            }
        });
    }

    /// Render actions section.
    pub fn render_actions(&mut self, ui: &mut egui::Ui, scene: &mut Scene) {
        ui.heading("Actions");
        ui.separator();

        let has_diffuse = self
            .material_snapshot
            .as_ref()
            .and_then(|s| s.diffuse_tex)
            .is_some();

        ui.horizontal(|ui| {
            ui.set_enabled(has_diffuse);
            if ui.button("Export Texture to PNG").clicked() {
                self.request_texture_export(scene);
            }

            if ui.button("View Memory Heatmap").clicked() {
                // Placeholder - would activate heatmap visualization
                log::info!("Memory heatmap visualization requested");
            }
        });
    }

    // Internal helpers

    fn render_texture_slot(
        &self,
        ui: &mut egui::Ui,
        scene: &Scene,
        label: &str,
        handle: TextureHandle,
    ) {
        ui.label(label);

        let query = TextureQuery {
            handle,
            request_mip_level: Some(self.mipmap_level),
        };

        if let Some(result) = scene.query_texture_metadata_snapshot(query) {
            let meta = &result.metadata;
            ui.label(format!(
                "{}x{} {} ({})",
                meta.resolution.0,
                meta.resolution.1,
                meta.format,
                format_bytes(meta.vram_bytes as usize)
            ));
            ui.label(format!("Mipmaps: {}", meta.mipmap_count));

            // Placeholder for thumbnail preview
            let thumb_size = egui::vec2(128.0, 128.0);
            let thumb_rect = egui::Rect::from_min_size(ui.cursor().min, thumb_size);
            ui.painter().rect_filled(
                thumb_rect,
                4.0,
                egui::Color32::from_gray(64),
            );
            ui.painter().text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Preview",
                egui::FontId::default(),
                egui::Color32::WHITE,
            );
            ui.add_space(thumb_size.y + 4.0);
        }

        ui.add_space(8.0);
    }

    fn request_texture_export(&mut self, scene: &mut Scene) {
        if let Some(snapshot) = &self.material_snapshot {
            if let Some(handle) = snapshot.diffuse_tex {
                match scene.request_texture_readback(handle, self.mipmap_level) {
                    Some(id) => {
                        self.pending_readbacks.push(id);
                        log::info!("Texture readback requested: {:?}", id);
                    }
                    None => {
                        log::warn!("Failed to request texture readback");
                    }
                }
            }
        }
    }

    fn copy_config_to_clipboard(&self, params: &PBRParams) {
        let config = format!(
            "; PBR Material Configuration\n\
             [matl_roughness]\n{:.3}\n\
             [matl_metalness]\n{:.3}\n\
             [matl_normal_intensity]\n{:.3}\n\
             [matl_env_reflection]\n{:.3}\n",
            params.roughness, params.metalness, params.normal_intensity, params.env_reflection
        );

        // Placeholder - would use clipboard API
        log::info!("Configuration copied:\n{}", config);
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
