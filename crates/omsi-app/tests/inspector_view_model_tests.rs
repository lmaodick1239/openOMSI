//! Tests for UI-independent inspector view models.

use openomsi_game::inspector::core::*;
use openomsi_game::inspector::view_models::*;

#[test]
fn test_inspector_main_view_from_selection_none() {
    let selection = InspectorSelection::default();
    let view = InspectorMainView::from(&selection);

    assert_eq!(view.selection_status, "No selection");
    assert_eq!(view.entity_type, None);
    assert_eq!(view.entity_identity, None);
    assert!(view.penetration_stack.is_empty());
}

#[test]
fn test_inspector_main_view_from_selection_vehicle() {
    let target = SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    };
    let selection = InspectorSelection::new(target);
    let view = InspectorMainView::from(&selection);

    assert_eq!(view.selection_status, "Selected");
    assert_eq!(view.entity_type, Some("Vehicle".to_string()));
    assert!(view.entity_identity.unwrap().contains("Player Vehicle"));
}

#[test]
fn test_inspector_main_view_from_selection_scenery() {
    let target = SelectionTarget::Scenery {
        key: SceneryKey::Editable { map_id: 42 },
        mesh: None,
    };
    let selection = InspectorSelection::new(target);
    let view = InspectorMainView::from(&selection);

    assert_eq!(view.selection_status, "Selected");
    assert_eq!(view.entity_type, Some("Scenery".to_string()));
    assert!(view.entity_identity.unwrap().contains("Editable #42"));
}

#[test]
fn test_inspector_main_view_from_selection_invalidated() {
    let mut selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });
    selection.invalidate("Entity removed".to_string());
    let view = InspectorMainView::from(&selection);

    assert!(view.selection_status.contains("Invalidated"));
    assert!(view.selection_status.contains("Entity removed"));
}

#[test]
fn test_inspector_main_view_with_penetration_stack() {
    let mut selection = InspectorSelection::default();
    let hits = vec![
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.0m] cockpit".to_string(),
        ),
        PenetrationHit::new(
            2.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[2.0m] body".to_string(),
        ),
    ];
    selection.set_penetration_stack(hits);

    let view = InspectorMainView::from(&selection);
    assert_eq!(view.penetration_stack.len(), 2);
    assert_eq!(view.penetration_stack[0].distance, 1.0);
    assert_eq!(view.penetration_stack[1].distance, 2.0);
    assert_eq!(view.current_hit_index, 0);
}

#[test]
fn test_view_model_serialization_material() {
    let view = MaterialView {
        name: "test_material".to_string(),
        shader_variant: "standard".to_string(),
        alpha_mode: "Opaque".to_string(),
        blend_mode: "None".to_string(),
        base_color: [1.0, 0.5, 0.0, 1.0],
        metallic: 0.8,
        roughness: 0.2,
        base_texture: Some("texture.png".to_string()),
        mipmap_level: 0,
        sandbox_active: false,
    };

    let json = serde_json::to_string(&view).unwrap();
    let deserialized: MaterialView = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "test_material");
    assert_eq!(deserialized.metallic, 0.8);
    assert_eq!(deserialized.roughness, 0.2);
}

#[test]
fn test_view_model_serialization_render() {
    let view = RenderView {
        passes: vec![PassView {
            name: "main_pass".to_string(),
            gpu_time_ms: 5.2,
            draw_calls: 100,
            triangles: 50000,
            children: vec![],
        }],
        total_frame_time_ms: 16.7,
        total_draw_calls: 150,
        total_triangles: 75000,
        isolation_mode: None,
        wireframe_enabled: false,
        collision_hulls_enabled: false,
        show_normals: false,
        show_uv_seams: false,
    };

    let json = serde_json::to_string(&view).unwrap();
    let deserialized: RenderView = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.passes.len(), 1);
    assert_eq!(deserialized.total_frame_time_ms, 16.7);
}

#[test]
fn test_view_model_serialization_human() {
    let view = HumanView {
        id: 42,
        generation: 1,
        is_driver: true,
        position: [100.0, 50.0, 200.0],
        velocity: 1.5,
        current_animation: "walk".to_string(),
        animation_phase: 0.5,
        bone_names: (0..32).map(|index| format!("bone_{index}")).collect(),
        playback: "Playing".to_string(),
    };

    let json = serde_json::to_string(&view).unwrap();
    let deserialized: HumanView = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, 42);
    assert_eq!(deserialized.velocity, 1.5);
}

#[test]
fn test_view_model_serialization_telemetry() {
    let view = TelemetryView {
        timestamp_ms: 1234567890,
        frame_time_ms: 16.7,
        inspector_query_time_ms: 0.5,
        gpu_staging_time_ms: 0.2,
        watch_expressions: vec![WatchExpressionView {
            expression: "L.throttle".to_string(),
            value: "0.75".to_string(),
            label: Some("Throttle".to_string()),
        }],
    };

    let json = serde_json::to_string(&view).unwrap();
    let deserialized: TelemetryView = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.timestamp_ms, 1234567890);
    assert_eq!(deserialized.watch_expressions.len(), 1);
}

#[test]
fn test_view_model_serialization_editor() {
    let view = EditorView {
        sandbox_active: true,
        original_transform: Some(TransformView {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
        }),
        current_transform: Some(TransformView {
            position: [1.0, 2.0, 3.0],
            rotation: [45.0, 0.0, 0.0],
        }),
        entity_key: Some("player_vehicle".to_string()),
        transaction_count: 3,
    };

    let json = serde_json::to_string(&view).unwrap();
    let deserialized: EditorView = serde_json::from_str(&json).unwrap();

    assert!(deserialized.sandbox_active);
    assert_eq!(deserialized.transaction_count, 3);
}

#[test]
fn test_view_model_serialization_export() {
    let view = ExportView {
        status: ExportStatus::Success,
        destination: Some("/exports/test.glb".to_string()),
        error: None,
    };

    let json = serde_json::to_string(&view).unwrap();
    let deserialized: ExportView = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.status, ExportStatus::Success);
    assert!(deserialized.destination.is_some());
}

#[test]
fn test_inspector_main_view_enrichment() {
    let selection = InspectorSelection::new(SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    });

    let snapshot = InspectorSnapshot {
        target: SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        },
        position: Some([100.0, 50.0, 200.0]),
        rotation: Some([0.0, 0.0, 0.707, 0.707]),
        bounds: None,
        model_path: Some("vehicles/bus.cfg".to_string()),
        mesh_name: Some("body".to_string()),
        metadata: vec![
            ("Type".to_string(), "Bus".to_string()),
            ("Length".to_string(), "12m".to_string()),
        ],
    };

    let view = InspectorMainView::from(&selection)
        .with_snapshot(&snapshot)
        .unwrap();

    assert_eq!(view.position, Some([100.0, 50.0, 200.0]));
    assert_eq!(view.model_path, Some("vehicles/bus.cfg".to_string()));
    assert_eq!(view.mesh_name, Some("body".to_string()));
    assert_eq!(view.metadata.len(), 2);
}
