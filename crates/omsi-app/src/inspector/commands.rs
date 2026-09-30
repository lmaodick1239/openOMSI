//! UI-independent inspector commands.
//!
//! Typed, validated commands queued by UI and executed at application boundary.
//! Preserves stable selection keys and prevents stale UI references.

use serde::{Deserialize, Serialize};
use crate::inspector::core::*;

/// Inspector command enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectorCommand {
    /// Select an entity.
    Select(SelectionTarget),
    /// Clear the current selection.
    ClearSelection,
    /// Cycle to next penetration hit (Tab).
    CycleNextHit,
    /// Cycle to previous penetration hit (Shift+Tab).
    CyclePrevHit,
    /// Jump to specific penetration hit index.
    JumpToHit(usize),
    /// Toggle view option.
    ToggleView(ViewToggle),
    /// Material commands.
    Material(MaterialCommand),
    /// Render commands.
    Render(RenderCommand),
    /// Human commands.
    Human(HumanCommand),
    /// Editor commands.
    Editor(EditorCommand),
    /// Export commands.
    Export(ExportCommand),
    /// Telemetry commands.
    Telemetry(TelemetryCommand),
}

/// View toggle commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewToggle {
    ShowBounds,
    ShowLocalAxes,
    ShowMeshName,
}

/// Material-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaterialCommand {
    /// Set mipmap level.
    SetMipmapLevel(u8),
    /// Toggle sandbox mode.
    ToggleSandbox,
    /// Set PBR parameter override.
    SetPBROverride { metallic: f32, roughness: f32 },
    /// Clear PBR overrides.
    ClearOverrides,
}

/// Render-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderCommand {
    /// Toggle render pass on/off.
    TogglePass(String),
    /// Set isolation mode.
    SetIsolation(Option<String>),
    /// Toggle wireframe mode.
    ToggleWireframe,
    /// Set wireframe color.
    SetWireframeColor([f32; 4]),
    /// Toggle collision hulls.
    ToggleCollisionHulls,
    /// Toggle surface normals visualization.
    ToggleNormals,
    /// Toggle UV seam visualization.
    ToggleUVSeams,
}

/// Human-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HumanCommand {
    /// Set playback control mode.
    SetPlayback(PlaybackMode),
    /// Toggle bone tree node collapsed state.
    ToggleBone(String),
}

/// Playback control mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackMode {
    Playing,
    Paused,
    SlowMotion,
}

/// Editor-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorCommand {
    /// Start transform sandbox for entity.
    StartSandbox(String),
    /// Update sandbox transform.
    UpdateTransform { position: [f32; 3], rotation: [f32; 3] },
    /// Apply sandbox changes.
    ApplySandbox,
    /// Revert sandbox changes.
    RevertSandbox,
    /// Close sandbox.
    CloseSandbox,
}

/// Export-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportCommand {
    /// Export selected entity to glTF.
    ExportSelection { destination: String },
    /// Cancel ongoing export.
    CancelExport,
}

/// Telemetry-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryCommand {
    /// Add watch expression.
    AddWatch(String),
    /// Remove watch expression.
    RemoveWatch(String),
    /// Clear all watch expressions.
    ClearWatches,
}

/// Command validation result.
pub type CommandResult = Result<(), CommandError>;

/// Command validation/execution errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandError {
    /// Selection is stale or invalid.
    StaleSelection(String),
    /// Command not supported for current selection.
    NotSupported(String),
    /// Invalid parameter.
    InvalidParameter(String),
    /// Operation failed.
    Failed(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::StaleSelection(msg) => write!(f, "Stale selection: {}", msg),
            CommandError::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            CommandError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            CommandError::Failed(msg) => write!(f, "Failed: {}", msg),
        }
    }
}

impl std::error::Error for CommandError {}

/// Validate a command against current selection state.
pub fn validate_command(
    command: &InspectorCommand,
    selection: &InspectorSelection,
) -> CommandResult {
    match command {
        InspectorCommand::Select(_) => Ok(()),
        InspectorCommand::ClearSelection => Ok(()),
        InspectorCommand::CycleNextHit | InspectorCommand::CyclePrevHit => {
            if selection.penetration_stack.is_empty() {
                Err(CommandError::InvalidParameter(
                    "No penetration stack available".to_string(),
                ))
            } else {
                Ok(())
            }
        }
        InspectorCommand::JumpToHit(index) => {
            if *index >= selection.penetration_stack.len() {
                Err(CommandError::InvalidParameter(format!(
                    "Hit index {} out of range (max {})",
                    index,
                    selection.penetration_stack.len()
                )))
            } else {
                Ok(())
            }
        }
        InspectorCommand::ToggleView(_) => {
            if !selection.is_active() {
                Err(CommandError::StaleSelection(
                    "No active selection".to_string(),
                ))
            } else {
                Ok(())
            }
        }
        InspectorCommand::Material(cmd) => validate_material_command(cmd, selection),
        InspectorCommand::Render(_) => Ok(()), // Render commands are global
        InspectorCommand::Human(_) => validate_human_command(selection),
        InspectorCommand::Editor(cmd) => validate_editor_command(cmd, selection),
        InspectorCommand::Export(_) => validate_export_command(selection),
        InspectorCommand::Telemetry(_) => Ok(()), // Telemetry commands are global
    }
}

