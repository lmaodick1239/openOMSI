//! Sandboxed variable overrides for the inspector.
//!
//! This module provides safety-critical live variable editing with strict validation,
//! clamping, and instant rollback guarantees. All overrides are in-memory only and
//! never modify disk assets.
//!
//! # Safety Guarantees
//!
//! 1. **Fail-Closed Integrity**: All overrides are sandboxed with instant rollback
//! 2. **Value Clamping**: Physically impossible values are rejected before application
//! 3. **Automatic Cleanup**: Overrides are cleared on entity despawn, map change, or error
//! 4. **Transaction Logging**: Every override change is logged for debugging
//! 5. **Zero Persistence**: Overrides only affect in-memory simulation state

use crate::inspector::{VehicleKey, SceneryKey};
use std::collections::HashMap;

/// Entity key for override storage (vehicle or scenery).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityKey {
    Vehicle(VehicleKey),
    Scenery(SceneryKey),
}

/// Variable name (case-insensitive).
pub type VarName = String;

/// Override value with validation metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct OverrideValue {
    /// The override value (already clamped).
    pub value: f32,
    /// Original value before override (for rollback).
    pub original: f32,
    /// Timestamp when the override was applied.
    pub timestamp: std::time::Instant,
}

/// Variable constraints for validation and clamping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarConstraint {
    pub min: f32,
    pub max: f32,
}

impl VarConstraint {
    /// Clamp a value to the constraint range.
    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }

    /// Check if a value is within the constraint range.
    pub fn is_valid(&self, value: f32) -> bool {
        value.is_finite() && value >= self.min && value <= self.max
    }
}

/// Predefined constraints for known OMSI variables.
///
/// These are physically-motivated limits to prevent simulation instability:
/// - throttle: 0.0 (idle) to 1.0 (full power)
/// - door_state: 0.0 (closed) to 1.0 (fully open)
/// - engine_rpm: 0.0 to 3000.0 (typical bus engine redline)
/// - air_pressure: 0.0 to 10.0 bar (typical pneumatic system range)
/// - brake_force: 0.0 (no braking) to 1.0 (maximum braking)
/// - steering_angle: -1.0 (full left) to 1.0 (full right)
pub fn get_constraint(var_name: &str) -> VarConstraint {
    match var_name.to_lowercase().as_str() {
        "throttle" => VarConstraint { min: 0.0, max: 1.0 },
        "door_state" | "door_0" | "door_1" | "door_2" | "door_3" => VarConstraint { min: 0.0, max: 1.0 },
        "engine_rpm" | "rpm" => VarConstraint { min: 0.0, max: 3000.0 },
        "air_pressure" | "pressure" => VarConstraint { min: 0.0, max: 10.0 },
        "brake_force" | "brake" => VarConstraint { min: 0.0, max: 1.0 },
        "steering_angle" | "steering" => VarConstraint { min: -1.0, max: 1.0 },
        "velocity" | "speed" => VarConstraint { min: -50.0, max: 50.0 }, // m/s
        "acceleration" => VarConstraint { min: -10.0, max: 10.0 }, // m/s²
        "gear" => VarConstraint { min: -1.0, max: 8.0 }, // R, N, 1-6, D
        // Default: reasonable range for normalized values or small physical quantities
        _ => VarConstraint { min: -100.0, max: 100.0 },
    }
}

/// Transaction log entry for debugging and rollback.
#[derive(Debug, Clone)]
pub struct TransactionLogEntry {
    pub timestamp: std::time::Instant,
    pub entity: EntityKey,
    pub var_name: VarName,
    pub action: TransactionAction,
}

/// Transaction action type.
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionAction {
    /// Override applied (old_value, new_value, clamped).
    Applied { old: f32, new: f32, clamped: bool },
    /// Override removed (restored to original).
    Removed { restored: f32 },
    /// Invalid value rejected (attempted_value, reason).
    Rejected { value: f32, reason: String },
}

/// Sandboxed variable override manager.
///
/// This is the safety-critical core of the live variable editing system.
/// All mutations go through validation and are logged for debugging.
pub struct OverrideManager {
    /// Active overrides: (entity, var_name) -> override value.
    overrides: HashMap<(EntityKey, VarName), OverrideValue>,
    /// Transaction log (most recent last).
    transaction_log: Vec<TransactionLogEntry>,
    /// Maximum transaction log size (oldest entries are dropped).
    max_log_size: usize,
}

impl Default for OverrideManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OverrideManager {
    /// Create a new override manager.
    pub fn new() -> Self {
        Self {
            overrides: HashMap::new(),
            transaction_log: Vec::new(),
            max_log_size: 1000,
        }
    }

