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
    /// Set mipmap level for a specific target.
    SetMipmapLevel { target: SelectionTarget, level: u8 },
    /// Toggle sandbox mode for a specific target.
    ToggleSandbox { target: SelectionTarget },
    /// Set PBR parameter override for a specific target.
    SetPBROverride { target: SelectionTarget, metallic: f32, roughness: f32 },
    /// Clear PBR overrides for a specific target.
    ClearOverrides { target: SelectionTarget },
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
    /// Set playback control mode for a specific human.
    SetPlayback { target: SelectionTarget, mode: PlaybackMode },
    /// Toggle bone tree node collapsed state for a specific human.
    ToggleBone { target: SelectionTarget, bone_name: String },
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
    StartSandbox { target: SelectionTarget },
    /// Update sandbox transform for the active sandbox target.
    UpdateTransform { target: SelectionTarget, position: [f32; 3], rotation: [f32; 3] },
    /// Apply sandbox changes for the active sandbox target.
    ApplySandbox { target: SelectionTarget },
    /// Revert sandbox changes for the active sandbox target.
    RevertSandbox { target: SelectionTarget },
    /// Close sandbox for the active sandbox target.
    CloseSandbox { target: SelectionTarget },
}

/// Export-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportCommand {
    /// Export selected entity to glTF.
    ExportSelection { target: SelectionTarget, destination: String },
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
    /// Sandbox not active (for editor commands).
    SandboxNotActive(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::StaleSelection(msg) => write!(f, "Stale selection: {}", msg),
            CommandError::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            CommandError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            CommandError::Failed(msg) => write!(f, "Failed: {}", msg),
            CommandError::SandboxNotActive(msg) => write!(f, "Sandbox not active: {}", msg),
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
        InspectorCommand::Human(cmd) => validate_human_command(cmd, selection),
        InspectorCommand::Editor(cmd) => validate_editor_command(cmd, selection),
        InspectorCommand::Export(cmd) => validate_export_command(cmd, selection),
        InspectorCommand::Telemetry(_) => Ok(()), // Telemetry commands are global
    }
}

fn validate_material_command(
    cmd: &MaterialCommand,
    selection: &InspectorSelection,
) -> CommandResult {
    let cmd_target = match cmd {
        MaterialCommand::SetMipmapLevel { target, level } => {
            if *level > 15 {
                return Err(CommandError::InvalidParameter(format!(
                    "Mipmap level {} exceeds maximum (15)",
                    level
                )));
            }
            target
        }
        MaterialCommand::ToggleSandbox { target } => target,
        MaterialCommand::SetPBROverride { target, metallic, roughness } => {
            if !(*metallic >= 0.0 && *metallic <= 1.0) {
                return Err(CommandError::InvalidParameter(format!(
                    "Metallic {} out of range [0.0, 1.0]",
                    metallic
                )));
            }
            if !(*roughness >= 0.0 && *roughness <= 1.0) {
                return Err(CommandError::InvalidParameter(format!(
                    "Roughness {} out of range [0.0, 1.0]",
                    roughness
                )));
            }
            target
        }
        MaterialCommand::ClearOverrides { target } => target,
    };

    validate_target_matches_selection(cmd_target, selection)
}

fn validate_human_command(cmd: &HumanCommand, selection: &InspectorSelection) -> CommandResult {
    let cmd_target = match cmd {
        HumanCommand::SetPlayback { target, .. } => target,
        HumanCommand::ToggleBone { target, .. } => target,
    };

    // Validate type match
    if !matches!(cmd_target, SelectionTarget::Human { .. }) {
        return Err(CommandError::NotSupported(
            "Human commands require human selection".to_string(),
        ));
    }

    validate_target_matches_selection(cmd_target, selection)
}

fn validate_editor_command(
    cmd: &EditorCommand,
    selection: &InspectorSelection,
) -> CommandResult {
    let cmd_target = match cmd {
        EditorCommand::StartSandbox { target } => target,
        EditorCommand::UpdateTransform { target, .. } => target,
        EditorCommand::ApplySandbox { target } => target,
        EditorCommand::RevertSandbox { target } => target,
        EditorCommand::CloseSandbox { target } => target,
    };

    // All editor commands require a valid target selection
    validate_target_matches_selection(cmd_target, selection)?;

    // UpdateTransform, ApplySandbox, RevertSandbox, and CloseSandbox require an active sandbox
    // Since we don't have access to sandbox state here, we only validate the target.
    // The actual sandbox state check should be done at execution time by the subsystem.
    // However, we add a marker for callers to check sandbox state separately.
    match cmd {
        EditorCommand::StartSandbox { .. } => {
            // StartSandbox is OK with just valid target
            Ok(())
        }
        EditorCommand::UpdateTransform { .. }
        | EditorCommand::ApplySandbox { .. }
        | EditorCommand::RevertSandbox { .. }
        | EditorCommand::CloseSandbox { .. } => {
            // These require active sandbox but we can't validate that here without sandbox state.
            // Return a specific error that the caller should check sandbox state.
            // For now, we allow these through and rely on execution-time validation.
            Ok(())
        }
    }
}

