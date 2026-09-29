//! Inspector selection model and stable identity.
//!
//! The visual debug inspector allows read-only selection of vehicles and scenery objects
//! without triggering interactive side-effects. This module defines the types for stable
//! entity identity, hit candidates, selection state, and ordering policy.

use std::cmp::Ordering;
use glam::{DVec3, Vec3};

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

/// A single vehicle mesh raycast hit candidate.
///
/// This is the inspector-only side-effect-free mesh intersection result. It includes
/// the mesh index, hit distance, and optional mesh identity for building an [`InspectorHit`].
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleMeshHit {
    /// Mesh index in the vehicle's mesh list.
    pub mesh_index: usize,
    /// Hit distance from ray origin.
    pub distance: f32,
    /// Definition index for mesh identity lookup.
    pub def_index: usize,
}

/// Read-only vehicle mesh raycast helper.
///
/// Returns all nearest hit candidates without triggering `[mouseevent]` bindings or
/// mutating vehicle state. Reuses the existing vehicle mesh visibility state,
/// `mesh_local_transform`, broadphase check, and `ray_mesh` triangle intersection.
///
/// This helper is suitable for inspector selection across all vehicle domains:
/// player, AI, remote, and trailer parts.
///
/// # Parameters
/// - `vehicle`: The vehicle instance to raycast against
/// - `origin`: Ray origin in world space
/// - `dir`: Ray direction (normalized)
/// - `include_all_visible`: If true, includes all visible meshes; if false, only switch meshes
/// - `inside`: Whether the camera is inside the vehicle (for viewpoint-specific filtering)
///
/// # Returns
/// All hit candidates sorted nearest-first, with world-space distances comparable across
/// vehicle sections.
///
/// # Viewpoint Filtering
/// Respects mesh `[viewpoint]` bits: 1=exterior, 2=interior, 4=AI vehicles, 0=always.
/// Interior cameras only hit meshes visible from inside (bit 2 set or 0).
/// Exterior cameras only hit meshes visible from outside (bit 1 set or 0).
pub fn raycast_vehicle_meshes(
    vehicle: &omsi_sim::VehicleInstance,
    origin: DVec3,
    dir: Vec3,
    include_all_visible: bool,
    inside: bool,
) -> Vec<VehicleMeshHit> {
    let o = (origin - vehicle.position).as_vec3();
    let mut hits = Vec::new();

    for (i, vm) in vehicle.ty.meshes.iter().enumerate() {
        let props = &vehicle.mesh_props[i];
        if !props.visible {
            continue;
        }

        // Viewpoint filtering: respect [viewpoint] bits
        let def = &vehicle.ty.model.meshes[vm.def_index];
        let vp = def.viewpoint;
        let vp_ok = vp == 0 || (inside && vp & 2 != 0) || (!inside && vp & 1 != 0);
        if !vp_ok {
            continue;
        }

        // Filter: switch meshes or all visible meshes
        let is_switch = def.mouse_event.is_some();
        if !include_all_visible && !is_switch {
            continue;
        }

        // Broadphase: sphere check using existing bounds
        let xf = vehicle.mesh_local_transform(i);
        if !crate::camera_util::ray_may_hit(&vehicle.ty, i, &xf, o, dir, 0.0) {
            continue;
        }

        // Narrowphase: triangle intersection
        if let Some(t) = omsi_geometry::ray_mesh(o, dir, &vm.data, &xf) {
            hits.push(VehicleMeshHit {
                mesh_index: i,
                distance: t,
                def_index: vm.def_index,
            });
        }
    }

    hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
    hits
}

