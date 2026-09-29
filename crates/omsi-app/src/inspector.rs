//! Inspector selection model and stable identity.
//!
//! The visual debug inspector allows read-only selection of vehicles and scenery objects
//! without triggering interactive side-effects. This module defines the types for stable
//! entity identity, hit candidates, selection state, and ordering policy.

use std::cmp::Ordering;

/// Stable identity for a vehicle entity across frames.
///
/// Different vehicle domains have different identity stability guarantees:
/// - Player: singleton, but can be replaced; detect via generation counter
/// - AI traffic: stable monotonic ID, but vec index is unstable (swap_remove)
/// - Remote (LAN): player ID, but same ID may be replaced (type change); needs generation
/// - Trailer parts: index-based; invalidate on coupling/length changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VehicleKey {
    /// The player's vehicle. Generation increments on vehicle replacement.
    Player { generation: u64 },
    /// AI traffic car by stable monotonic ID.
    AiCar { id: u64 },
    /// Remote vehicle by player ID. Generation increments on replacement under same ID.
    Remote { player_id: u32, generation: u64 },
    /// Player trailer part by index.
    PlayerTrailer { generation: u64, trailer_index: usize },
    /// AI car trailer part.
    AiTrailer { car_id: u64, trailer_index: usize },
    /// Remote vehicle trailer part.
    RemoteTrailer {
        player_id: u32,
        generation: u64,
        trailer_index: usize,
    },
}

impl PartialOrd for VehicleKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VehicleKey {
    fn cmp(&self, other: &Self) -> Ordering {
        use VehicleKey::*;
        match (self, other) {
            (Player { generation: g1 }, Player { generation: g2 }) => g1.cmp(g2),
            (AiCar { id: id1 }, AiCar { id: id2 }) => id1.cmp(id2),
            (
                Remote {
                    player_id: p1,
                    generation: g1,
                },
                Remote {
                    player_id: p2,
                    generation: g2,
                },
            ) => p1.cmp(p2).then(g1.cmp(g2)),
            (
                PlayerTrailer {
                    generation: g1,
                    trailer_index: t1,
                },
                PlayerTrailer {
                    generation: g2,
                    trailer_index: t2,
                },
            ) => g1.cmp(g2).then(t1.cmp(t2)),
            (
                AiTrailer {
                    car_id: c1,
                    trailer_index: t1,
                },
                AiTrailer {
                    car_id: c2,
                    trailer_index: t2,
                },
            ) => c1.cmp(c2).then(t1.cmp(t2)),
            (
                RemoteTrailer {
                    player_id: p1,
                    generation: g1,
                    trailer_index: t1,
                },
                RemoteTrailer {
                    player_id: p2,
                    generation: g2,
                    trailer_index: t2,
                },
            ) => p1.cmp(p2).then(g1.cmp(g2)).then(t1.cmp(t2)),
            // Cross-variant ordering: Player < AiCar < Remote < PlayerTrailer < AiTrailer < RemoteTrailer
            (Player { .. }, _) => Ordering::Less,
            (_, Player { .. }) => Ordering::Greater,
            (AiCar { .. }, Remote { .. })
            | (AiCar { .. }, PlayerTrailer { .. })
            | (AiCar { .. }, AiTrailer { .. })
            | (AiCar { .. }, RemoteTrailer { .. }) => Ordering::Less,
            (Remote { .. }, AiCar { .. }) => Ordering::Greater,
            (Remote { .. }, PlayerTrailer { .. })
            | (Remote { .. }, AiTrailer { .. })
            | (Remote { .. }, RemoteTrailer { .. }) => Ordering::Less,
            (PlayerTrailer { .. }, AiCar { .. }) | (PlayerTrailer { .. }, Remote { .. }) => {
                Ordering::Greater
            }
            (PlayerTrailer { .. }, AiTrailer { .. })
            | (PlayerTrailer { .. }, RemoteTrailer { .. }) => Ordering::Less,
            (AiTrailer { .. }, RemoteTrailer { .. }) => Ordering::Less,
            (AiTrailer { .. }, _) => Ordering::Greater,
            (RemoteTrailer { .. }, _) => Ordering::Greater,
        }
    }
}

/// Stable identity for a scenery object instance.
///
/// Scenery objects are keyed by map ID for editable objects, or by tile + collision key
/// for non-editable objects. Tile unload or reload invalidates the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SceneryKey {
    /// Editable scenery object by map ID.
    Editable { map_id: i64 },
    /// Non-editable scenery object by tile coordinate and collision key.
    NonEditable { tile_x: i32, tile_y: i32, key: i64 },
    /// Parked object (scenery instance, not runtime vehicle mesh).
    Parked { key: i64 },
}

