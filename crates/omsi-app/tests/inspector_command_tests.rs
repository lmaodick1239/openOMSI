//! Tests for UI-independent inspector commands with stable identity validation.

use openomsi_game::inspector::*;

fn make_vehicle_target(generation: u64) -> SelectionTarget {
    SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation },
        mesh: None,
    }
}

fn make_human_target(id: u32, generation: u64) -> SelectionTarget {
    SelectionTarget::Human {
        key: HumanKey {
            id,
            generation,
            is_driver: false,
        },
        mesh_id: None,
    }
}

#[test]
fn test_command_serialization_select() {
    let cmd = InspectorCommand::Select(make_vehicle_target(1));
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
fn test_command_serialization_material() {
    let target = make_vehicle_target(1);
    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        target: target.clone(),
        metallic: 0.8,
        roughness: 0.2,
    });

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: InspectorCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        InspectorCommand::Material(MaterialCommand::SetPBROverride {
            target: _,
            metallic,
            roughness,
        }) => {
            assert_eq!(metallic, 0.8);
            assert_eq!(roughness, 0.2);
        }
        _ => panic!("Expected Material SetPBROverride command"),
    }
}

#[test]
fn test_command_serialization_render() {
    let cmd = InspectorCommand::Render(RenderCommand::ToggleWireframe);
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: InspectorCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        InspectorCommand::Render(RenderCommand::ToggleWireframe) => {}
        _ => panic!("Expected Render ToggleWireframe command"),
    }
}

