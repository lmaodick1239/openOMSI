//! Integration tests for inspector export functionality (Phase 5).

use glam::Mat4;

// Re-export types from inspector modules for testing
mod test_helpers {
    use super::*;
    
    pub fn create_test_mesh() -> openomsi_game::inspector::export::MeshData {
        openomsi_game::inspector::export::MeshData {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: Some(vec![
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ]),
            tex_coords: Some(vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [0.5, 1.0],
            ]),
            colors: None,
            indices: vec![0, 1, 2],
        }
    }
    
    pub fn create_test_material() -> openomsi_game::inspector::export::MaterialData {
        openomsi_game::inspector::export::MaterialData {
            name: "test_material".to_string(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.8,
            base_texture: None,
            alpha_mode: openomsi_game::inspector::export::AlphaMode::Opaque,
        }
    }
}

#[test]
fn test_persistence_roundtrip() {
    use openomsi_game::inspector::persistence::*;
    
    let layout = InspectorLayout {
        panel_geometry: PanelGeometry {
            x: 150,
            y: 200,
            width: 480,
            collapsed_sections: 0b1010,
        },
        active_tab: InspectorTab::Material,
        watch_table: vec![
            WatchExpression {
                expression: "L.velocity".to_string(),
                label: Some("Speed".to_string()),
                show_sparkline: true,
            },
        ],
        filter_query: "mesh".to_string(),
        version: 1,
    };
    
    let json = serde_json::to_string(&layout).unwrap();
    let decoded: InspectorLayout = serde_json::from_str(&json).unwrap();
    
    assert_eq!(decoded.panel_geometry.x, 150);
    assert_eq!(decoded.active_tab, InspectorTab::Material);
    assert_eq!(decoded.watch_table.len(), 1);
}

#[test]
fn test_export_directory_creation() {
    use openomsi_game::inspector::export;
    
    // Should not panic, may return error on restrictive systems
    let _ = export::export_dir();
}

#[test]
fn test_gltf_export_validation() {
    use openomsi_game::inspector::export;
    
    let mesh = test_helpers::create_test_mesh();
    let material = test_helpers::create_test_material();
    
    let result = export::export_to_gltf(
        "test_mesh",
        Mat4::IDENTITY,
        &mesh,
        &material,
    );
    
    // Should succeed with valid mesh data
    assert!(result.is_ok(), "Valid mesh should export successfully");
    
    // Test with empty mesh (should fail)
    let empty_mesh = export::MeshData {
        positions: vec![],
        normals: None,
        tex_coords: None,
        colors: None,
        indices: vec![],
    };
    
    let result = export::export_to_gltf(
        "empty",
        Mat4::IDENTITY,
        &empty_mesh,
        &material,
    );
    
    assert!(result.is_err(), "Empty mesh should fail validation");
}

#[test]
fn test_telemetry_snapshot_serialization() {
    use openomsi_game::inspector::telemetry::*;
    
    let snapshot = InspectorSnapshot {
        timestamp_ms: 1234567890,
        vehicle: Some(VehicleTelemetry {
            velocity_ms: 12.5,
            engine_rpm: 1800.0,
            gear: 2,
            door_states: vec![1, 0, 1, 0],
            throttle: 0.5,
            brake: 0.1,
        }),
        selection: Some(SelectionTelemetry {
            name: "wheel.o3d".to_string(),
            position: [10.0, 5.0, 1.2],
            rotation: [45.0, 0.0, 0.0],
        }),
        watch_values: vec![
            WatchValue {
                expression: "L.rpm".to_string(),
                value: 1800.0,
                label: Some("RPM".to_string()),
            },
        ],
    };
    
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(json.contains("velocity_ms"));
    assert!(json.contains("wheel.o3d"));
    
    let decoded: InspectorSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.timestamp_ms, 1234567890);
    assert!(decoded.vehicle.is_some());
    assert_eq!(decoded.vehicle.unwrap().gear, 2);
}

#[test]
fn test_editor_bridge_sandbox_workflow() {
    use openomsi_game::inspector::editor_bridge::*;
    
    let mut bridge = EditorBridge::new();
    
    let original = Transform::new([100.0, 50.0, 2.5], [0.0, 0.0, 0.0]);
    bridge.enter_sandbox("test_entity".to_string(), original);
    
    assert!(bridge.is_sandbox_active());
    
    let modified = Transform::new([105.0, 52.0, 2.5], [15.0, 5.0, 0.0]);
    bridge.update_sandbox_transform(modified).unwrap();
    
    let delta = bridge.get_sandbox_delta().unwrap();
    assert_eq!(delta.position_delta[0], 5.0);
    assert_eq!(delta.position_delta[1], 2.0);
    assert_eq!(delta.rotation_delta[0], 15.0);
    
    // Test revert
    bridge.revert_sandbox().unwrap();
    let delta = bridge.get_sandbox_delta().unwrap();
    assert_eq!(delta.position_delta[0], 0.0);
    
    // Test commit
    bridge.update_sandbox_transform(modified).unwrap();
    bridge.commit_sandbox().unwrap();
    
    assert!(!bridge.is_sandbox_active());
    assert_eq!(bridge.transaction_count(), 1);
    
    // Test rollback
    let restored = bridge.rollback_last().unwrap();
    assert_eq!(restored.position[0], 100.0);
}

#[test]
fn test_transform_matrix_conversion() {
    use openomsi_game::inspector::editor_bridge::Transform;
    
    let transform = Transform::new([1.0, 2.0, 3.0], [0.0, 0.0, 0.0]);
    let matrix = transform.to_matrix();
    
    // Verify translation component
    let translation = matrix.w_axis;
    assert!((translation.x - 1.0).abs() < 0.001);
    assert!((translation.y - 2.0).abs() < 0.001);
    assert!((translation.z - 3.0).abs() < 0.001);
}

#[test]
fn test_gltf_bounds_calculation() {
    use omsi_model::gltf_export::calculate_bounds;
    
    let positions = vec![
        [-1.0, -2.0, -3.0],
        [0.0, 0.0, 0.0],
        [2.0, 4.0, 6.0],
    ];
    
    let (min, max) = calculate_bounds(&positions);
    assert_eq!(min, vec![-1.0, -2.0, -3.0]);
    assert_eq!(max, vec![2.0, 4.0, 6.0]);
}

#[test]
fn test_gltf_buffer_encoding() {
    use omsi_model::gltf_export::{encode_floats, encode_u32s};
    
    let floats = vec![1.0, 2.0, 3.0];
    let encoded = encode_floats(&floats);
    assert_eq!(encoded.len(), 12); // 3 floats * 4 bytes
    
    let indices = vec![0u32, 1u32, 2u32];
    let encoded = encode_u32s(&indices);
    assert_eq!(encoded.len(), 12); // 3 u32s * 4 bytes
}

#[test]
fn test_telemetry_server_creation() {
    use openomsi_game::inspector::telemetry::TelemetryServer;
    
    // Test server creation (stub implementation)
    let server = TelemetryServer::start(9002);
    assert!(server.is_ok(), "Server should start without error");
    
    if let Ok(server) = server {
        assert!(server.is_running());
        assert_eq!(server.client_count(), 0);
    }
}

#[test]
fn test_editor_promotion() {
    use openomsi_game::inspector::editor_bridge::*;
    
    let mut bridge = EditorBridge::new();
    let result = bridge.promote_to_editor("scenery_object_123".to_string());
    
    // Should return success for now (stub implementation)
    assert_eq!(result, EditorTransition::Success);
}