/// Logical mesh identity within a model.
///
/// A mesh is identified by its source model path, definition index, and a disambiguator
/// for duplicate mesh names. When the selected mesh is absent at the current LOD, the
/// fallback policy selects the closest available mesh by definition index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MeshIdentity {
    /// Model source path (e.g., `.sco` or vehicle `.cfg` relative path).
    pub model_path: String,
    /// Mesh definition index in the model's mesh list.
    pub definition_index: usize,
    /// Disambiguator for duplicate mesh names. `None` if the mesh name is unique.
    pub disambiguator: Option<usize>,
    /// Original mesh name at selection time (for display and validation).
    pub mesh_name: String,
}

impl MeshIdentity {
    /// Create a new mesh identity.
    pub fn new(
        model_path: String,
        definition_index: usize,
        mesh_name: String,
        disambiguator: Option<usize>,
    ) -> Self {
        Self {
            model_path,
            definition_index,
            disambiguator,
            mesh_name,
        }
    }

    /// LOD fallback policy: find the closest mesh by definition index when the precise
    /// mesh is absent. Returns the candidate with the smallest index distance.
    ///
    /// If multiple candidates have equal distance, prefer lower indices (coarser LOD).
    pub fn find_fallback_index(&self, available_indices: &[usize]) -> Option<usize> {
        if available_indices.is_empty() {
            return None;
        }

        let target = self.definition_index;
        let mut best: Option<(usize, usize)> = None; // (index, distance)

        for &idx in available_indices {
            let distance = if idx >= target {
                idx - target
            } else {
                target - idx
            };

            match best {
                None => best = Some((idx, distance)),
                Some((_, best_dist)) if distance < best_dist => best = Some((idx, distance)),
                Some((best_idx, best_dist)) if distance == best_dist && idx < best_idx => {
                    best = Some((idx, distance))
                }
                _ => {}
            }
        }

        best.map(|(idx, _)| idx)
    }
}

/// Selection target: vehicle or scenery.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectionTarget {
    Vehicle {
        key: VehicleKey,
        mesh: Option<MeshIdentity>,
    },
    Scenery {
        key: SceneryKey,
        mesh: Option<MeshIdentity>,
    },
}

/// A raycast hit candidate with distance and stable identity.
///
/// Used for ordering and tie-breaking. Distances must be finite and positive.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectorHit {
    /// Hit distance from ray origin (must be finite and positive).
    pub distance: f32,
    /// Selection target.
    pub target: SelectionTarget,
}

impl InspectorHit {
    /// Create a new hit. Returns `None` if distance is non-finite or non-positive.
    pub fn new(distance: f32, target: SelectionTarget) -> Option<Self> {
        if distance.is_finite() && distance > 0.0 {
            Some(Self { distance, target })
        } else {
            None
        }
    }

    /// Order hits by distance (nearest first), with stable identity tie-breaking.
    ///
    /// Invalid distances (non-finite or non-positive) are rejected at construction.
    /// Equal distances are broken by stable identity ordering (vehicles before scenery,
    /// then by key ordering within each domain).
    pub fn order_candidates(hits: &mut [InspectorHit]) {
        hits.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(Ordering::Equal)
                .then_with(|| Self::stable_identity_order(&a.target, &b.target))
        });
    }

    fn stable_identity_order(a: &SelectionTarget, b: &SelectionTarget) -> Ordering {
        use SelectionTarget::*;
        match (a, b) {
            (Vehicle { key: k1, .. }, Vehicle { key: k2, .. }) => k1.cmp(k2),
            (Scenery { key: k1, .. }, Scenery { key: k2, .. }) => k1.cmp(k2),
            (Vehicle { .. }, Scenery { .. }) => Ordering::Less,
            (Scenery { .. }, Vehicle { .. }) => Ordering::Greater,
        }
    }
}

/// Current inspector selection state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionStatus {
    /// No selection.
    None,
    /// Valid selection.
    Selected(SelectionTarget),
    /// Selection was invalidated (entity removed, tile unloaded, LOD change, etc.).
    /// Briefly shown before clearing.
    Invalidated { reason: String },
}

/// View toggles for inspector panel display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewToggles {
    pub show_bounds: bool,
    pub show_local_axes: bool,
    pub show_mesh_name: bool,
}

