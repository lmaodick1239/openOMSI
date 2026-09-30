//! Inspector subsystem modules.

#[path = "../inspector_core.rs"]
pub mod core;

pub mod persistence;
pub mod export;
pub mod telemetry;
pub mod editor_bridge;

// Re-export core types
pub use core::*;

// Re-export Phase 5 types
pub use persistence::{InspectorLayout, InspectorTab, PanelGeometry, WatchExpression};
pub use export::{export_to_gltf, ExportError, MeshData, MaterialData, AlphaMode};
pub use telemetry::{TelemetryServer, InspectorSnapshot as TelemetrySnapshot};
pub use editor_bridge::{EditorBridge, TransformSandbox, EditorTransition, Transform};