/// Raycast against vehicle trailer parts.
///
/// Returns hit candidates with trailer index, mesh index, and world-space distance
/// comparable to lead vehicle hits.
pub fn raycast_vehicle_trailers(
    vehicle: &omsi_sim::VehicleInstance,
    origin: DVec3,
    dir: Vec3,
    include_all_visible: bool,
    inside: bool,
) -> Vec<(usize, VehicleMeshHit)> {
    let mut hits = Vec::new();

    for (ti, trailer) in vehicle.trailers.iter().enumerate() {
        let o = (origin - trailer.position).as_vec3();

        for (i, vm) in trailer.ty.meshes.iter().enumerate() {
            let props = &trailer.mesh_props[i];
            if !props.visible {
                continue;
            }

            // Viewpoint filtering: respect [viewpoint] bits
            let def = &trailer.ty.model.meshes[vm.def_index];
            let vp = def.viewpoint;
            let vp_ok = vp == 0 || (inside && vp & 2 != 0) || (!inside && vp & 1 != 0);
            if !vp_ok {
                continue;
            }

            // Filter: switch meshes or all visible meshes
            let is_switch = def.mouse_event.is_some();
            if !include_all_visible && !is_switch {
                continue;
            }

            // Broadphase: sphere check using existing bounds
            let xf = trailer.mesh_local_transform(i);
            if !crate::camera_util::ray_may_hit(&trailer.ty, i, &xf, o, dir, 0.0) {
                continue;
            }

            // Narrowphase: triangle intersection
            if let Some(t) = omsi_geometry::ray_mesh(o, dir, &vm.data, &xf) {
                hits.push((
                    ti,
                    VehicleMeshHit {
                        mesh_index: i,
                        distance: t,
                        def_index: vm.def_index,
                    },
                ));
            }
        }
    }

    hits.sort_by(|a, b| a.1.distance.partial_cmp(&b.1.distance).unwrap_or(Ordering::Equal));
    hits
}

/// Compute mesh disambiguator: occurrence index for duplicate mesh names.
///
/// Returns `None` if the mesh name is unique, or `Some(occurrence_index)` where 0 is the
/// first occurrence, 1 is the second, etc.
fn compute_mesh_disambiguator(model_meshes: &[omsi_model::MeshDef], def_index: usize) -> Option<usize> {
    let target_name = &model_meshes[def_index].file;
    let mut occurrence = 0;
    let mut count = 0;

    for (i, mesh) in model_meshes.iter().enumerate() {
        if mesh.file == *target_name {
            if i == def_index {
                occurrence = count;
            }
            count += 1;
        }
    }

    // Only return disambiguator if there are duplicates
    if count > 1 {
        Some(occurrence)
    } else {
        None
    }
}

/// Inspector-mode raycast for the player's vehicle: all visible meshes, lead and trailers combined.
///
/// Returns hits sorted nearest-first, with lead vehicle hits and trailer hits in the same list
/// for proper cross-section distance comparison.
///
/// # Parameters
/// - `vehicle`: The player's vehicle instance
/// - `generation`: Player vehicle generation counter for stable identity
/// - `origin`: Ray origin in world space
/// - `dir`: Ray direction (normalized)
/// - `inside`: Whether the camera is inside the vehicle (for viewpoint filtering)
pub fn raycast_player_vehicle(
    vehicle: &omsi_sim::VehicleInstance,
    generation: u64,
    origin: DVec3,
    dir: Vec3,
    inside: bool,
) -> Vec<InspectorHit> {
    let mut hits = Vec::new();

    // Lead vehicle hits
    for hit in raycast_vehicle_meshes(vehicle, origin, dir, true, inside) {
        let mesh_name = vehicle.ty.model.meshes[hit.def_index].file.clone();
        let disambiguator = compute_mesh_disambiguator(&vehicle.ty.model.meshes, hit.def_index);
        let mesh_identity = MeshIdentity::new(
            vehicle.ty.model_dir.to_string_lossy().to_string(),
            hit.def_index,
            mesh_name,
            disambiguator,
        );

        if let Some(inspector_hit) = InspectorHit::new(
            hit.distance,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation },
                mesh: Some(mesh_identity),
            },
        ) {
            hits.push(inspector_hit);
        }
    }

    // Trailer hits
    for (ti, hit) in raycast_vehicle_trailers(vehicle, origin, dir, true, inside) {
        let trailer = &vehicle.trailers[ti];
        let mesh_name = trailer.ty.model.meshes[hit.def_index].file.clone();
        let disambiguator = compute_mesh_disambiguator(&trailer.ty.model.meshes, hit.def_index);
        let mesh_identity = MeshIdentity::new(
            trailer.ty.model_dir.to_string_lossy().to_string(),
            hit.def_index,
            mesh_name,
            disambiguator,
        );

        if let Some(inspector_hit) = InspectorHit::new(
            hit.distance,
            SelectionTarget::Vehicle {
                key: VehicleKey::PlayerTrailer {
                    generation,
                    trailer_index: ti,
                },
                mesh: Some(mesh_identity),
            },
        ) {
            hits.push(inspector_hit);
        }
    }

    InspectorHit::order_candidates(&mut hits);
    hits
}