#[test]
fn test_validate_select_command() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::Select(make_vehicle_target(1));
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_clear_selection_command() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::ClearSelection;
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_cycle_no_stack() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::CycleNextHit;

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::InvalidParameter(msg)) => {
            assert!(msg.contains("penetration stack"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

#[test]
fn test_validate_cycle_with_stack() {
    let mut selection = InspectorSelection::default();
    selection.penetration_stack = vec![PenetrationHit::new(
        1.0,
        make_vehicle_target(1),
        "Test".to_string(),
    )];

    let cmd = InspectorCommand::CycleNextHit;
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_jump_to_hit_out_of_range() {
    let mut selection = InspectorSelection::default();
    selection.penetration_stack = vec![PenetrationHit::new(
        1.0,
        make_vehicle_target(1),
        "Test".to_string(),
    )];

    let cmd = InspectorCommand::JumpToHit(5);
    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
}

#[test]
fn test_validate_jump_to_hit_in_range() {
    let mut selection = InspectorSelection::default();
    selection.penetration_stack = vec![
        PenetrationHit::new(1.0, make_vehicle_target(1), "Test1".to_string()),
        PenetrationHit::new(2.0, make_vehicle_target(1), "Test2".to_string()),
    ];

    let cmd = InspectorCommand::JumpToHit(1);
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_toggle_view_no_selection() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::ToggleView(ViewToggle::ShowBounds);

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(_)) => {}
        _ => panic!("Expected StaleSelection error"),
    }
}

#[test]
fn test_validate_toggle_view_with_selection() {
    let selection = InspectorSelection::new(make_vehicle_target(1));
    let cmd = InspectorCommand::ToggleView(ViewToggle::ShowBounds);
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_material_mipmap_level_too_high() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel {
        target,
        level: 20,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::InvalidParameter(msg)) => {
            assert!(msg.contains("Mipmap level"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

#[test]
fn test_validate_material_mipmap_level_valid() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel { target, level: 5 });
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_material_pbr_metallic_out_of_range() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        target,
        metallic: 1.5,
        roughness: 0.5,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::InvalidParameter(msg)) => {
            assert!(msg.contains("Metallic"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

#[test]
fn test_validate_material_pbr_roughness_out_of_range() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        target,
        metallic: 0.5,
        roughness: 1.5,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::InvalidParameter(msg)) => {
            assert!(msg.contains("Roughness"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

#[test]
fn test_validate_material_pbr_valid() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        target,
        metallic: 0.8,
        roughness: 0.2,
    });
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_human_command_wrong_selection_type() {
    let vehicle_target = make_vehicle_target(1);
    let selection = InspectorSelection::new(vehicle_target);
    let human_target = make_human_target(1, 1);
    let cmd = InspectorCommand::Human(HumanCommand::SetPlayback {
        target: human_target,
        mode: PlaybackMode::Paused,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(_)) => {}
        _ => panic!("Expected StaleSelection error"),
    }
}

#[test]
fn test_validate_human_command_correct_selection_type() {
    let target = make_human_target(1, 1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Human(HumanCommand::SetPlayback {
        target,
        mode: PlaybackMode::Paused,
    });
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_export_no_selection() {
    let selection = InspectorSelection::default();
    let target = make_vehicle_target(1);
    let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
        target,
        destination: "test.glb".to_string(),
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(_)) => {}
        _ => panic!("Expected StaleSelection error"),
    }
}

#[test]
fn test_validate_export_with_selection() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
        target,
        destination: "test.glb".to_string(),
    });
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_render_commands_always_valid() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::Render(RenderCommand::ToggleWireframe);
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_telemetry_commands_always_valid() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::Telemetry(TelemetryCommand::AddWatch("test".to_string()));
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_command_error_display() {
    let err = CommandError::StaleSelection("test".to_string());
    assert_eq!(format!("{}", err), "Stale selection: test");

    let err = CommandError::NotSupported("test".to_string());
    assert_eq!(format!("{}", err), "Not supported: test");

    let err = CommandError::InvalidParameter("test".to_string());
    assert_eq!(format!("{}", err), "Invalid parameter: test");

    let err = CommandError::Failed("test".to_string());
    assert_eq!(format!("{}", err), "Failed: test");
}

// NEW TESTS FOR REVIEW FINDINGS

#[test]
fn test_validate_material_command_stale_generation() {
    let target_gen1 = make_vehicle_target(1);
    let target_gen2 = make_vehicle_target(2);
    let selection = InspectorSelection::new(target_gen2);
    
    let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel {
        target: target_gen1,
        level: 5,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(msg)) => {
            assert!(msg.contains("does not match"));
        }
        _ => panic!("Expected StaleSelection error for generation mismatch"),
    }
}

#[test]
fn test_validate_human_command_stale_generation() {
    let target_gen1 = make_human_target(42, 1);
    let target_gen2 = make_human_target(42, 2);
    let selection = InspectorSelection::new(target_gen2);
    
    let cmd = InspectorCommand::Human(HumanCommand::SetPlayback {
        target: target_gen1,
        mode: PlaybackMode::Paused,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(_)) => {}
        _ => panic!("Expected StaleSelection error for generation mismatch"),
    }
}

#[test]
fn test_validate_editor_command_target_mismatch() {
    let target1 = make_vehicle_target(1);
    let target2 = make_vehicle_target(2);
    let selection = InspectorSelection::new(target2);
    
    let cmd = InspectorCommand::Editor(EditorCommand::StartSandbox { target: target1 });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
}

#[test]
fn test_validate_editor_command_invalidated_selection() {
    let target = make_vehicle_target(1);
    let mut selection = InspectorSelection::new(target.clone());
    selection.invalidate("Entity removed".to_string());
    
    let cmd = InspectorCommand::Editor(EditorCommand::StartSandbox { target });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::StaleSelection(msg)) => {
            assert!(msg.contains("invalidated"));
        }
        _ => panic!("Expected StaleSelection error for invalidated selection"),
    }
}

#[test]
fn test_validate_export_command_target_mismatch() {
    let target1 = make_vehicle_target(1);
    let target2 = make_vehicle_target(2);
    let selection = InspectorSelection::new(target2);
    
    let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
        target: target1,
        destination: "test.glb".to_string(),
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
}

#[test]
fn test_validate_material_command_different_entity_type() {
    let vehicle_target = make_vehicle_target(1);
    let human_target = make_human_target(1, 1);
    let selection = InspectorSelection::new(human_target);
    
    let cmd = InspectorCommand::Material(MaterialCommand::ToggleSandbox {
        target: vehicle_target,
    });

    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
}
