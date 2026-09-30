//! Tests for editor sandbox command validation with sandbox state.

use openomsi_game::inspector::*;

fn make_vehicle_target(generation: u64) -> SelectionTarget {
    SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation },
        mesh: None,
    }
}

fn make_scenery_target(map_id: i64) -> SelectionTarget {
    SelectionTarget::Scenery {
        key: SceneryKey::Editable { map_id },
        mesh: None,
    }
}

#[test]
fn test_validate_start_sandbox_no_active_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = EditorCommand::StartSandbox { target };

    // Should succeed when no sandbox is active
    let result = validate_editor_command_with_sandbox(&cmd, &selection, None);
    assert!(result.is_ok());
}

#[test]
fn test_validate_start_sandbox_with_active_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let active_sandbox = make_vehicle_target(1);
    let cmd = EditorCommand::StartSandbox { target };

    // Should fail when sandbox already active
    let result = validate_editor_command_with_sandbox(&cmd, &selection, Some(&active_sandbox));
    assert!(result.is_err());
    match result {
        Err(CommandError::NotSupported(msg)) => {
            assert!(msg.contains("already active"));
        }
        _ => panic!("Expected NotSupported error"),
    }
}

#[test]
fn test_validate_update_transform_no_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = EditorCommand::UpdateTransform {
        target,
        position: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0],
    };

    // Should fail when no sandbox is active
    let result = validate_editor_command_with_sandbox(&cmd, &selection, None);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(msg)) => {
            assert!(msg.contains("No active sandbox"));
        }
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_update_transform_matching_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let active_sandbox = target.clone();
    let cmd = EditorCommand::UpdateTransform {
        target,
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 90.0, 0.0],
    };

    // Should succeed when sandbox matches target
    let result = validate_editor_command_with_sandbox(&cmd, &selection, Some(&active_sandbox));
    assert!(result.is_ok());
}

#[test]
fn test_validate_update_transform_mismatched_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let active_sandbox = make_vehicle_target(2); // Different generation
    let cmd = EditorCommand::UpdateTransform {
        target,
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 90.0, 0.0],
    };

    // Should fail when sandbox target doesn't match
    let result = validate_editor_command_with_sandbox(&cmd, &selection, Some(&active_sandbox));
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(msg)) => {
            assert!(msg.contains("does not match"));
        }
        _ => panic!("Expected StaleSelection error"),
    }
}

#[test]
fn test_validate_apply_sandbox_no_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = EditorCommand::ApplySandbox { target };

    // Should fail when no sandbox is active
    let result = validate_editor_command_with_sandbox(&cmd, &selection, None);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_apply_sandbox_matching_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let active_sandbox = target.clone();
    let cmd = EditorCommand::ApplySandbox { target };

    // Should succeed when sandbox matches target
    let result = validate_editor_command_with_sandbox(&cmd, &selection, Some(&active_sandbox));
    assert!(result.is_ok());
}

#[test]
fn test_validate_revert_sandbox_no_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = EditorCommand::RevertSandbox { target };

    // Should fail when no sandbox is active
    let result = validate_editor_command_with_sandbox(&cmd, &selection, None);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_close_sandbox_no_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = EditorCommand::CloseSandbox { target };

    // Should fail when no sandbox is active
    let result = validate_editor_command_with_sandbox(&cmd, &selection, None);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_sandbox_different_entity_types() {
    let vehicle_target = make_vehicle_target(1);
    let scenery_target = make_scenery_target(100);
    let selection = InspectorSelection::new(vehicle_target.clone());
    let active_sandbox = scenery_target.clone();

    let cmd = EditorCommand::UpdateTransform {
        target: vehicle_target,
        position: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0],
    };

    // Should fail when entity types don't match
    let result = validate_editor_command_with_sandbox(&cmd, &selection, Some(&active_sandbox));
    assert!(result.is_err());
}

#[test]
fn test_command_error_sandbox_not_active_display() {
    let err = CommandError::SandboxNotActive("test message".to_string());
    assert_eq!(format!("{}", err), "Sandbox not active: test message");
}
