//! UI-independent inspector view models.
//!
//! Owned, serializable snapshots for ImGui or other UI backends.
//! No egui dependencies, no mutable simulation borrows across frames.

use serde::{Deserialize, Serialize};
use crate::inspector::core::*;

/// Subsystem adapter trait for building view models from runtime state.
///
/// Each inspector subsystem (material, render, human, etc.) implements this
/// trait to snapshot its current state into an owned, serializable view model.
pub trait SubsystemAdapter {
    /// The view model type this adapter produces.
    type ViewModel;
    
    /// Snapshot the current subsystem state for the given selection target.
    fn snapshot(&self, target: &SelectionTarget) -> Option<Self::ViewModel>;
}

/// Material subsystem adapter interface.
pub trait MaterialAdapter: SubsystemAdapter<ViewModel = MaterialView> {
    /// Check if sandbox mode is active for the target.
    fn is_sandbox_active(&self, target: &SelectionTarget) -> bool;
    
    /// Get current mipmap level for the target material.
    fn get_mipmap_level(&self, target: &SelectionTarget) -> Option<u8>;
}

/// Render subsystem adapter interface.
pub trait RenderAdapter: SubsystemAdapter<ViewModel = RenderView> {
    /// Get active isolation mode.
    fn get_isolation_mode(&self) -> Option<String>;
    
    /// Check if wireframe mode is enabled.
    fn is_wireframe_enabled(&self) -> bool;
}

/// Human subsystem adapter interface.
pub trait HumanAdapter: SubsystemAdapter<ViewModel = HumanView> {
    /// Get current playback control state.
    fn get_playback_state(&self, target: &SelectionTarget) -> Option<String>;
    
    /// Get animation phase for the target human.
    fn get_animation_phase(&self, target: &SelectionTarget) -> Option<f32>;
}

/// Telemetry subsystem adapter interface.
pub trait TelemetryAdapter: SubsystemAdapter<ViewModel = TelemetryView> {
    /// Get current frame time in milliseconds.
    fn get_frame_time_ms(&self) -> f32;
    
    /// Get inspector query time in milliseconds.
    fn get_query_time_ms(&self) -> f32;
}

/// Editor subsystem adapter interface.
pub trait EditorAdapter: SubsystemAdapter<ViewModel = EditorView> {
    /// Check if a transform sandbox is active for the target.
    fn is_sandbox_active(&self, target: &SelectionTarget) -> bool;
    
    /// Get the active sandbox target, if any.
    fn get_sandbox_target(&self) -> Option<SelectionTarget>;
}

/// Export subsystem adapter interface.
pub trait ExportAdapter: SubsystemAdapter<ViewModel = ExportView> {
    /// Get current export status.
    fn get_export_status(&self) -> ExportStatus;
    
    /// Get export destination path, if an export is in progress.
    fn get_export_destination(&self) -> Option<String>;
}

/// Material panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialView {
    /// Material name or identifier.
    pub name: String,
    /// Shader variant.
    pub shader_variant: String,
    /// Alpha mode (Opaque, Mask, Blend).
    pub alpha_mode: String,
    /// Blend mode.
    pub blend_mode: String,
    /// Base color RGBA.
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Base texture path (if any).
    pub base_texture: Option<String>,
    /// Current mipmap level.
    pub mipmap_level: u8,
    /// Whether sandbox overrides are active.
    pub sandbox_active: bool,
}

/// Render panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderView {
    /// Frame graph passes.
    pub passes: Vec<PassView>,
    /// Total frame time (ms).
    pub total_frame_time_ms: f32,
    /// Total draw calls.
    pub total_draw_calls: usize,
    /// Total triangles rendered.
    pub total_triangles: usize,
    /// Active isolation mode.
    pub isolation_mode: Option<String>,
    /// Wireframe enabled.
    pub wireframe_enabled: bool,
    /// Collision hulls enabled.
    pub collision_hulls_enabled: bool,
    /// Show normals.
    pub show_normals: bool,
    /// Show UV seams.
    pub show_uv_seams: bool,
}

/// Single render pass view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassView {
    /// Pass name.
    pub name: String,
    /// GPU time (ms).
    pub gpu_time_ms: f32,
    /// Draw calls in this pass.
    pub draw_calls: usize,
    /// Triangles in this pass.
    pub triangles: usize,
    /// Child passes (for hierarchical passes).
    pub children: Vec<PassView>,
}

/// Human panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanView {
    /// Human ID.
    pub id: u32,
    /// Human generation.
    pub generation: u64,
    /// Whether this is a driver.
    pub is_driver: bool,
    /// World position.
    pub position: [f32; 3],
    /// Velocity (m/s).
    pub velocity: f32,
    /// Current animation name.
    pub current_animation: String,
    /// Animation phase [0.0, 1.0].
    pub animation_phase: f32,
    /// Skeleton bones (simplified).
    pub bone_count: usize,
    /// Playback control state.
    pub playback: String,
}

