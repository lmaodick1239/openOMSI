//! Tests for context-aware command validation.

use openomsi_game::inspector::*;

struct MockEditorAdapter {
    sandbox_target: Option<SelectionTarget>,
}

impl SubsystemAdapter for MockEditorAdapter {
    type ViewModel = EditorView;

    fn snapshot(&self, _target: &SelectionTarget) -> Option<Self::ViewModel> {
        None
    }
}

impl EditorAdapter for MockEditorAdapter {
    fn is_sandbox_active(&self, target: &SelectionTarget) -> bool {
        self.sandbox_target.as_ref() == Some(target)
    }

    fn get_sandbox_target(&self) -> Option<SelectionTarget> {
        self.sandbox_target.clone()
    }
}

fn make_vehicle_target(generation: u64) -> SelectionTarget {
    SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation },
        mesh: None,
    }
}

#[test]
fn test_validate_command_rejects_sandbox_ops_without_context() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    
    // UpdateTransform should be rejected by validate_command (no context)
    let cmd = InspectorCommand::Editor(EditorCommand::UpdateTransform {
        target,
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 90.0, 0.0],
    });
    
    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_command_with_editor_adapter_rejects_inactive_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let command = InspectorCommand::Editor(EditorCommand::ApplySandbox { target });
    let editor = MockEditorAdapter {
        sandbox_target: None,
    };

    assert!(matches!(
        validate_command_with_editor_adapter(&command, &selection, &editor),
        Err(CommandError::SandboxNotActive(_))
    ));
}

#[test]
fn test_validate_command_with_editor_adapter_accepts_matching_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let command = InspectorCommand::Editor(EditorCommand::UpdateTransform {
        target: target.clone(),
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 90.0, 0.0],
    });
    let editor = MockEditorAdapter {
        sandbox_target: Some(target),
    };

    assert!(validate_command_with_editor_adapter(&command, &selection, &editor).is_ok());
}

#[test]
fn test_validate_command_with_context_accepts_sandbox_ops() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let sandbox_target = target.clone();
    
    let cmd = InspectorCommand::Editor(EditorCommand::UpdateTransform {
        target,
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 90.0, 0.0],
    });
    
    let context = ValidationContext {
        sandbox_target: Some(&sandbox_target),
    };
    
    let result = validate_command_with_context(&cmd, &selection, &context);
    assert!(result.is_ok());
}

#[test]
fn test_validate_command_with_context_rejects_mismatched_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let sandbox_target = make_vehicle_target(2); // Different generation
    
    let cmd = InspectorCommand::Editor(EditorCommand::UpdateTransform {
        target,
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 90.0, 0.0],
    });
    
    let context = ValidationContext {
        sandbox_target: Some(&sandbox_target),
    };
    
    let result = validate_command_with_context(&cmd, &selection, &context);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(_)) => {}
        _ => panic!("Expected StaleSelection error"),
    }
}

#[test]
fn test_validate_command_with_context_rejects_no_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    
    let cmd = InspectorCommand::Editor(EditorCommand::ApplySandbox {
        target,
    });
    
    let context = ValidationContext {
        sandbox_target: None,
    };
    
    let result = validate_command_with_context(&cmd, &selection, &context);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_command_allows_start_sandbox() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    
    let cmd = InspectorCommand::Editor(EditorCommand::StartSandbox {
        target,
    });
    
    // StartSandbox should be allowed by validate_command (no context needed)
    let result = validate_command(&cmd, &selection);
    assert!(result.is_ok());
}

#[test]
fn test_validate_command_with_context_rejects_start_when_active() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let active_sandbox = make_vehicle_target(1);
    
    let cmd = InspectorCommand::Editor(EditorCommand::StartSandbox {
        target,
    });
    
    let context = ValidationContext {
        sandbox_target: Some(&active_sandbox),
    };
    
    let result = validate_command_with_context(&cmd, &selection, &context);
    assert!(result.is_err());
    match result {
        Err(CommandError::NotSupported(_)) => {}
        _ => panic!("Expected NotSupported error"),
    }
}

#[test]
fn test_validate_command_with_context_passes_through_non_editor() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    
    let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel {
        target,
        level: 5,
    });
    
    let context = ValidationContext {
        sandbox_target: None,
    };
    
    // Material commands should pass through to standard validation
    let result = validate_command_with_context(&cmd, &selection, &context);
    assert!(result.is_ok());
}

#[test]
fn test_validate_close_sandbox_without_context() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    
    let cmd = InspectorCommand::Editor(EditorCommand::CloseSandbox {
        target,
    });
    
    // Should be rejected without context
    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}

#[test]
fn test_validate_revert_sandbox_without_context() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    
    let cmd = InspectorCommand::Editor(EditorCommand::RevertSandbox {
        target,
    });
    
    // Should be rejected without context
    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::SandboxNotActive(_)) => {}
        _ => panic!("Expected SandboxNotActive error"),
    }
}
