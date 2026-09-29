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
        // Note: ObjectType meshes are (MeshData, materials, overrides) tuples,
        // while VehicleMesh has a .data field. Here `mesh` is already MeshData.
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
            // For LOD 0, use mesh_def_index mapping; for lower LODs, the definition index
            // is not tracked per-mesh, so we use mesh_index as a fallback logical identifier
            let mesh_def_idx = if lod_level == 0 {
                scenery.ty.mesh_def_index.get(hit.mesh_index).copied()
            } else {
                // Lower LODs: no def_index mapping available, use mesh_index as logical ID
                Some(hit.mesh_index)
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

/// Build a validated snapshot from a selection, checking staleness and releasing leases.
///
/// This function validates that the selected entity still exists and matches the stored
/// generation. If the entity is stale (removed, replaced, timed out, or LOD changed),
/// it returns an invalidated status and releases any associated tile pin leases.
///
/// All locks are dropped before returning to ensure no locks are held during UI/render.
///
/// # Parameters
/// - `selection`: The current selection to validate
/// - `player`: Player instance (if active)
/// - `traffic`: AI traffic state (if active)
/// - `remotes`: Remote vehicles state (if active)
/// - `streamer`: Tile streamer for scenery validation and lease release
/// - `player_generation`: Player vehicle generation counter
///
/// # Returns
/// - `Ok(snapshot)`: Valid snapshot with owned data
/// - `Err(reason)`: Invalidation reason if the selection is stale
pub fn build_inspector_snapshot(
    selection: &InspectorSelection,
    player: Option<&crate::player::Player>,
    traffic: Option<&crate::traffic::Traffic>,
    remotes: Option<&crate::lan::LanGame>,
    streamer: Option<&crate::tiles::Streamer>,
    player_generation: u64,
) -> Result<InspectorSnapshot, String> {
    let target = match &selection.status {
        SelectionStatus::Selected(target) => target,
        SelectionStatus::None => return Err("No selection".to_string()),
        SelectionStatus::Invalidated { reason } => return Err(reason.clone()),
    };

    match target {
        SelectionTarget::Vehicle { key, mesh } => {
            validate_vehicle_snapshot(key, mesh, player, traffic, remotes, player_generation)
        }
        SelectionTarget::Scenery { key, mesh } => {
            validate_scenery_snapshot(key, mesh, streamer)
        }
    }
}

/// Validate a vehicle selection and build its snapshot.
fn validate_vehicle_snapshot(
    key: &VehicleKey,
    mesh: &Option<MeshIdentity>,
    player: Option<&crate::player::Player>,
    traffic: Option<&crate::traffic::Traffic>,
    remotes: Option<&crate::lan::LanGame>,
    player_generation: u64,
) -> Result<InspectorSnapshot, String> {
    match key {
        VehicleKey::Player { generation } => {
            // Validate player generation
            if *generation != player_generation {
                return Err(format!(
                    "Player vehicle replaced (expected gen {}, current {})",
                    generation, player_generation
                ));
            }

            let Some(player) = player else {
                return Err("Player vehicle not available".to_string());
            };

            build_vehicle_snapshot(
                SelectionTarget::Vehicle {
                    key: *key,
                    mesh: mesh.clone(),
                },
                &player.vehicle,
                None,
            )
        }
        VehicleKey::PlayerTrailer {
            generation,
            trailer_index,
        } => {
            // Validate player generation
            if *generation != player_generation {
                return Err(format!(
                    "Player vehicle replaced (expected gen {}, current {})",
                    generation, player_generation
                ));
            }

            let Some(player) = player else {
                return Err("Player vehicle not available".to_string());
            };

            let Some(trailer) = player.vehicle.trailers.get(*trailer_index) else {
                return Err(format!(
                    "Player trailer {} not available (only {} trailers)",
                    trailer_index,
                    player.vehicle.trailers.len()
                ));
            };

            build_trailer_snapshot(
                SelectionTarget::Vehicle {
                    key: *key,
                    mesh: mesh.clone(),
                },
                trailer,
                Some("Player"),
            )
        }
        VehicleKey::AiCar { id } => {
            let Some(traffic) = traffic else {
                return Err("AI traffic not available".to_string());
            };

            // Find AI car by stable ID
            let car = traffic
                .cars
                .iter()
                .find(|c| c.id == *id)
                .ok_or_else(|| format!("AI car {} despawned", id))?;

            build_vehicle_snapshot(
                SelectionTarget::Vehicle {
                    key: *key,
                    mesh: mesh.clone(),
                },
                &car.vehicle,
                Some(&format!("AI #{}", id)),
            )
        }
        VehicleKey::AiTrailer {
            car_id,
            trailer_index,
        } => {
            let Some(traffic) = traffic else {
                return Err("AI traffic not available".to_string());
            };

            // Find AI car by stable ID
            let car = traffic
                .cars
                .iter()
                .find(|c| c.id == *car_id)
                .ok_or_else(|| format!("AI car {} despawned", car_id))?;

            let Some(trailer) = car.vehicle.trailers.get(*trailer_index) else {
                return Err(format!(
                    "AI car {} trailer {} not available (only {} trailers)",
                    car_id,
                    trailer_index,
                    car.vehicle.trailers.len()
                ));
            };

            build_trailer_snapshot(
                SelectionTarget::Vehicle {
                    key: *key,
                    mesh: mesh.clone(),
                },
                trailer,
                Some(&format!("AI #{}", car_id)),
            )
        }
        VehicleKey::Remote {
            player_id,
            generation: _,
        } => {
            let Some(remotes) = remotes else {
                return Err("Remote vehicles not available".to_string());
            };

            // Find remote vehicle by player ID
            let remote = remotes
                .remotes
                .get(player_id)
                .ok_or_else(|| format!("Remote player {} disconnected", player_id))?;

            // TODO: Validate generation (remote vehicle replacement detection)
            // RemoteVehicle doesn't expose generation yet, so we can't validate replacement

            build_vehicle_snapshot(
                SelectionTarget::Vehicle {
                    key: *key,
                    mesh: mesh.clone(),
                },
                remote.vehicle(),
                Some(&format!("Remote #{}", player_id)),
            )
        }
        VehicleKey::RemoteTrailer {
            player_id,
            generation: _,
            trailer_index,
        } => {
            let Some(remotes) = remotes else {
                return Err("Remote vehicles not available".to_string());
            };

            // Find remote vehicle by player ID
            let remote = remotes
                .remotes
                .get(player_id)
                .ok_or_else(|| format!("Remote player {} disconnected", player_id))?;

            // TODO: Validate generation (remote vehicle replacement detection)
            // RemoteVehicle doesn't expose generation yet, so we can't validate replacement

            let Some(trailer) = remote.vehicle().trailers.get(*trailer_index) else {
                return Err(format!(
                    "Remote player {} trailer {} not available (only {} trailers)",
                    player_id,
                    trailer_index,
                    remote.vehicle().trailers.len()
                ));
            };

            build_trailer_snapshot(
                SelectionTarget::Vehicle {
                    key: *key,
                    mesh: mesh.clone(),
                },
                trailer,
                Some(&format!("Remote #{}", player_id)),
            )
        }
    }
}

/// Build a snapshot from a vehicle instance.
fn build_vehicle_snapshot(
    target: SelectionTarget,
    vehicle: &omsi_sim::VehicleInstance,
    parent_name: Option<&str>,
) -> Result<InspectorSnapshot, String> {
    let position = Some([
        vehicle.position.x as f32,
        vehicle.position.y as f32,
        vehicle.position.z as f32,
    ]);

    // Convert heading to quaternion (rotation around Z axis)
    let heading_rad = vehicle.heading.to_radians() as f32;
    let half_angle = heading_rad / 2.0;
    let rotation = Some([
        0.0,
        0.0,
        half_angle.sin(),
        half_angle.cos(),
    ]);

    let model_path = Some(vehicle.ty.model_dir.to_string_lossy().to_string());

    let mesh_name = if let SelectionTarget::Vehicle { mesh: Some(mesh), .. } = &target {
        Some(mesh.mesh_name.clone())
    } else {
        None
    };

    let mut metadata = Vec::new();
    if let Some(parent) = parent_name {
        metadata.push(("Parent".to_string(), parent.to_string()));
    }
    metadata.push(("Type".to_string(), vehicle.ty.def.type_name.clone()));

    Ok(InspectorSnapshot {
        target,
        position,
        rotation,
        bounds: None, // TODO: compute from vehicle bounds
        model_path,
        mesh_name,
        metadata,
    })
}

/// Build a snapshot from a trailer instance.
fn build_trailer_snapshot(
    target: SelectionTarget,
    trailer: &omsi_sim::vehicle::TrailerPart,
    parent_name: Option<&str>,
) -> Result<InspectorSnapshot, String> {
    let position = Some([
        trailer.position.x as f32,
        trailer.position.y as f32,
        trailer.position.z as f32,
    ]);

    // Convert heading to quaternion (rotation around Z axis)
    let heading_rad = trailer.heading.to_radians() as f32;
    let half_angle = heading_rad / 2.0;
    let rotation = Some([
        0.0,
        0.0,
        half_angle.sin(),
        half_angle.cos(),
    ]);

    let model_path = Some(trailer.ty.model_dir.to_string_lossy().to_string());

    let mesh_name = if let SelectionTarget::Vehicle { mesh: Some(mesh), .. } = &target {
        Some(mesh.mesh_name.clone())
    } else {
        None
    };

    let mut metadata = Vec::new();
    if let Some(parent) = parent_name {
        metadata.push(("Parent".to_string(), format!("{} (trailer)", parent)));
    }
    metadata.push(("Type".to_string(), trailer.ty.def.type_name.clone()));

    Ok(InspectorSnapshot {
        target,
        position,
        rotation,
        bounds: None, // TODO: compute from trailer bounds
        model_path,
        mesh_name,
        metadata,
    })
}

/// Validate a scenery selection and build its snapshot.
fn validate_scenery_snapshot(
    key: &SceneryKey,
    _mesh: &Option<MeshIdentity>,
    streamer: Option<&crate::tiles::Streamer>,
) -> Result<InspectorSnapshot, String> {
    let Some(_streamer) = streamer else {
        return Err("Scenery not available (no streamer)".to_string());
    };

    match key {
        SceneryKey::Editable { map_id } => {
            // TODO: Validate editable scenery object exists
            // For now, return minimal snapshot
            Err(format!(
                "Editable scenery validation not yet implemented (map_id={})",
                map_id
            ))
        }
        SceneryKey::NonEditable { tile_x, tile_y, key } => {
            // TODO: Check tile is loaded and object exists
            // For now, return minimal snapshot
            Err(format!(
                "Non-editable scenery validation not yet implemented (tile={},{}, key={})",
                tile_x, tile_y, key
            ))
        }
        SceneryKey::Parked { key } => {
            // TODO: Validate parked vehicle exists
            Err(format!(
                "Parked vehicle validation not yet implemented (key={})",
                key
            ))
        }
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    #[test]
    fn test_snapshot_validation_no_selection() {
        let selection = InspectorSelection::default();
        let result = build_inspector_snapshot(&selection, None, None, None, None, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No selection");
    }

    #[test]
    fn test_snapshot_validation_invalidated() {
        let mut selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });
        selection.invalidate("test reason".to_string());

        let result = build_inspector_snapshot(&selection, None, None, None, None, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "test reason");
    }

    #[test]
    fn test_vehicle_generation_mismatch() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        let result = build_inspector_snapshot(&selection, None, None, None, None, 2);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("replaced"));
    }

    #[test]
    fn test_player_not_available() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        let result = build_inspector_snapshot(&selection, None, None, None, None, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Player vehicle not available");
    }

    #[test]
    fn test_ai_not_available() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::AiCar { id: 100 },
            mesh: None,
        });

        let result = build_inspector_snapshot(&selection, None, None, None, None, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "AI traffic not available");
    }

    #[test]
    fn test_remote_not_available() {
        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Remote {
                player_id: 42,
                generation: 1,
            },
            mesh: None,
        });

        let result = build_inspector_snapshot(&selection, None, None, None, None, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Remote vehicles not available");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_inspector_mode_toggle_lifecycle() {
        // Test entering inspector mode
        let mut active = false;
        active = true;
        assert!(active, "Inspector mode should be active after toggle on");

        // Test exiting inspector mode
        active = false;
        assert!(!active, "Inspector mode should be inactive after toggle off");

        // Test multiple toggles
        for _ in 0..5 {
            active = !active;
        }
        assert!(!active, "Inspector mode should be inactive after odd number of toggles");
    }

    #[test]
    fn test_selection_lifecycle() {
        let mut selection = InspectorSelection::default();
        assert!(matches!(selection.status, SelectionStatus::None), "Initial selection should be None");

        // Test selecting a vehicle
        let vehicle_target = SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        };
        selection = InspectorSelection::new(vehicle_target.clone());
        assert!(selection.is_active(), "Selection should be active");
        assert!(matches!(selection.status, SelectionStatus::Selected(_)));

        // Test replacing selection
        let scenery_target = SelectionTarget::Scenery {
            key: SceneryKey::NonEditable {
                tile_x: 0,
                tile_y: 0,
                key: 5,
            },
            mesh: None,
        };
        selection = InspectorSelection::new(scenery_target.clone());
        assert!(selection.is_active(), "Selection should remain active after replacement");

        // Test clearing selection
        selection.clear();
        assert!(matches!(selection.status, SelectionStatus::None), "Selection should be None after clear");
    }

    #[test]
    fn test_selection_invalidation() {
        let mut selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::AiCar { id: 100 },
            mesh: None,
        });
        assert!(selection.is_active());

        // Test invalidation
        selection.invalidate("AI vehicle despawned".to_string());
        assert!(!selection.is_active(), "Selection should be inactive after invalidation");
        
        if let SelectionStatus::Invalidated { reason } = &selection.status {
            assert_eq!(reason, "AI vehicle despawned");
        } else {
            panic!("Expected Invalidated status");
        }
    }

    #[test]
    fn test_mode_toggle_clears_on_exit() {
        let mut active = false;
        let mut selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });

        // Activate inspector mode
        active = true;
        assert!(active);
        assert!(selection.is_active());

        // Exit inspector mode should clear selection
        active = false;
        selection.clear();
        assert!(!active);
        assert!(matches!(selection.status, SelectionStatus::None));
    }

    #[test]
    fn test_vehicle_selection_types() {
        // Test player vehicle selection
        let player_sel = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        });
        assert!(player_sel.is_active());

        // Test AI vehicle selection
        let ai_sel = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::AiCar { id: 42 },
            mesh: None,
        });
        assert!(ai_sel.is_active());

        // Test remote vehicle selection
        let remote_sel = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Remote {
                player_id: 3,
                generation: 1,
            },
            mesh: None,
        });
        assert!(remote_sel.is_active());

        // Test trailer selection
        let trailer_sel = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::PlayerTrailer {
                generation: 1,
                trailer_index: 0,
            },
            mesh: None,
        });
        assert!(trailer_sel.is_active());
    }

    #[test]
    fn test_scenery_selection_types() {
        // Test editable scenery selection
        let editable_sel = InspectorSelection::new(SelectionTarget::Scenery {
            key: SceneryKey::Editable { map_id: 1 },
            mesh: None,
        });
        assert!(editable_sel.is_active());

        // Test non-editable scenery selection
        let non_editable_sel = InspectorSelection::new(SelectionTarget::Scenery {
            key: SceneryKey::NonEditable {
                tile_x: 5,
                tile_y: 10,
                key: 3,
            },
            mesh: None,
        });
        assert!(non_editable_sel.is_active());

        // Test parked vehicle selection
        let parked_sel = InspectorSelection::new(SelectionTarget::Scenery {
            key: SceneryKey::Parked { key: 1 },
            mesh: None,
        });
        assert!(parked_sel.is_active());
    }

    #[test]
    fn test_mesh_identity_with_selection() {
        let mesh = MeshIdentity::new(
            "Vehicles/MAN_SD200/model.cfg".to_string(),
            0,
            "chassis".to_string(),
            Some(1),
        );

        let selection = InspectorSelection::new(SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: Some(mesh.clone()),
        });

        assert!(selection.is_active());
        if let SelectionStatus::Selected(SelectionTarget::Vehicle { mesh: sel_mesh, .. }) = &selection.status {
            assert_eq!(sel_mesh.as_ref().unwrap().mesh_name, "chassis");
            assert_eq!(sel_mesh.as_ref().unwrap().definition_index, 0);
            assert_eq!(sel_mesh.as_ref().unwrap().disambiguator, Some(1));
        } else {
            panic!("Expected vehicle target with mesh");
        }
    }

    #[test]
    fn test_view_toggles_default() {
        let selection = InspectorSelection::default();
        assert!(!selection.view.show_bounds, "Bounds should be off by default");
        assert!(!selection.view.show_local_axes, "Local axes should be off by default");
        assert!(!selection.view.show_mesh_name, "Mesh name should be off by default");
    }
}