/// Telemetry panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryView {
    /// Timestamp (milliseconds since epoch).
    pub timestamp_ms: u64,
    /// Frame time (ms).
    pub frame_time_ms: f32,
    /// Inspector query time (ms).
    pub inspector_query_time_ms: f32,
    /// GPU staging time (ms).
    pub gpu_staging_time_ms: f32,
    /// Active watch expressions.
    pub watch_expressions: Vec<WatchExpressionView>,
}

/// Watch expression view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchExpressionView {
    /// Expression string (e.g., "L.throttle").
    pub expression: String,
    /// Current value.
    pub value: String,
    /// Optional label.
    pub label: Option<String>,
}

/// Editor panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorView {
    /// Whether a transform sandbox is active.
    pub sandbox_active: bool,
    /// Original transform (if sandbox active).
    pub original_transform: Option<TransformView>,
    /// Current transform (if sandbox active).
    pub current_transform: Option<TransformView>,
    /// Entity key being edited.
    pub entity_key: Option<String>,
    /// Transaction count.
    pub transaction_count: usize,
}

/// Transform view (position + rotation).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TransformView {
    /// Position [x, y, z].
    pub position: [f32; 3],
    /// Rotation [yaw, pitch, roll] in degrees.
    pub rotation: [f32; 3],
}

/// Export panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportView {
    /// Export status.
    pub status: ExportStatus,
    /// Export destination path (if any).
    pub destination: Option<String>,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Export status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    Idle,
    InProgress,
    Success,
    Failed,
}

/// Inspector main panel view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorMainView {
    /// Current selection status.
    pub selection_status: String,
    /// Selected entity type.
    pub entity_type: Option<String>,
    /// Stable identity (e.g., "Player Vehicle", "AI Car #42").
    pub entity_identity: Option<String>,
    /// World position.
    pub position: Option<[f32; 3]>,
    /// Rotation (quaternion [x, y, z, w]).
    pub rotation: Option<[f32; 4]>,
    /// Model path.
    pub model_path: Option<String>,
    /// Selected mesh name.
    pub mesh_name: Option<String>,
    /// Entity metadata (key-value pairs).
    pub metadata: Vec<(String, String)>,
    /// Penetration stack (for Tab cycling).
    pub penetration_stack: Vec<PenetrationHitView>,
    /// Current hit index in stack.
    pub current_hit_index: usize,
    /// Internal: full selection target for snapshot validation.
    #[serde(skip)]
    _selection_target: Option<SelectionTarget>,
}

/// Penetration hit view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenetrationHitView {
    /// Distance from ray origin (meters).
    pub distance: f32,
    /// Display name (e.g., "[1.2m] cockpit_speedo.o3d").
    pub display_name: String,
}

impl From<&InspectorSelection> for InspectorMainView {
    fn from(selection: &InspectorSelection) -> Self {
        let (selection_status, entity_type, entity_identity, selection_target) = match &selection.status {
            SelectionStatus::None => ("No selection".to_string(), None, None, None),
            SelectionStatus::Selected(target) => {
                let (etype, eid) = match target {
                    SelectionTarget::Vehicle { key, .. } => {
                        let id = match key {
                            VehicleKey::Player { generation } => format!("Player Vehicle (gen {})", generation),
                            VehicleKey::AiCar { id } => format!("AI Car #{}", id),
                            VehicleKey::Remote { player_id, generation } => format!("Remote #{} (gen {})", player_id, generation),
                            VehicleKey::PlayerTrailer { generation, trailer_index } => format!("Player Trailer {} (gen {})", trailer_index, generation),
                            VehicleKey::AiTrailer { car_id, trailer_index } => format!("AI Car #{} Trailer {}", car_id, trailer_index),
                            VehicleKey::RemoteTrailer { player_id, generation, trailer_index } => format!("Remote #{} Trailer {} (gen {})", player_id, trailer_index, generation),
                        };
                        ("Vehicle".to_string(), id)
                    }
                    SelectionTarget::Scenery { key, .. } => {
                        let id = match key {
                            SceneryKey::Editable { map_id } => format!("Editable #{}", map_id),
                            SceneryKey::NonEditable { tile_x, tile_y, key } => format!("Tile ({}, {}) key {}", tile_x, tile_y, key),
                            SceneryKey::Parked { key } => format!("Parked #{}", key),
                        };
                        ("Scenery".to_string(), id)
                    }
                    SelectionTarget::Human { key, .. } => {
                        let id = format!("Human #{} (gen {})", key.id, key.generation);
                        ("Human".to_string(), id)
                    }
                };
                ("Selected".to_string(), Some(etype), Some(eid), Some(target.clone()))
            }
            SelectionStatus::Invalidated { reason } => {
                (format!("Invalidated: {}", reason), None, None, None)
            }
        };

        let penetration_stack = selection
            .penetration_stack
            .iter()
            .map(|hit| PenetrationHitView {
                distance: hit.distance,
                display_name: hit.display_name.clone(),
            })
            .collect();

        Self {
            selection_status,
            entity_type,
            entity_identity,
            position: None,
            rotation: None,
            model_path: None,
            mesh_name: None,
            metadata: Vec::new(),
            penetration_stack,
            current_hit_index: selection.current_hit_index,
            // Store the full selection target for snapshot validation
            _selection_target: selection_target,
        }
    }
}

