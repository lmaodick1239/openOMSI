//! Inspector subsystem modules.

#[path = "../inspector_core.rs"]
pub mod core;

pub mod commands;
pub mod editor_bridge;
pub mod export;
pub mod persistence;
pub mod telemetry;
pub mod view_models;

#[cfg(not(target_os = "android"))]
pub mod imgui_inspector;

// Re-export core types
pub use core::*;

// Re-export Phase 5 types
pub use editor_bridge::{EditorBridge, EditorTransition, Transform, TransformSandbox};
pub use export::{export_to_gltf, AlphaMode, ExportError, MaterialData, MeshData};
pub use persistence::{InspectorLayout, InspectorTab, PanelGeometry, WatchExpression};
pub use telemetry::{InspectorSnapshot as TelemetrySnapshot, TelemetryServer};

// Re-export view models and commands
pub use commands::*;
pub use view_models::*;