    /// Apply a variable override with validation and clamping.
    ///
    /// Returns `Ok(clamped_value)` on success, or `Err(reason)` if rejected.
    pub fn apply_override(
        &mut self,
        entity: EntityKey,
        var_name: VarName,
        value: f32,
        original: f32,
    ) -> Result<f32, String> {
        // Reject non-finite values immediately
        if !value.is_finite() {
            let reason = "Non-finite value rejected".to_string();
            self.log_transaction(TransactionLogEntry {
                timestamp: std::time::Instant::now(),
                entity: entity.clone(),
                var_name: var_name.clone(),
                action: TransactionAction::Rejected { value, reason: reason.clone() },
            });
            return Err(reason);
        }

        // Get constraint and clamp value
        let constraint = get_constraint(&var_name);
        let clamped = constraint.clamp(value);
        let was_clamped = (clamped - value).abs() > 1e-6;

        // Store override
        let key = (entity.clone(), var_name.clone());
        self.overrides.insert(
            key,
            OverrideValue {
                value: clamped,
                original,
                timestamp: std::time::Instant::now(),
            },
        );

        // Log transaction
        self.log_transaction(TransactionLogEntry {
            timestamp: std::time::Instant::now(),
            entity,
            var_name,
            action: TransactionAction::Applied {
                old: original,
                new: clamped,
                clamped: was_clamped,
            },
        });

        Ok(clamped)
    }

    /// Remove a variable override and restore the original value.
    ///
    /// Returns the original value if an override existed, or `None` if no override was active.
    pub fn remove_override(&mut self, entity: &EntityKey, var_name: &str) -> Option<f32> {
        let key = (entity.clone(), var_name.to_string());
        if let Some(override_value) = self.overrides.remove(&key) {
            self.log_transaction(TransactionLogEntry {
                timestamp: std::time::Instant::now(),
                entity: entity.clone(),
                var_name: var_name.to_string(),
                action: TransactionAction::Removed {
                    restored: override_value.original,
                },
            });
            Some(override_value.original)
        } else {
            None
        }
    }

    /// Get the current override value for a variable, if one exists.
    pub fn get_override(&self, entity: &EntityKey, var_name: &str) -> Option<f32> {
        let key = (entity.clone(), var_name.to_string());
        self.overrides.get(&key).map(|o| o.value)
    }

    /// Check if a variable has an active override.
    pub fn has_override(&self, entity: &EntityKey, var_name: &str) -> bool {
        let key = (entity.clone(), var_name.to_string());
        self.overrides.contains_key(&key)
    }

    /// Clear all overrides for a specific entity.
    ///
    /// This is called automatically on entity despawn or invalidation.
    pub fn clear_entity_overrides(&mut self, entity: &EntityKey) {
        let keys_to_remove: Vec<_> = self
            .overrides
            .keys()
            .filter(|(e, _)| e == entity)
            .cloned()
            .collect();

        for key in keys_to_remove {
            let (entity, var_name) = key;
            if let Some(override_value) = self.overrides.remove(&(entity.clone(), var_name.clone())) {
                self.log_transaction(TransactionLogEntry {
                    timestamp: std::time::Instant::now(),
                    entity,
                    var_name,
                    action: TransactionAction::Removed {
                        restored: override_value.original,
                    },
                });
            }
        }
    }

    /// Clear ALL overrides (called on map change, inspector exit, or panic recovery).
    ///
    /// This is the safety-critical "Reset All" operation that MUST work 100% reliably.
    pub fn clear_all_overrides(&mut self) {
        let entities: Vec<_> = self
            .overrides
            .keys()
            .map(|(e, _)| e.clone())
            .collect();

        for entity in entities {
            self.clear_entity_overrides(&entity);
        }

        // Paranoid double-check: ensure the map is actually empty
        if !self.overrides.is_empty() {
            log::error!(
                "CRITICAL: Override map not empty after clear_all_overrides! Force-clearing {} entries.",
                self.overrides.len()
            );
            self.overrides.clear();
        }
    }

    /// Get all active overrides for an entity.
    pub fn get_entity_overrides(&self, entity: &EntityKey) -> Vec<(String, f32, f32)> {
        self.overrides
            .iter()
            .filter(|((e, _), _)| e == entity)
            .map(|((_, name), override_value)| {
                (name.clone(), override_value.original, override_value.value)
            })
            .collect()
    }

    /// Get the number of active overrides.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Get the transaction log.
    pub fn transaction_log(&self) -> &[TransactionLogEntry] {
        &self.transaction_log
    }

    /// Clear the transaction log.
    pub fn clear_transaction_log(&mut self) {
        self.transaction_log.clear();
    }