impl InspectorMainView {
    /// Enrich with snapshot data. Validates that the snapshot target matches the view's selection.
    /// Performs full typed identity validation including generation counters and mesh identity.
    pub fn with_snapshot(mut self, snapshot: &InspectorSnapshot) -> Result<Self, String> {
        // Validate snapshot matches the view's current selection
        if self.selection_status == "No selection" {
            return Err("Cannot enrich view with no selection".to_string());
        }
        
        if self.selection_status.starts_with("Invalidated") {
            return Err("Cannot enrich invalidated selection".to_string());
        }

        // Full typed identity validation: compare generation counters, entity keys, and mesh identity
        if let Some(ref view_target) = self._selection_target {
            if !targets_match(view_target, &snapshot.target) {
                return Err(format!(
                    "Snapshot target mismatch: view and snapshot targets do not match (different generation, entity, or mesh)"
                ));
            }
        } else {
            // Fallback to type-only validation if we don't have the full target
            let snapshot_type = match &snapshot.target {
                SelectionTarget::Vehicle { .. } => "Vehicle",
                SelectionTarget::Scenery { .. } => "Scenery",
                SelectionTarget::Human { .. } => "Human",
            };
            
            if let Some(ref view_type) = self.entity_type {
                if view_type != snapshot_type {
                    return Err(format!(
                        "Snapshot type mismatch: view has {}, snapshot has {}",
                        view_type, snapshot_type
                    ));
                }
            }
        }

        self.position = snapshot.position;
        self.rotation = snapshot.rotation;
        self.model_path = snapshot.model_path.clone();
        self.mesh_name = snapshot.mesh_name.clone();
        self.metadata = snapshot.metadata.clone();
        Ok(self)
    }
}

/// Helper function to check if two SelectionTargets match (including generation and mesh identity).
/// This is the same logic used in commands.rs for command validation.
fn targets_match(a: &SelectionTarget, b: &SelectionTarget) -> bool {
    match (a, b) {
        (
            SelectionTarget::Vehicle { key: key_a, mesh: mesh_a },
            SelectionTarget::Vehicle { key: key_b, mesh: mesh_b },
        ) => key_a == key_b && mesh_a == mesh_b,
        (
            SelectionTarget::Scenery { key: key_a, mesh: mesh_a },
            SelectionTarget::Scenery { key: key_b, mesh: mesh_b },
        ) => key_a == key_b && mesh_a == mesh_b,
        (
            SelectionTarget::Human { key: key_a, mesh_id: mesh_a },
            SelectionTarget::Human { key: key_b, mesh_id: mesh_b },
        ) => key_a == key_b && mesh_a == mesh_b,
        _ => false,
    }
}

impl PartialEq for MaterialView {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.shader_variant == other.shader_variant
            && self.alpha_mode == other.alpha_mode
            && self.blend_mode == other.blend_mode
            && self.base_color == other.base_color
            && (self.metallic - other.metallic).abs() < 1e-6
            && (self.roughness - other.roughness).abs() < 1e-6
            && self.base_texture == other.base_texture
            && self.mipmap_level == other.mipmap_level
            && self.sandbox_active == other.sandbox_active
    }
}

impl PartialEq for RenderView {
    fn eq(&self, other: &Self) -> bool {
        (self.total_frame_time_ms - other.total_frame_time_ms).abs() < 1e-6
            && self.total_draw_calls == other.total_draw_calls
            && self.total_triangles == other.total_triangles
            && self.isolation_mode == other.isolation_mode
            && self.wireframe_enabled == other.wireframe_enabled
            && self.collision_hulls_enabled == other.collision_hulls_enabled
            && self.show_normals == other.show_normals
            && self.show_uv_seams == other.show_uv_seams
    }
}

impl PartialEq for HumanView {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && self.is_driver == other.is_driver
            && (self.velocity - other.velocity).abs() < 1e-6
            && self.current_animation == other.current_animation
            && (self.animation_phase - other.animation_phase).abs() < 1e-6
            && self.bone_count == other.bone_count
            && self.playback == other.playback
    }
}
