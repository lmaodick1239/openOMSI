//! Human module with generational handle tracking and raycasting.

pub mod animation;
pub mod snapshot;
pub mod spatial;
pub mod raycast;

// Re-export all types from legacy human.rs
#[path = "../human_legacy.rs"]
mod legacy;

pub use legacy::{
    Activity, Pose, PoseInput, Posed, Rig, Joints, Influence, HumanMesh, HumanType,
    slot_of, run_factor, skin, skin_from, curl_hands, grip_centres, hand_slot,
    slots_from_omsi,
    BONE_OS_L, BONE_OS_R, BONE_US_L, BONE_US_R, BONE_OA_L, BONE_OA_R,
    BONE_UA_L, BONE_UA_R, BONE_HIP, BONE_MAIN, BONE_HEAD, BONE_HAND_L, BONE_HAND_R,
    SLOTS,
};

pub use animation::{AnimationArtifact, AnimationClip, AnimationState, BoneTransform};
pub use snapshot::{BehaviorState, HumanSnapshot, PassengerEconomy};
pub use spatial::{HumanBroadphase, HumanCylinder};
pub use raycast::{HumanRaycastResult, HumanNarrowphase};

use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

/// Generational handle for stable human identity across despawn/respawn cycles.
///
/// The generation counter increments when a human despawns and the ID is reused,
/// preventing stale references from accessing wrong entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HumanKey {
    /// Stable monotonic ID for the human entity.
    pub index: u32,
    /// Generation counter incremented on despawn/respawn.
    pub generation: u32,
}

impl HumanKey {
    /// Create a new human key.
    #[inline]
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Check if this key is still alive in the registry.
    #[inline]
    pub fn is_alive(&self, registry: &HumanRegistry) -> bool {
        registry.validate(*self)
    }
}

/// Human registry tracking live humans with generational validation.
///
/// Provides lock-free validation of human keys and generation tracking.
pub struct HumanRegistry {
    /// Per-index generation counters. Index maps to human ID.
    generations: Vec<AtomicU64>,
    /// Maximum capacity (pre-allocated).
    capacity: usize,
}

impl HumanRegistry {
    /// Create a new registry with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let mut generations = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            generations.push(AtomicU64::new(0));
        }
        
        Self {
            generations,
            capacity,
        }
    }

    /// Allocate a new human key (called on spawn).
    pub fn allocate(&self, index: u32) -> HumanKey {
        let idx = index as usize;
        if idx >= self.capacity {
            return HumanKey::new(index, 0);
        }
        
        let generation = self.generations[idx].load(AtomicOrdering::Acquire) as u32;
        HumanKey::new(index, generation)
    }

    /// Invalidate a human key (called on despawn).
    pub fn invalidate(&self, key: HumanKey) {
        let idx = key.index as usize;
        if idx >= self.capacity {
            return;
        }
        
        // Increment generation to invalidate all existing keys with old generation
        self.generations[idx].fetch_add(1, AtomicOrdering::Release);
    }

    /// Validate that a key is still alive.
    #[inline]
    pub fn validate(&self, key: HumanKey) -> bool {
        let idx = key.index as usize;
        if idx >= self.capacity {
            return false;
        }
        
        let current_gen = self.generations[idx].load(AtomicOrdering::Acquire) as u32;
        current_gen == key.generation
    }

    /// Get current generation for an index (for debugging).
    pub fn get_generation(&self, index: u32) -> u32 {
        let idx = index as usize;
        if idx >= self.capacity {
            return 0;
        }
        self.generations[idx].load(AtomicOrdering::Acquire) as u32
    }
}

impl Default for HumanRegistry {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generational_validation() {
        let registry = HumanRegistry::new(128);
        
        let key1 = registry.allocate(0);
        assert!(registry.validate(key1));
        
        registry.invalidate(key1);
        assert!(!registry.validate(key1));
        
        let key2 = registry.allocate(0);
        assert!(registry.validate(key2));
        assert!(!registry.validate(key1));
        assert_ne!(key1.generation, key2.generation);
    }

    #[test]
    fn test_multiple_humans() {
        let registry = HumanRegistry::new(128);
        
        let key1 = registry.allocate(0);
        let key2 = registry.allocate(1);
        let key3 = registry.allocate(2);
        
        assert!(registry.validate(key1));
        assert!(registry.validate(key2));
        assert!(registry.validate(key3));
        
        registry.invalidate(key2);
        
        assert!(registry.validate(key1));
        assert!(!registry.validate(key2));
        assert!(registry.validate(key3));
    }
}