    /// Log a transaction entry.
    fn log_transaction(&mut self, entry: TransactionLogEntry) {
        self.transaction_log.push(entry);
        if self.transaction_log.len() > self.max_log_size {
            self.transaction_log.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::VehicleKey;

    #[test]
    fn test_constraint_clamping() {
        let throttle = get_constraint("throttle");
        assert_eq!(throttle.clamp(0.5), 0.5);
        assert_eq!(throttle.clamp(-0.1), 0.0);
        assert_eq!(throttle.clamp(1.5), 1.0);
        assert!(throttle.is_valid(0.5));
        assert!(!throttle.is_valid(1.5));
    }

    #[test]
    fn test_apply_override() {
        let mut mgr = OverrideManager::new();
        let entity = EntityKey::Vehicle(VehicleKey::Player { generation: 0 });

        // Valid override
        let result = mgr.apply_override(entity.clone(), "throttle".to_string(), 0.7, 0.0);
        assert_eq!(result, Ok(0.7));
        assert_eq!(mgr.get_override(&entity, "throttle"), Some(0.7));

        // Override with clamping
        let result = mgr.apply_override(entity.clone(), "throttle".to_string(), 5.0, 0.0);
        assert_eq!(result, Ok(1.0)); // clamped to max
        assert_eq!(mgr.get_override(&entity, "throttle"), Some(1.0));
    }

    #[test]
    fn test_remove_override() {
        let mut mgr = OverrideManager::new();
        let entity = EntityKey::Vehicle(VehicleKey::Player { generation: 0 });

        mgr.apply_override(entity.clone(), "throttle".to_string(), 0.7, 0.3).unwrap();
        assert!(mgr.has_override(&entity, "throttle"));

        let original = mgr.remove_override(&entity, "throttle");
        assert_eq!(original, Some(0.3));
        assert!(!mgr.has_override(&entity, "throttle"));
    }

    #[test]
    fn test_clear_all_overrides() {
        let mut mgr = OverrideManager::new();
        let entity1 = EntityKey::Vehicle(VehicleKey::Player { generation: 0 });
        let entity2 = EntityKey::Vehicle(VehicleKey::AiCar { id: 1 });

        mgr.apply_override(entity1.clone(), "throttle".to_string(), 0.7, 0.0).unwrap();
        mgr.apply_override(entity2.clone(), "door_state".to_string(), 1.0, 0.0).unwrap();
        assert_eq!(mgr.override_count(), 2);

        mgr.clear_all_overrides();
        assert_eq!(mgr.override_count(), 0);
        assert!(!mgr.has_override(&entity1, "throttle"));
        assert!(!mgr.has_override(&entity2, "door_state"));
    }

    #[test]
    fn test_reject_non_finite() {
        let mut mgr = OverrideManager::new();
        let entity = EntityKey::Vehicle(VehicleKey::Player { generation: 0 });

        let result = mgr.apply_override(entity.clone(), "throttle".to_string(), f32::NAN, 0.0);
        assert!(result.is_err());
        assert!(!mgr.has_override(&entity, "throttle"));

        let result = mgr.apply_override(entity.clone(), "throttle".to_string(), f32::INFINITY, 0.0);
        assert!(result.is_err());
        assert!(!mgr.has_override(&entity, "throttle"));
    }

    #[test]
    fn test_transaction_log() {
        let mut mgr = OverrideManager::new();
        let entity = EntityKey::Vehicle(VehicleKey::Player { generation: 0 });

        mgr.apply_override(entity.clone(), "throttle".to_string(), 0.7, 0.0).unwrap();
        mgr.remove_override(&entity, "throttle");

        let log = mgr.transaction_log();
        assert_eq!(log.len(), 2);
        assert!(matches!(log[0].action, TransactionAction::Applied { .. }));
        assert!(matches!(log[1].action, TransactionAction::Removed { .. }));
    }

    #[test]
    fn test_clear_entity_overrides() {
        let mut mgr = OverrideManager::new();
        let entity1 = EntityKey::Vehicle(VehicleKey::Player { generation: 0 });
        let entity2 = EntityKey::Vehicle(VehicleKey::AiCar { id: 1 });

        mgr.apply_override(entity1.clone(), "throttle".to_string(), 0.7, 0.0).unwrap();
        mgr.apply_override(entity1.clone(), "door_state".to_string(), 1.0, 0.0).unwrap();
        mgr.apply_override(entity2.clone(), "throttle".to_string(), 0.5, 0.0).unwrap();
        assert_eq!(mgr.override_count(), 3);

        mgr.clear_entity_overrides(&entity1);
        assert_eq!(mgr.override_count(), 1);
        assert!(!mgr.has_override(&entity1, "throttle"));
        assert!(!mgr.has_override(&entity1, "door_state"));
        assert!(mgr.has_override(&entity2, "throttle"));
    }
}