/// Current inspector selection with owned state.
///
/// No renderer IDs or borrowed references. All state is owned and serializable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorSelection {
    pub status: SelectionStatus,
    pub view: ViewToggles,
}

impl Default for InspectorSelection {
    fn default() -> Self {
        Self {
            status: SelectionStatus::None,
            view: ViewToggles::default(),
        }
    }
}

impl InspectorSelection {
    /// Create a new selection with the given target.
    pub fn new(target: SelectionTarget) -> Self {
        Self {
            status: SelectionStatus::Selected(target),
            view: ViewToggles::default(),
        }
    }

    /// Mark the selection as invalidated with a reason.
    pub fn invalidate(&mut self, reason: String) {
        if !matches!(self.status, SelectionStatus::None) {
            self.status = SelectionStatus::Invalidated { reason };
        }
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.status = SelectionStatus::None;
    }

    /// Check if a selection is active (not `None` or `Invalidated`).
    pub fn is_active(&self) -> bool {
        matches!(self.status, SelectionStatus::Selected(_))
    }
}

/// Validated snapshot of a selected entity for display.
///
/// Resolves stored handles into owned data. Stale handles show as unavailable, then clear.
/// This type is computed per-frame or on generation change, and contains no locks or
/// borrowed references.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectorSnapshot {
    /// Entity type and identity.
    pub target: SelectionTarget,
    /// World position (if available).
    pub position: Option<[f32; 3]>,
    /// Rotation (if available, as quaternion [x, y, z, w]).
    pub rotation: Option<[f32; 4]>,
    /// Bounding box (if available, as min/max).
    pub bounds: Option<([f32; 3], [f32; 3])>,
    /// Model source path (for display).
    pub model_path: Option<String>,
    /// Selected mesh name (if a mesh is selected).
    pub mesh_name: Option<String>,
    /// Entity-specific metadata (e.g., vehicle type name, scenery object type).
    pub metadata: Vec<(String, String)>,
}

impl InspectorSnapshot {
    /// Create a minimal snapshot with only target identity.
    pub fn minimal(target: SelectionTarget) -> Self {
        Self {
            target,
            position: None,
            rotation: None,
            bounds: None,
            model_path: None,
            mesh_name: None,
            metadata: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vehicle_key_ordering() {
        let player1 = VehicleKey::Player { generation: 1 };
        let player2 = VehicleKey::Player { generation: 2 };
        let ai1 = VehicleKey::AiCar { id: 100 };
        let ai2 = VehicleKey::AiCar { id: 200 };
        let remote1 = VehicleKey::Remote {
            player_id: 1,
            generation: 1,
        };

        assert!(player1 < player2);
        assert!(player1 < ai1);
        assert!(ai1 < ai2);
        assert!(ai1 < remote1);
        assert!(player2 < ai1);
    }

    #[test]
    fn test_scenery_key_ordering() {
        let edit1 = SceneryKey::Editable { map_id: 100 };
        let edit2 = SceneryKey::Editable { map_id: 200 };
        let non_edit = SceneryKey::NonEditable {
            tile_x: 0,
            tile_y: 0,
            key: 50,
        };
        let parked = SceneryKey::Parked { key: 75 };

        assert!(edit1 < edit2);
        assert!(edit1 < non_edit);
        assert!(edit1 < parked);
    }

    #[test]
    fn test_hit_rejects_invalid_distance() {
        let target = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };

        assert!(InspectorHit::new(f32::NAN, target.clone()).is_none());
        assert!(InspectorHit::new(f32::INFINITY, target.clone()).is_none());
        assert!(InspectorHit::new(-1.0, target.clone()).is_none());
        assert!(InspectorHit::new(0.0, target.clone()).is_none());
        assert!(InspectorHit::new(1.0, target).is_some());
    }

    #[test]
    fn test_hit_ordering_by_distance() {
        let target1 = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };
        let target2 = SelectionTarget::Vehicle {
            key: VehicleKey::AiCar { id: 100 },
            mesh: None,
        };

        let mut hits = vec![
            InspectorHit::new(5.0, target1.clone()).unwrap(),
            InspectorHit::new(2.0, target2.clone()).unwrap(),
            InspectorHit::new(10.0, target1.clone()).unwrap(),
        ];

        InspectorHit::order_candidates(&mut hits);

        assert_eq!(hits[0].distance, 2.0);
        assert_eq!(hits[1].distance, 5.0);
        assert_eq!(hits[2].distance, 10.0);
    }