/// Inspector-mode raycast for an AI vehicle: all visible meshes, lead and trailers combined.
///
/// # Parameters
/// - `vehicle`: The AI vehicle instance
/// - `car_id`: Stable monotonic ID for the AI car
/// - `origin`: Ray origin in world space
/// - `dir`: Ray direction (normalized)
/// - `inside`: Whether the camera is inside the vehicle (for viewpoint filtering)
pub fn raycast_ai_vehicle(
    vehicle: &omsi_sim::VehicleInstance,
    car_id: u64,
    origin: DVec3,
    dir: Vec3,
    inside: bool,
) -> Vec<InspectorHit> {
    let mut hits = Vec::new();

    // Lead vehicle hits
    for hit in raycast_vehicle_meshes(vehicle, origin, dir, true, inside) {
        let mesh_name = vehicle.ty.model.meshes[hit.def_index].file.clone();
        let disambiguator = compute_mesh_disambiguator(&vehicle.ty.model.meshes, hit.def_index);
        let mesh_identity = MeshIdentity::new(
            vehicle.ty.model_dir.to_string_lossy().to_string(),
            hit.def_index,
            mesh_name,
            disambiguator,
        );

        if let Some(inspector_hit) = InspectorHit::new(
            hit.distance,
            SelectionTarget::Vehicle {
                key: VehicleKey::AiCar { id: car_id },
                mesh: Some(mesh_identity),
            },
        ) {
            hits.push(inspector_hit);
        }
    }

    // Trailer hits
    for (ti, hit) in raycast_vehicle_trailers(vehicle, origin, dir, true, inside) {
        let trailer = &vehicle.trailers[ti];
        let mesh_name = trailer.ty.model.meshes[hit.def_index].file.clone();
        let disambiguator = compute_mesh_disambiguator(&trailer.ty.model.meshes, hit.def_index);
        let mesh_identity = MeshIdentity::new(
            trailer.ty.model_dir.to_string_lossy().to_string(),
            hit.def_index,
            mesh_name,
            disambiguator,
        );

        if let Some(inspector_hit) = InspectorHit::new(
            hit.distance,
            SelectionTarget::Vehicle {
                key: VehicleKey::AiTrailer {
                    car_id,
                    trailer_index: ti,
                },
                mesh: Some(mesh_identity),
            },
        ) {
            hits.push(inspector_hit);
        }
    }

    InspectorHit::order_candidates(&mut hits);
    hits
}

/// Inspector-mode raycast for a remote (LAN) vehicle: all visible meshes, lead and trailers combined.
///
/// # Parameters
/// - `vehicle`: The remote vehicle instance
/// - `player_id`: Remote player's network ID
/// - `generation`: Generation counter for this remote player's vehicle
/// - `origin`: Ray origin in world space
/// - `dir`: Ray direction (normalized)
/// - `inside`: Whether the camera is inside the vehicle (for viewpoint filtering)
pub fn raycast_remote_vehicle(
    vehicle: &omsi_sim::VehicleInstance,
    player_id: u32,
    generation: u64,
    origin: DVec3,
    dir: Vec3,
    inside: bool,
) -> Vec<InspectorHit> {
    let mut hits = Vec::new();

    // Lead vehicle hits
    for hit in raycast_vehicle_meshes(vehicle, origin, dir, true, inside) {
        let mesh_name = vehicle.ty.model.meshes[hit.def_index].file.clone();
        let disambiguator = compute_mesh_disambiguator(&vehicle.ty.model.meshes, hit.def_index);
        let mesh_identity = MeshIdentity::new(
            vehicle.ty.model_dir.to_string_lossy().to_string(),
            hit.def_index,
            mesh_name,
            disambiguator,
        );

        if let Some(inspector_hit) = InspectorHit::new(
            hit.distance,
            SelectionTarget::Vehicle {
                key: VehicleKey::Remote {
                    player_id,
                    generation,
                },
                mesh: Some(mesh_identity),
            },
        ) {
            hits.push(inspector_hit);
        }
    }

    // Trailer hits
    for (ti, hit) in raycast_vehicle_trailers(vehicle, origin, dir, true, inside) {
        let trailer = &vehicle.trailers[ti];
        let mesh_name = trailer.ty.model.meshes[hit.def_index].file.clone();
        let disambiguator = compute_mesh_disambiguator(&trailer.ty.model.meshes, hit.def_index);
        let mesh_identity = MeshIdentity::new(
            trailer.ty.model_dir.to_string_lossy().to_string(),
            hit.def_index,
            mesh_name,
            disambiguator,
        );

        if let Some(inspector_hit) = InspectorHit::new(
            hit.distance,
            SelectionTarget::Vehicle {
                key: VehicleKey::RemoteTrailer {
                    player_id,
                    generation,
                    trailer_index: ti,
                },
                mesh: Some(mesh_identity),
            },
        ) {
            hits.push(inspector_hit);
        }
    }

    InspectorHit::order_candidates(&mut hits);
    hits
}

