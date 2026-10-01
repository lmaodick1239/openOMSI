//! Inspector-to-Editor workflow bridge.
//!
//! Provides atomic transition from inspector to object editor, non-destructive transform
//! sandbox, and transactional commit/revert workflow. Original values preserved in
//! transaction log.

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Editor bridge state and transition manager.
pub struct EditorBridge {
    /// Active transform sandbox (if any).
    sandbox: Option<TransformSandbox>,
    /// Transaction log for rollback.
    transaction_log: Vec<TransactionEntry>,
}

/// Transform sandbox for non-destructive tweaking.
#[derive(Debug, Clone)]
pub struct TransformSandbox {
    /// Entity key being edited.
    pub entity_key: String,
    /// Original world transform.
    pub original_transform: Transform,
    /// Current modified transform.
    pub current_transform: Transform,
    /// Whether the sandbox is active.
    pub active: bool,
}

/// Transform representation (position + rotation).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Transform {
    /// Position in world coordinates.
    pub position: [f32; 3],
    /// Rotation (yaw, pitch, roll) in degrees.
    pub rotation: [f32; 3],
}

impl Transform {
    /// Create identity transform.
    pub fn identity() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
        }
    }

    /// Create from position and rotation.
    pub fn new(position: [f32; 3], rotation: [f32; 3]) -> Self {
        Self { position, rotation }
    }

    /// Calculate delta from another transform.
    pub fn delta_from(&self, other: &Transform) -> TransformDelta {
        TransformDelta {
            position_delta: [
                self.position[0] - other.position[0],
                self.position[1] - other.position[1],
                self.position[2] - other.position[2],
            ],
            rotation_delta: [
                self.rotation[0] - other.rotation[0],
                self.rotation[1] - other.rotation[1],
                self.rotation[2] - other.rotation[2],
            ],
        }
    }

    /// Convert to glam Mat4.
    pub fn to_matrix(&self) -> Mat4 {
        let pos = Vec3::from_array(self.position);
        let rot = Quat::from_euler(
            glam::EulerRot::YXZ,
            self.rotation[0].to_radians(),
            self.rotation[1].to_radians(),
            self.rotation[2].to_radians(),
        );
        Mat4::from_rotation_translation(rot, pos)
    }
}

/// Transform delta (difference between two transforms).
#[derive(Debug, Clone, Copy)]
pub struct TransformDelta {
    /// Position offset.
    pub position_delta: [f32; 3],
    /// Rotation offset (degrees).
    pub rotation_delta: [f32; 3],
}

/// Transaction log entry for rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransactionEntry {
    entity_key: String,
    original_transform: Transform,
    modified_transform: Transform,
    timestamp: u64,
}

/// Editor transition result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTransition {
    /// Successfully promoted to editor.
    Success,
    /// Entity is not editable (e.g., remote vehicle, non-editable scenery).
    NotEditable,
    /// Editor mode is unavailable.
    EditorUnavailable,
}

impl EditorBridge {
    /// Create a new editor bridge.
    pub fn new() -> Self {
        Self {
            sandbox: None,
            transaction_log: Vec::new(),
        }
    }

    /// Promote selected entity to object editor.
    ///
    /// Returns `EditorTransition::Success` if the entity can be edited,
    /// or an error code if promotion is not possible.
    pub fn promote_to_editor(&mut self, entity_key: String) -> EditorTransition {
        // TODO: Check entity editability
        // - Remote vehicles: NotEditable
        // - Non-editable scenery: NotEditable
        // - Parked vehicles: NotEditable

        log::info!("Promoting entity '{}' to editor", entity_key);

        // For now, assume success
        // TODO: Deactivate inspector mode, activate editor mode, pin gizmo
        EditorTransition::Success
    }

    /// Enter transform sandbox mode.
    pub fn enter_sandbox(&mut self, entity_key: String, original: Transform) {
        log::info!("Entering transform sandbox for '{}'", entity_key);

        self.sandbox = Some(TransformSandbox {
            entity_key,
            original_transform: original,
            current_transform: original,
            active: true,
        });
    }

    /// Update sandbox transform.
    pub fn update_sandbox_transform(&mut self, transform: Transform) -> Result<(), String> {
        match self.sandbox.as_mut() {
            Some(sandbox) if sandbox.active => {
                sandbox.current_transform = transform;
                Ok(())
            }
            _ => Err("No active sandbox".to_string()),
        }
    }