fn validate_material_command(
    cmd: &MaterialCommand,
    selection: &InspectorSelection,
) -> CommandResult {
    if !selection.is_active() {
        return Err(CommandError::StaleSelection(
            "No active selection for material command".to_string(),
        ));
    }

    match cmd {
        MaterialCommand::SetMipmapLevel(level) => {
            if *level > 15 {
                Err(CommandError::InvalidParameter(format!(
                    "Mipmap level {} exceeds maximum (15)",
                    level
                )))
            } else {
                Ok(())
            }
        }
        MaterialCommand::SetPBROverride { metallic, roughness } => {
            if !(*metallic >= 0.0 && *metallic <= 1.0) {
                Err(CommandError::InvalidParameter(format!(
                    "Metallic {} out of range [0.0, 1.0]",
                    metallic
                )))
            } else if !(*roughness >= 0.0 && *roughness <= 1.0) {
                Err(CommandError::InvalidParameter(format!(
                    "Roughness {} out of range [0.0, 1.0]",
                    roughness
                )))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn validate_human_command(selection: &InspectorSelection) -> CommandResult {
    if !selection.is_active() {
        return Err(CommandError::StaleSelection(
            "No active selection for human command".to_string(),
        ));
    }

    match &selection.status {
        SelectionStatus::Selected(SelectionTarget::Human { .. }) => Ok(()),
        SelectionStatus::Selected(_) => Err(CommandError::NotSupported(
            "Human commands require human selection".to_string(),
        )),
        _ => Err(CommandError::StaleSelection(
            "No active selection".to_string(),
        )),
    }
}

fn validate_editor_command(
    cmd: &EditorCommand,
    selection: &InspectorSelection,
) -> CommandResult {
    match cmd {
        EditorCommand::StartSandbox(_) => {
            if !selection.is_active() {
                Err(CommandError::StaleSelection(
                    "No active selection for editor sandbox".to_string(),
                ))
            } else {
                Ok(())
            }
        }
        EditorCommand::UpdateTransform { .. }
        | EditorCommand::ApplySandbox
        | EditorCommand::RevertSandbox
        | EditorCommand::CloseSandbox => {
            // These require an active sandbox, validated at execution time
            Ok(())
        }
    }
}

fn validate_export_command(selection: &InspectorSelection) -> CommandResult {
    if !selection.is_active() {
        Err(CommandError::StaleSelection(
            "No active selection to export".to_string(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialization() {
        let cmd = InspectorCommand::Select(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: InspectorCommand = serde_json::from_str(&json).unwrap();

        match deserialized {
            InspectorCommand::Select(SelectionTarget::Vehicle { key, .. }) => {
                assert_eq!(key, VehicleKey::Player { generation: 1 });
            }
            _ => panic!("Expected Select command"),
        }
    }

    #[test]
    fn test_validate_cycle_with_empty_stack() {
        let selection = InspectorSelection::default();
        let cmd = InspectorCommand::CycleNextHit;

        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jump_out_of_range() {
        let mut selection = InspectorSelection::default();
        selection.penetration_stack = vec![
            PenetrationHit::new(
                1.0,
                SelectionTarget::Vehicle {
                    key: VehicleKey::Player { generation: 1 },
                    mesh: None,
                },
                "Test".to_string(),
            ),
        ];

        let cmd = InspectorCommand::JumpToHit(5);
        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_material_mipmap_level() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel(20));
        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_material_pbr_range() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
            metallic: 1.5,
            roughness: 0.5,
        });
        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_human_command_wrong_selection() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        let cmd = InspectorCommand::Human(HumanCommand::SetPlayback(PlaybackMode::Paused));
        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_export_no_selection() {
        let selection = InspectorSelection::default();
        let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
            destination: "/tmp/test.glb".to_string(),
        });

        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }
}