    #[test]
    fn test_hit_ordering_equal_distance_tie_break() {
        let player = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };
        let ai = SelectionTarget::Vehicle {
            key: VehicleKey::AiCar { id: 100 },
            mesh: None,
        };
        let scenery = SelectionTarget::Scenery {
            key: SceneryKey::Editable { map_id: 50 },
            mesh: None,
        };

        let mut hits = vec![
            InspectorHit::new(5.0, scenery.clone()).unwrap(),
            InspectorHit::new(5.0, ai.clone()).unwrap(),
            InspectorHit::new(5.0, player.clone()).unwrap(),
        ];

        InspectorHit::order_candidates(&mut hits);

        // All distances are 5.0. Stable identity order: vehicles before scenery, player before AI.
        match &hits[0].target {
            SelectionTarget::Vehicle { key, .. } => {
                assert!(matches!(key, VehicleKey::Player { .. }))
            }
            _ => panic!("Expected player vehicle first"),
        }
        match &hits[1].target {
            SelectionTarget::Vehicle { key, .. } => assert!(matches!(key, VehicleKey::AiCar { .. })),
            _ => panic!("Expected AI vehicle second"),
        }
        match &hits[2].target {
            SelectionTarget::Scenery { .. } => {}
            _ => panic!("Expected scenery third"),
        }
    }

    #[test]
    fn test_mesh_identity_unique_name() {
        let mesh = MeshIdentity::new(
            "test.sco".to_string(),
            5,
            "door_front".to_string(),
            None,
        );

        assert_eq!(mesh.model_path, "test.sco");
        assert_eq!(mesh.definition_index, 5);
        assert_eq!(mesh.mesh_name, "door_front");
        assert_eq!(mesh.disambiguator, None);
    }

    #[test]
    fn test_mesh_identity_duplicate_name() {
        let mesh = MeshIdentity::new(
            "test.sco".to_string(),
            7,
            "wheel".to_string(),
            Some(2),
        );

        assert_eq!(mesh.disambiguator, Some(2));
    }

    #[test]
    fn test_mesh_lod_fallback_exact_match() {
        let mesh = MeshIdentity::new(
            "test.sco".to_string(),
            5,
            "body".to_string(),
            None,
        );

        let available = vec![3, 5, 8];
        let fallback = mesh.find_fallback_index(&available);

        assert_eq!(fallback, Some(5));
    }

    #[test]
    fn test_mesh_lod_fallback_nearest() {
        let mesh = MeshIdentity::new(
            "test.sco".to_string(),
            6,
            "body".to_string(),
            None,
        );

        // Available: 3, 5, 8. Target: 6. Distances: 3, 1, 2. Nearest: 5.
        let available = vec![3, 5, 8];
        let fallback = mesh.find_fallback_index(&available);

        assert_eq!(fallback, Some(5));
    }

    #[test]
    fn test_mesh_lod_fallback_tie_prefers_lower() {
        let mesh = MeshIdentity::new(
            "test.sco".to_string(),
            5,
            "body".to_string(),
            None,
        );

        // Available: 3, 7. Target: 5. Distances: 2, 2. Tie-break: prefer 3 (lower index).
        let available = vec![3, 7];
        let fallback = mesh.find_fallback_index(&available);

        assert_eq!(fallback, Some(3));
    }

    #[test]
    fn test_mesh_lod_fallback_empty() {
        let mesh = MeshIdentity::new(
            "test.sco".to_string(),
            5,
            "body".to_string(),
            None,
        );

        let fallback = mesh.find_fallback_index(&[]);
        assert_eq!(fallback, None);
    }

    #[test]
    fn test_selection_lifecycle() {
        let target = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };

        let mut sel = InspectorSelection::new(target.clone());
        assert!(sel.is_active());

        sel.invalidate("entity removed".to_string());
        assert!(!sel.is_active());
        assert!(matches!(
            sel.status,
            SelectionStatus::Invalidated { .. }
        ));

        sel.clear();
        assert_eq!(sel.status, SelectionStatus::None);
    }

    #[test]
    fn test_vehicle_generation_replacement() {
        let gen1 = VehicleKey::Player { generation: 1 };
        let gen2 = VehicleKey::Player { generation: 2 };

        assert_ne!(gen1, gen2);
        assert!(gen1 < gen2);
    }

    #[test]
    fn test_remote_vehicle_replacement() {
        let remote_gen1 = VehicleKey::Remote {
            player_id: 42,
            generation: 1,
        };
        let remote_gen2 = VehicleKey::Remote {
            player_id: 42,
            generation: 2,
        };

        assert_ne!(remote_gen1, remote_gen2);
        assert!(remote_gen1 < remote_gen2);
    }
}