    /// Get current sandbox delta.
    pub fn get_sandbox_delta(&self) -> Option<TransformDelta> {
        self.sandbox
            .as_ref()
            .map(|s| s.current_transform.delta_from(&s.original_transform))
    }

    /// Revert sandbox to original transform.
    pub fn revert_sandbox(&mut self) -> Result<Transform, String> {
        match self.sandbox.as_mut() {
            Some(sandbox) if sandbox.active => {
                log::info!("Reverting sandbox for '{}'", sandbox.entity_key);
                sandbox.current_transform = sandbox.original_transform;
                Ok(sandbox.original_transform)
            }
            _ => Err("No active sandbox".to_string()),
        }
    }

    /// Commit sandbox changes to map.
    pub fn commit_sandbox(&mut self) -> Result<(), String> {
        match self.sandbox.take() {
            Some(sandbox) if sandbox.active => {
                log::info!("Committing sandbox changes for '{}'", sandbox.entity_key);

                // Record transaction for rollback
                let entry = TransactionEntry {
                    entity_key: sandbox.entity_key.clone(),
                    original_transform: sandbox.original_transform,
                    modified_transform: sandbox.current_transform,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };
                self.transaction_log.push(entry);

                // TODO: Push modified coordinates into editor undo/save queue
                // TODO: Write to tile_x_y.map file

                Ok(())
            }
            _ => Err("No active sandbox".to_string()),
        }
    }

    /// Exit sandbox without committing.
    pub fn exit_sandbox(&mut self) {
        if let Some(sandbox) = self.sandbox.take() {
            log::info!("Exiting sandbox for '{}'", sandbox.entity_key);
        }
    }

    /// Check if sandbox is active.
    pub fn is_sandbox_active(&self) -> bool {
        self.sandbox.as_ref().map(|s| s.active).unwrap_or(false)
    }

    /// Get transaction log size.
    pub fn transaction_count(&self) -> usize {
        self.transaction_log.len()
    }

    /// Rollback last transaction.
    pub fn rollback_last(&mut self) -> Result<Transform, String> {
        match self.transaction_log.pop() {
            Some(entry) => {
                log::info!("Rolling back transaction for '{}'", entry.entity_key);
                // TODO: Restore original transform in map
                Ok(entry.original_transform)
            }
            None => Err("No transactions to rollback".to_string()),
        }
    }
}

impl Default for EditorBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_delta() {
        let original = Transform::new([100.0, 50.0, 2.5], [0.0, 0.0, 0.0]);
        let modified = Transform::new([105.0, 50.0, 2.5], [15.0, 0.0, 0.0]);

        let delta = modified.delta_from(&original);
        assert_eq!(delta.position_delta[0], 5.0);
        assert_eq!(delta.rotation_delta[0], 15.0);
    }

    #[test]
    fn test_sandbox_workflow() {
        let mut bridge = EditorBridge::new();

        let original = Transform::new([100.0, 50.0, 2.5], [0.0, 0.0, 0.0]);
        bridge.enter_sandbox("test_entity".to_string(), original);

        assert!(bridge.is_sandbox_active());

        let modified = Transform::new([105.0, 50.0, 2.5], [15.0, 0.0, 0.0]);
        bridge.update_sandbox_transform(modified).unwrap();

        let delta = bridge.get_sandbox_delta().unwrap();
        assert_eq!(delta.position_delta[0], 5.0);

        bridge.revert_sandbox().unwrap();
        let delta = bridge.get_sandbox_delta().unwrap();
        assert_eq!(delta.position_delta[0], 0.0);

        bridge.update_sandbox_transform(modified).unwrap();
        bridge.commit_sandbox().unwrap();

        assert!(!bridge.is_sandbox_active());
        assert_eq!(bridge.transaction_count(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let mut bridge = EditorBridge::new();

        let original = Transform::new([100.0, 50.0, 2.5], [0.0, 0.0, 0.0]);
        bridge.enter_sandbox("test_entity".to_string(), original);

        let modified = Transform::new([105.0, 50.0, 2.5], [15.0, 0.0, 0.0]);
        bridge.update_sandbox_transform(modified).unwrap();
        bridge.commit_sandbox().unwrap();

        let restored = bridge.rollback_last().unwrap();
        assert_eq!(restored.position[0], 100.0);
    }
}