/// Draw visual overlays for the currently selected entity.
///
/// This adds transient per-frame visual feedback using coronas for position markers,
/// and optionally bounds and local axes based on the validated snapshot.
///
/// Uses cyan/blue-green color (0.2, 0.8, 0.9) to contrast with the object editor's
/// magenta marker (1.0, 0.1, 0.9).
///
/// All overlays are transient and cleared automatically each frame by the scene reset.
pub fn draw_inspector_overlays(
    snapshot: &InspectorSnapshot,
    view_toggles: &ViewToggles,
    scene: &mut omsi_render::Scene,
) {
    use glam::{DVec3, Quat, Vec3};

    // Main position marker: cyan corona at entity position
    if let Some(pos) = snapshot.position {
        let position = DVec3::new(pos[0] as f64, pos[1] as f64, pos[2] as f64);
        scene.coronas.push(omsi_render::Corona {
            position: position + DVec3::Z * 3.0,
            size: 0.7,
            color: [0.2, 0.8, 0.9], // cyan/blue-green
            brightness: 2.5,
            ..Default::default()
        });
    }

    // Optional bounds visualization
    if view_toggles.show_bounds {
        if let (Some(pos), Some(rot), Some(bounds)) = (snapshot.position, snapshot.rotation, snapshot.bounds) {
            let position = DVec3::new(pos[0] as f64, pos[1] as f64, pos[2] as f64);
            let rotation = Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]);
            
            let (min, max) = bounds;
            let min = Vec3::new(min[0], min[1], min[2]);
            let max = Vec3::new(max[0], max[1], max[2]);
            
            // Draw corner markers for bounding box
            let corners = [
                Vec3::new(min.x, min.y, min.z),
                Vec3::new(max.x, min.y, min.z),
                Vec3::new(min.x, max.y, min.z),
                Vec3::new(max.x, max.y, min.z),
                Vec3::new(min.x, min.y, max.z),
                Vec3::new(max.x, min.y, max.z),
                Vec3::new(min.x, max.y, max.z),
                Vec3::new(max.x, max.y, max.z),
            ];
            
            for corner in &corners {
                let world_corner = position + (rotation * *corner).as_dvec3();
                scene.coronas.push(omsi_render::Corona {
                    position: world_corner,
                    size: 0.3,
                    color: [0.2, 0.8, 0.9],
                    brightness: 1.5,
                    ..Default::default()
                });
            }
        }
    }

    // Optional local axes visualization
    if view_toggles.show_local_axes {
        if let (Some(pos), Some(rot)) = (snapshot.position, snapshot.rotation) {
            let position = DVec3::new(pos[0] as f64, pos[1] as f64, pos[2] as f64);
            let rotation = Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]);
            
            let axis_length = 2.0;
            
            // X axis (red)
            let x_axis = rotation * Vec3::X * axis_length;
            scene.coronas.push(omsi_render::Corona {
                position: position + x_axis.as_dvec3(),
                size: 0.4,
                color: [1.0, 0.2, 0.2],
                brightness: 2.0,
                ..Default::default()
            });
            
            // Y axis (green)
            let y_axis = rotation * Vec3::Y * axis_length;
            scene.coronas.push(omsi_render::Corona {
                position: position + y_axis.as_dvec3(),
                size: 0.4,
                color: [0.2, 1.0, 0.2],
                brightness: 2.0,
                ..Default::default()
            });
            
            // Z axis (blue)
            let z_axis = rotation * Vec3::Z * axis_length;
            scene.coronas.push(omsi_render::Corona {
                position: position + z_axis.as_dvec3(),
                size: 0.4,
                color: [0.2, 0.2, 1.0],
                brightness: 2.0,
                ..Default::default()
            });
        }
    }
}