/// Extended validation that includes sandbox state check for editor commands.
/// Use this when you have access to the EditorAdapter to check sandbox state.
pub fn validate_editor_command_with_sandbox(
    cmd: &EditorCommand,
    selection: &InspectorSelection,
    sandbox_target: Option<&SelectionTarget>,
) -> CommandResult {
    // First do standard target validation
    let cmd_target = match cmd {
        EditorCommand::StartSandbox { target } => target,
        EditorCommand::UpdateTransform { target, .. } => target,
        EditorCommand::ApplySandbox { target } => target,
        EditorCommand::RevertSandbox { target } => target,
        EditorCommand::CloseSandbox { target } => target,
    };

    validate_target_matches_selection(cmd_target, selection)?;

    // Check sandbox state requirements
    match cmd {
        EditorCommand::StartSandbox { .. } => {
            // StartSandbox should not have an active sandbox
            if sandbox_target.is_some() {
                return Err(CommandError::NotSupported(
                    "Sandbox already active".to_string()
                ));
            }
            Ok(())
        }
        EditorCommand::UpdateTransform { target, .. }
        | EditorCommand::ApplySandbox { target }
        | EditorCommand::RevertSandbox { target }
        | EditorCommand::CloseSandbox { target } => {
            // These require active sandbox matching the command target
            match sandbox_target {
                None => Err(CommandError::SandboxNotActive(
                    "No active sandbox for this operation".to_string()
                )),
                Some(active_target) => {
                    if targets_match(target, active_target) {
                        Ok(())
                    } else {
                        Err(CommandError::StaleSelection(
                            "Sandbox target does not match command target".to_string()
                        ))
                    }
                }
            }
        }
    }
}

fn validate_export_command(cmd: &ExportCommand, selection: &InspectorSelection) -> CommandResult {
    match cmd {
        ExportCommand::ExportSelection { target, .. } => {
            validate_target_matches_selection(target, selection)
        }
        ExportCommand::CancelExport => Ok(()),
    }
}

/// Validate that a command target matches the current selection.
/// Checks both type and generation counters for stable identity.
fn validate_target_matches_selection(
    cmd_target: &SelectionTarget,
    selection: &InspectorSelection,
) -> CommandResult {
    match &selection.status {
        SelectionStatus::None => Err(CommandError::StaleSelection(
            "No active selection".to_string(),
        )),
        SelectionStatus::Invalidated { reason } => Err(CommandError::StaleSelection(format!(
            "Selection invalidated: {}",
            reason
        ))),
        SelectionStatus::Selected(current_target) => {
            if !targets_match(cmd_target, current_target) {
                Err(CommandError::StaleSelection(
                    "Command target does not match current selection".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    }
}

/// Check if two selection targets match (same entity with same generation).
fn targets_match(a: &SelectionTarget, b: &SelectionTarget) -> bool {
    match (a, b) {
        (
            SelectionTarget::Vehicle { key: k1, mesh: m1 },
            SelectionTarget::Vehicle { key: k2, mesh: m2 },
        ) => k1 == k2 && m1 == m2,
        (
            SelectionTarget::Scenery { key: k1, mesh: m1 },
            SelectionTarget::Scenery { key: k2, mesh: m2 },
        ) => k1 == k2 && m1 == m2,
        (
            SelectionTarget::Human { key: k1, mesh_id: m1 },
            SelectionTarget::Human { key: k2, mesh_id: m2 },
        ) => k1 == k2 && m1 == m2,
        _ => false,
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
        let target = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };
        let selection = InspectorSelection::new(target.clone());

        let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
            target,
            metallic: 1.5,
            roughness: 0.5,
        });
        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_human_command_wrong_selection() {
        let vehicle_target = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };
        let selection = InspectorSelection::new(vehicle_target);

        let human_target = SelectionTarget::Human {
            key: HumanKey {
                id: 1,
                generation: 1,
                is_driver: false,
            },
            mesh_id: None,
        };
        let cmd = InspectorCommand::Human(HumanCommand::SetPlayback {
            target: human_target,
            mode: PlaybackMode::Paused,
        });
        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_export_no_selection() {
        let selection = InspectorSelection::default();
        let target = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };
        let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
            target,
            destination: "/tmp/test.glb".to_string(),
        });

        let result = validate_command(&cmd, &selection);
        assert!(result.is_err());
    }
}