/// A single scenery object mesh raycast hit candidate.
///
/// This is the inspector-only side-effect-free mesh intersection result for scenery objects.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneryMeshHit {
    /// Mesh index in the object type's mesh list.
    pub mesh_index: usize,
    /// Hit distance from ray origin.
    pub distance: f32,
    /// Definition index for mesh identity lookup.
    pub def_index: usize,
}

/// Read-only scenery object mesh raycast helper.
///
/// Returns all nearest hit candidates for a single scenery object placement without triggering
/// any side effects. Reuses existing geometry infrastructure: [`omsi_geometry::ray_mesh`] for
/// triangle intersection with mesh bounds checking.
///
/// This helper is suitable for inspector selection across all scenery domains:
/// editable objects, non-editable placed objects, and parked vehicles (as scenery).
///
/// # Parameters
/// - `object_type`: The object type to raycast against
/// - `pos`: Object position in world space
/// - `xf`: Object transform matrix
/// - `origin`: Ray origin in world space
/// - `dir`: Ray direction (normalized)
///
/// # Returns
/// All hit candidates sorted nearest-first with world-space distances.
///
/// # LOD Handling
/// Only the currently visible LOD level meshes are tested. The caller must determine the
/// appropriate LOD level before calling this function based on camera distance and LOD thresholds.
pub fn raycast_scenery_object_meshes(
    object_type: &crate::scene::ObjectType,
    pos: DVec3,
    xf: &glam::Mat4,
    origin: DVec3,
    dir: Vec3,
    lod_level: usize,
) -> Vec<SceneryMeshHit> {
    let o = (origin - pos).as_vec3();
    let mut hits = Vec::new();

    // Determine which mesh list to use based on LOD level
    let meshes = if lod_level == 0 {
        &object_type.meshes
    } else if let Some((_, lod_meshes)) = object_type.lower_lods.get(lod_level - 1) {
        lod_meshes
    } else {
        return hits; // LOD level out of range
    };

    for (i, (mesh, _materials, _)) in meshes.iter().enumerate() {
        // Narrowphase: triangle intersection
        // MeshData IS the mesh data itself, no .data field needed
        if let Some(t) = omsi_geometry::ray_mesh(o, dir, mesh, xf) {
            hits.push(SceneryMeshHit {
                mesh_index: i,
                distance: t,
                def_index: i, // For scenery, mesh_index == def_index
            });
        }
    }

    hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
    hits
}

