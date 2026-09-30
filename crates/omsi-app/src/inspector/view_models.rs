//! UI-independent inspector view models.
//!
//! Owned, serializable snapshots for ImGui or other UI backends.
//! No egui dependencies, no mutable simulation borrows across frames.

use serde::{Deserialize, Serialize};
use crate::inspector::core::*;

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
        let (selection_status, entity_type, entity_identity) = match &selection.status {
            SelectionStatus::None => ("No selection".to_string(), None, None),
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
                ("Selected".to_string(), Some(etype), Some(eid))
            }
            SelectionStatus::Invalidated { reason } => {
                (format!("Invalidated: {}", reason), None, None)
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
        }
    }
}

impl InspectorMainView {
    /// Enrich with snapshot data.
    pub fn with_snapshot(mut self, snapshot: &InspectorSnapshot) -> Self {
        self.position = snapshot.position;
        self.rotation = snapshot.rotation;
        self.model_path = snapshot.model_path.clone();
        self.mesh_name = snapshot.mesh_name.clone();
        self.metadata = snapshot.metadata.clone();
        self
    }
}
