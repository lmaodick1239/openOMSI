//! Tests for UI-independent inspector commands.

use openomsi_game::inspector::core::*;
use openomsi_game::inspector::commands::*;

#[test]
fn test_command_serialization_select() {
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
fn test_command_serialization_material() {
    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        metallic: 0.8,
        roughness: 0.2,
    });

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: InspectorCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        InspectorCommand::Material(MaterialCommand::SetPBROverride {
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
    let cmd = InspectorCommand::Select(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

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
        SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        },
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
        SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        },
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
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "Test1".to_string(),
        ),
        PenetrationHit::new(
            2.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "Test2".to_string(),
        ),
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
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });
    let cmd = InspectorCommand::ToggleView(ViewToggle::ShowBounds);

    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_material_mipmap_level_too_high() {
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

    let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel(20));
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
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

    let cmd = InspectorCommand::Material(MaterialCommand::SetMipmapLevel(5));
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_material_pbr_metallic_out_of_range() {
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
fn test_validate_material_pbr_roughness_out_of_range() {
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        metallic: 0.5,
        roughness: -0.1,
    });
    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
}

#[test]
fn test_validate_material_pbr_valid() {
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

    let cmd = InspectorCommand::Material(MaterialCommand::SetPBROverride {
        metallic: 0.8,
        roughness: 0.2,
    });
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_human_command_wrong_selection_type() {
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

    let cmd = InspectorCommand::Human(HumanCommand::SetPlayback(PlaybackMode::Paused));
    let result = validate_command(&cmd, &selection);
    assert!(result.is_err());
    match result {
        Err(CommandError::NotSupported(_)) => {}
        _ => panic!("Expected NotSupported error"),
    }
}

#[test]
fn test_validate_human_command_correct_selection_type() {
    let selection = InspectorSelection::new(SelectionTarget::Human {
        key: HumanKey {
            id: 1,
            generation: 1,
            is_driver: false,
        },
        mesh_id: None,
    });

    let cmd = InspectorCommand::Human(HumanCommand::SetPlayback(PlaybackMode::Paused));
    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_export_no_selection() {
    let selection = InspectorSelection::default();
    let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
        destination: "/exports/test.glb".to_string(),
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
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });
    let cmd = InspectorCommand::Export(ExportCommand::ExportSelection {
        destination: "/exports/test.glb".to_string(),
    });

    assert!(validate_command(&cmd, &selection).is_ok());
}

#[test]
fn test_validate_render_commands_always_valid() {
    let selection = InspectorSelection::default();

    let cmds = vec![
        InspectorCommand::Render(RenderCommand::ToggleWireframe),
        InspectorCommand::Render(RenderCommand::ToggleCollisionHulls),
        InspectorCommand::Render(RenderCommand::ToggleNormals),
    ];

    for cmd in cmds {
        assert!(validate_command(&cmd, &selection).is_ok());
    }
}

#[test]
fn test_validate_telemetry_commands_always_valid() {
    let selection = InspectorSelection::default();

    let cmds = vec![
        InspectorCommand::Telemetry(TelemetryCommand::AddWatch("L.throttle".to_string())),
        InspectorCommand::Telemetry(TelemetryCommand::RemoveWatch("L.throttle".to_string())),
        InspectorCommand::Telemetry(TelemetryCommand::ClearWatches),
    ];

    for cmd in cmds {
        assert!(validate_command(&cmd, &selection).is_ok());
    }
}

#[test]
fn test_command_error_display() {
    let err = CommandError::StaleSelection("Test".to_string());
    assert!(format!("{}", err).contains("Stale selection"));

    let err = CommandError::NotSupported("Test".to_string());
    assert!(format!("{}", err).contains("Not supported"));

    let err = CommandError::InvalidParameter("Test".to_string());
    assert!(format!("{}", err).contains("Invalid parameter"));

    let err = CommandError::Failed("Test".to_string());
    assert!(format!("{}", err).contains("Failed"));
}