/// Simple axis-aligned bounding box ray intersection check.
///
/// Returns entry and exit parameters (t0, t1) if the ray intersects the box.
fn slab_check(o: Vec3, d: Vec3, lo: Vec3, hi: Vec3) -> Option<(f32, f32)> {
    let (mut t0, mut t1) = (f32::MIN, f32::MAX);
    for k in 0..3 {
        if d[k].abs() < 1e-8 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

/// Inspector-mode raycast for a tile's scenery objects.
///
/// Returns hits for all non-editable scenery objects in a tile, sorted nearest-first.
///
/// # Parameters
/// - `tile_x`: Tile X coordinate
/// - `tile_y`: Tile Y coordinate
/// - `scenery_objects`: List of scenery object records
/// - `origin`: Ray origin in world space
/// - `dir`: Ray direction (normalized)
/// - `camera_pos`: Camera position for LOD determination
///
/// # Returns
/// All hit candidates with proper `SceneryKey::NonEditable` identity.
pub fn raycast_tile_scenery(
    tile_x: i32,
    tile_y: i32,
    scenery_objects: &[crate::scene::SceneryObjectRecord],
    origin: DVec3,
    dir: Vec3,
    camera_pos: DVec3,
) -> Vec<InspectorHit> {
    let mut hits = Vec::new();

    for scenery in scenery_objects {
        // Determine LOD level based on camera distance
        let distance = (camera_pos - scenery.pos).length();
        let lod_level = determine_lod_level(&scenery.ty, distance);

        // Raycast against this scenery object
        for hit in raycast_scenery_object_meshes(&scenery.ty, scenery.pos, &scenery.xf, origin, dir, lod_level) {
            // Get mesh name for identity from model definition
            let mesh_def_idx = if lod_level == 0 {
                scenery.ty.mesh_def_index.get(hit.mesh_index).copied()
            } else {
                // For lower LODs, we don't have a direct def_index mapping, use hit index
                Some(hit.def_index)
            };

            let mesh_name = mesh_def_idx
                .and_then(|idx| scenery.ty.model.meshes.get(idx))
                .map(|def| def.file.clone())
                .unwrap_or_default();

            let mesh_identity = MeshIdentity::new(
                scenery.ty.sco.path.to_string_lossy().to_string(),
                mesh_def_idx.unwrap_or(hit.def_index),
                mesh_name,
                None, // Scenery objects typically don't have duplicate mesh names
            );

            if let Some(inspector_hit) = InspectorHit::new(
                hit.distance,
                SelectionTarget::Scenery {
                    key: SceneryKey::NonEditable {
                        tile_x,
                        tile_y,
                        key: scenery.key,
                    },
                    mesh: Some(mesh_identity),
                },
            ) {
                hits.push(inspector_hit);
            }
        }
    }

    InspectorHit::order_candidates(&mut hits);
    hits
}

/// Determine the appropriate LOD level for a scenery object based on camera distance.
///
/// Returns 0 for the base LOD, or the index of the appropriate lower LOD (1-based).
fn determine_lod_level(_object_type: &crate::scene::ObjectType, _distance: f64) -> usize {
    // Simplified LOD determination: always use LOD 0 for now
    // Full implementation would require screen-space size calculation
    // based on camera distance, object bounds, and viewport dimensions
    0
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

    // Tests for vehicle mesh raycast helpers
    // Note: These are unit tests for the helper structure, not integration tests.
    // Integration tests would require a full VehicleInstance which needs map/content loading.

    #[test]
    fn test_vehicle_mesh_hit_ordering() {
        let hit1 = VehicleMeshHit {
            mesh_index: 0,
            distance: 5.0,
            def_index: 0,
        };
        let hit2 = VehicleMeshHit {
            mesh_index: 1,
            distance: 2.0,
            def_index: 1,
        };
        let hit3 = VehicleMeshHit {
            mesh_index: 2,
            distance: 10.0,
            def_index: 2,
        };

        let mut hits = vec![hit1, hit2.clone(), hit3];
        hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));

        assert_eq!(hits[0].distance, 2.0);
        assert_eq!(hits[0].mesh_index, 1);
        assert_eq!(hits[1].distance, 5.0);
        assert_eq!(hits[2].distance, 10.0);
    }

    #[test]
    fn test_raycast_helpers_are_side_effect_free() {
        // This test verifies the type signatures ensure side-effect-free operation:
        // 1. Both helpers take &VehicleInstance (immutable borrow)
        // 2. They return owned data (Vec<VehicleMeshHit>), not references
        // 3. No vehicle state mutation is possible through the API

        // The actual raycast logic is tested through integration tests with real vehicles,
        // but this test documents the contract: read-only, no side effects.
        
        // Type check: these functions exist and have the correct immutable signatures
        let _: fn(&omsi_sim::VehicleInstance, DVec3, Vec3, bool, bool) -> Vec<VehicleMeshHit> =
            raycast_vehicle_meshes;
        let _: fn(&omsi_sim::VehicleInstance, DVec3, Vec3, bool, bool) -> Vec<(usize, VehicleMeshHit)> =
            raycast_vehicle_trailers;
    }

    // Tests for scenery raycast helpers

    #[test]
    fn test_scenery_mesh_hit_ordering() {
        let hit1 = SceneryMeshHit {
            mesh_index: 0,
            distance: 5.0,
            def_index: 0,
        };
        let hit2 = SceneryMeshHit {
            mesh_index: 1,
            distance: 2.0,
            def_index: 1,
        };
        let hit3 = SceneryMeshHit {
            mesh_index: 2,
            distance: 10.0,
            def_index: 2,
        };

        let mut hits = vec![hit1, hit2.clone(), hit3];
        hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));

        assert_eq!(hits[0].distance, 2.0);
        assert_eq!(hits[0].mesh_index, 1);
        assert_eq!(hits[1].distance, 5.0);
        assert_eq!(hits[2].distance, 10.0);
    }

    #[test]
    fn test_scenery_key_distinguishes_placements() {
        // Two placements of the same asset must have different SceneryKey values
        let placement1 = SceneryKey::NonEditable {
            tile_x: 0,
            tile_y: 0,
            key: 100,
        };
        let placement2 = SceneryKey::NonEditable {
            tile_x: 0,
            tile_y: 0,
            key: 101,
        };

        // Same tile, different collision keys
        assert_ne!(placement1, placement2);

        // Verify ordering is stable
        assert!(placement1 < placement2);
    }

    #[test]
    fn test_scenery_mesh_identity_with_lod_fallback() {
        // Test that mesh identity supports LOD fallback via definition index
        let mesh_identity = MeshIdentity::new(
            "scenery/bus_stop.sco".to_string(),
            3,
            "shelter_wall".to_string(),
            None,
        );

        // LOD fallback: if exact mesh (def_index=3) is missing, find nearest
        let available = vec![1, 2, 5, 6]; // No 3, so nearest is 2 (distance=1)
        let fallback = mesh_identity.find_fallback_index(&available);

        assert_eq!(fallback, Some(2));

        // Verify mesh identity is preserved across LOD changes
        assert_eq!(mesh_identity.definition_index, 3);
        assert_eq!(mesh_identity.mesh_name, "shelter_wall");
    }

    #[test]
    fn test_slab_check_bounds_intersection() {
        let o = Vec3::new(0.0, 0.0, 0.0);
        let d = Vec3::new(1.0, 0.0, 0.0).normalize();
        let lo = Vec3::new(5.0, -1.0, -1.0);
        let hi = Vec3::new(10.0, 1.0, 1.0);

        // Ray along x-axis should intersect box at x=5 to x=10
        let result = slab_check(o, d, lo, hi);
        assert!(result.is_some());
        let (t0, t1) = result.unwrap();
        assert!(t0 >= 4.9 && t0 <= 5.1); // Entry at ~5.0
        assert!(t1 >= 9.9 && t1 <= 10.1); // Exit at ~10.0

        // Ray in opposite direction should also intersect (negative t values)
        // but for raycast purposes, we'd filter negative t values elsewhere
        let d_neg = Vec3::new(-1.0, 0.0, 0.0).normalize();
        let result_neg = slab_check(o, d_neg, lo, hi);
        // slab_check doesn't filter by direction, it returns mathematical intersection
        assert!(result_neg.is_some());

        // Ray parallel to box but outside should miss
        let o_outside = Vec3::new(0.0, 5.0, 0.0);
        let result_miss = slab_check(o_outside, d, lo, hi);
        assert!(result_miss.is_none());
    }

    #[test]
    fn test_scenery_raycast_helpers_are_side_effect_free() {
        // Type check: these functions exist and have the correct immutable signatures
        // All scenery raycast helpers take immutable borrows and return owned data
        
        let _: fn(&crate::scene::ObjectType, DVec3, &glam::Mat4, DVec3, Vec3, usize) -> Vec<SceneryMeshHit> =
            raycast_scenery_object_meshes;
        let _: fn(i32, i32, &[crate::scene::SceneryObjectRecord], DVec3, Vec3, DVec3) -> Vec<InspectorHit> =
            raycast_tile_scenery;
        
        // The signatures ensure no side effects: immutable borrows only, no state mutation
    }
}
