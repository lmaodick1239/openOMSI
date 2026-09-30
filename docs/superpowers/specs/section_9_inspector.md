## 9. Cross-Crate Dependencies & Integration Surface

This section provides a complete architectural map of how the Inspector subsystems integrate across the openOMSI crate ecosystem. Each subsystem crosses multiple crate boundaries; this design ensures clean separation of concerns while maintaining snapshot isolation, zero-stall GPU guarantees, and thread-safe operation.

---

### 9.1 High-Level Dependency Graph

The Inspector orchestration layer lives in **`omsi-app`** and coordinates across all domain-specific crates. The fundamental data flow follows a strict unidirectional pattern:

```
                    USER INPUT (Mouse, Keyboard, UI Events)
                                     |
                                     v
        +----------------------------------------------------------------+
        |                         omsi-app                               |
        |                    [Inspector Orchestrator]                    |
        |  - Input routing & mode toggling                               |
        |  - Frame synchronization boundary                              |
        |  - Atomic snapshot construction                                |
        +----------------------------------------------------------------+
                 |              |              |              |
       +---------+---------+----+----+----+----+----+---------+---------+
       |                   |         |         |              |         |
       v                   v         v         v              v         v
+-------------+  +----------------+  |  +--------------+  +----------+  |
| omsi-render |  | omsi-geometry  |  |  |  omsi-sim    |  | omsi-ui  |  |
|             |  |                |  |  |              |  |          |  |
| GPU queries |  | BVH raycasting |  |  | Physics data |  | Panel    |  |
| Frame graph |  | Spline hits    |  |  | Humans/AI    |  | rendering|  |
| Texture dump|  | Collision hulls|  |  | Script VM    |  | i18n     |  |
+-------------+  +----------------+  |  +--------------+  +----------+  |
       |                   |         |         |              |         |
       v                   v         v         v              v         v
+-------------+  +----------------+  |  +--------------+  +----------+  |
| omsi-texture|  | omsi-scenery   |  |  | omsi-script  |  | omsi-map |  |
|             |  |                |  |  |              |  |          |  |
| Mipmap meta |  | Tile pin lease |  |  | Variable     |  | Spline   |  |
| Format info |  | Scenery keys   |  |  | introspection|  | metadata |  |
+-------------+  +----------------+  |  +--------------+  +----------+  |
                                     |                                  |
                                     v                                  v
                          +----------------+              +----------------+
                          | omsi-vehicle   |              | omsi-audio     |
                          |                |              |                |
                          | Vehicle systems|              | Spatial sources|
                          | Part hierarchy |              | Attenuation    |
                          +----------------+              +----------------+
                                     |                                  |
                                     v                                  v
                          +----------------+              +----------------+
                          | omsi-model     |              | omsi-o3d       |
                          |                |              |                |
                          | Mesh metadata  |              | Vertex/triangle|
                          | Bone hierarchy |              | data access    |
                          +----------------+              +----------------+

                     READ-ONLY SNAPSHOT FLOW (60Hz)
                     ================================
                     All queries flow through immutable snapshots.
                     No crate holds inspector locks across frames.
```

---

### 9.2 Subsystem-to-Crate Integration Matrix

| Subsystem | Primary Crates | Integration Points | Snapshot Data Sources |
|:----------|:---------------|:-------------------|:----------------------|
| **E: Human/Pedestrian** | `omsi-sim`, `omsi-app`, `omsi-model`, `omsi-geometry` | `omsi-sim::human` state machines, skeletal pose queries, bounding cylinder raycasts | `HumanSnapshot { key, position, velocity, behavior_state, animation_phase, skeleton_bones }` |
| **A: Materials/Textures** | `omsi-render`, `omsi-texture`, `omsi-model` | Material property queries, texture metadata reads, async GPU staging pool | `MaterialSnapshot { diffuse_tex, normal_tex, pbr_params, vram_bytes, mipmap_count }` |
| **B: Script VM** | `omsi-script`, `omsi-vehicle`, `omsi-scenery` | Variable enumeration, live override injection, RPN stack introspection | `ScriptSnapshot { variables: HashMap<String, f32>, active_macros, stack_top }` |
| **C: Render Pipeline** | `omsi-render`, `omsi-geometry` | Frame graph traversal, render pass timings, wireframe/isolation toggles | `RenderSnapshot { passes: Vec<PassInfo>, gpu_time_ms, active_pipelines }` |
| **G: Selection/Hierarchy** | `omsi-geometry`, `omsi-app`, `omsi-vehicle`, `omsi-scenery` | BVH traversal, multi-hit corridor, parent-child navigation | `SelectionSnapshot { hit_stack: Vec<HitRecord>, world_pose, local_pose }` |
| **I: Persistence/Export** | `omsi-app`, `omsi-ui`, `omsi-model`, `omsi-o3d` | Layout serialization, glTF export, WebSocket telemetry server | `LayoutSnapshot { panel_geometry, watch_table, filter_state }` |
| **D: Splines/Traffic** | `omsi-map`, `omsi-geometry`, `omsi-sim` | Spline geometry raycasting, traffic path queries, signal controller state | `SplineSnapshot { key, profile, gradient_pct, curvature_radius, traffic_paths }` |
| **H: Audio/Environment** | `omsi-audio`, `omsi-sim`, `omsi-app` | Spatial emitter enumeration, acoustic cone geometry, weather probes | `AudioSnapshot { emitters: Vec<SoundSource>, volume_db, doppler_shift }` |
| **F: Editor Bridge** | `omsi-app`, `omsi-scenery`, `omsi-ui` | Non-destructive mutation buffer, editor mode handoff | `EditorSnapshot { modified_properties, original_values, transaction_id }` |

---

### 9.3 API Surface Changes by Crate

#### 9.3.1 `omsi-app` (Orchestration & Frame Boundary)

**New Public API:**
```rust
// crates/omsi-app/src/inspector/mod.rs
pub struct Inspector {
    mode: InspectorMode,
    raycast_request: Option<RaycastRequest>,
    snapshot: Option<InspectorSnapshot>,
    tile_pin_lease: Option<TilePinLease>,
}

pub enum InspectorMode {
    Disabled,
    Active { selected: Option<EntityKey> },
}

pub enum EntityKey {
    Vehicle(VehicleKey),
    Scenery(SceneryKey),
    Human(HumanKey),
    Spline(SplineKey),
}

pub struct InspectorSnapshot {
    pub frame_id: u64,
    pub entity: Option<EntitySnapshot>,
    pub render_info: Option<RenderSnapshot>,
    pub script_info: Option<ScriptSnapshot>,
    pub audio_info: Option<AudioSnapshot>,
}

impl Inspector {
    /// Called once per frame at the app_events.rs synchronization boundary.
    /// Atomically constructs a complete snapshot without holding any locks.
    pub fn update_snapshot(
        &mut self,
        world: &World,
        render_state: &RenderState,
        sim_state: &SimState,
    ) -> Result<(), InspectorError>;
    
    /// Inject live script overrides into the simulation state.
    /// Fails closed: invalid ranges are clamped, despawned entities reject writes.
    pub fn apply_script_overrides(
        &self,
        script_context: &mut ScriptContext,
    ) -> Result<(), InspectorError>;
}
```

**Integration Points:**
- `crates/omsi-app/src/app_events.rs`: Add `inspector.update_snapshot()` at frame sync point.
- `crates/omsi-app/src/input.rs`: Route mouse/keyboard to inspector when `InspectorMode::Active`.
- `crates/omsi-app/src/main_loop.rs`: Check `inspector.mode` to skip world interaction when active.

---

#### 9.3.2 `omsi-geometry` (Raycasting & Spatial Queries)

**New Public API:**
```rust
// crates/omsi-geometry/src/broadphase.rs
pub struct BroadphaseQuery {
    pub ray_origin: Vec3,
    pub ray_direction: Vec3,
    pub max_distance: f32,
}

pub struct BroadphaseResult {
    pub candidates: Vec<CandidateEntity>,
}

pub enum CandidateEntity {
    VehiclePart { key: VehicleKey, part_index: usize, aabb: Aabb },
    SceneryObject { key: SceneryKey, aabb: Aabb },
    Human { key: HumanKey, cylinder: BoundingCylinder },
    Spline { key: SplineKey, extruded_bounds: Aabb },
}

pub trait BroadphaseAccelerator {
    /// Fast spatial query: returns candidate entities sorted by distance.
    /// Target: < 0.4 ms even in 100k-triangle maps.
    fn query(&self, query: BroadphaseQuery) -> BroadphaseResult;
}

// crates/omsi-geometry/src/raycast.rs
pub struct RaycastHit {
    pub distance: f32,
    pub entity: EntityKey,
    pub triangle_index: usize,
    pub barycentric: Vec3,
    pub world_position: Vec3,
    pub world_normal: Vec3,
}

pub fn raycast_narrowphase(
    candidate: &CandidateEntity,
    ray: &Ray,
    mesh_data: &MeshData,
) -> Option<RaycastHit>;
```

**Integration Points:**
- `omsi-vehicle`, `omsi-scenery`, `omsi-sim::human`: Register bounding volumes into broadphase.
- `omsi-map`: Spline geometry converted to extruded AABBs for raycast acceleration.

---

#### 9.3.3 `omsi-render` (GPU Queries & Frame Graph)

**New Public API:**
```rust
// crates/omsi-render/src/inspector.rs
pub struct RenderSnapshot {
    pub frame_id: u64,
    pub passes: Vec<PassInfo>,
    pub total_gpu_time_ms: f32,
    pub active_pipelines: Vec<String>,
}

pub struct PassInfo {
    pub name: String,
    pub gpu_time_ms: f32,
    pub draw_calls: usize,
    pub triangles: usize,
}

pub struct TextureQuery {
    pub handle: TextureHandle,
    pub request_mip_level: Option<u8>,
}

pub struct TextureQueryResult {
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub mip_count: u8,
    pub vram_bytes: usize,
    pub async_readback_id: Option<ReadbackId>,
}

impl RenderState {
    /// Non-blocking texture metadata query.
    /// Actual pixel data retrieved via async staging pool (A3, A+).
    pub fn query_texture_metadata(
        &self,
        query: TextureQuery,
    ) -> TextureQueryResult;
    
    /// Submit async readback request for texture dump.
    /// Returns immediately; result available 2-3 frames later.
    pub fn request_texture_readback(
        &mut self,
        handle: TextureHandle,
        mip_level: u8,
    ) -> ReadbackId;
    
    /// Poll completed readback; returns None if still pending.
    pub fn poll_readback(&mut self, id: ReadbackId) -> Option<Vec<u8>>;
}
```

**Integration Points:**
- `omsi-texture`: Expose texture handle → metadata mapping.
- `wgpu` timestamp queries: Wrap render passes with query begin/end for GPU profiling.
- No synchronous `Device::poll(Wait)` calls; all staging uses async pools.

---

#### 9.3.4 `omsi-sim` (Physics, Humans, Script State)

**New Public API:**
```rust
// crates/omsi-sim/src/human.rs
pub struct HumanSnapshot {
    pub key: HumanKey,
    pub position: Vec3,
    pub heading_rad: f32,
    pub velocity_mps: f32,
    pub behavior: HumanBehavior,
    pub animation_phase: f32, // [0.0..1.0]
    pub skeleton: Option<SkeletonSnapshot>, // E+
}

pub enum HumanBehavior {
    Wandering,
    WalkingToStop { stop_id: usize },
    WaitingAtStop { stop_id: usize },
    BoardingBus { vehicle_key: VehicleKey },
    Seated { vehicle_key: VehicleKey, seat_index: usize },
    Alighting,
}

pub struct SkeletonSnapshot {
    pub bones: Vec<BoneTransform>,
}

pub struct BoneTransform {
    pub name: String,
    pub local_position: Vec3,
    pub local_rotation: Quat,
}

impl HumanSimulation {
    /// Snapshot all active humans without holding locks across frames.
    pub fn snapshot_humans(&self) -> Vec<HumanSnapshot>;
    
    /// Query skeletal animation state for detailed inspection (E+).
    pub fn snapshot_skeleton(&self, key: HumanKey) -> Option<SkeletonSnapshot>;
}
```

**Integration Points:**
- `omsi-app`: Call `human_sim.snapshot_humans()` during frame sync.
- `omsi-geometry`: Provide bounding cylinders for broadphase acceleration.

---

#### 9.3.5 `omsi-script` (VM Introspection & Live Overrides)

**New Public API:**
```rust
// crates/omsi-script/src/inspector.rs
pub struct ScriptSnapshot {
    pub variables: HashMap<String, ScriptValue>,
    pub active_macros: Vec<String>,
    pub rpn_stack_top: Vec<f32>, // Top 8 elements
    pub trigger_log: Vec<TriggerEvent>,
}

pub enum ScriptValue {
    Float(f32),
    String(String),
}

pub struct TriggerEvent {
    pub timestamp_ms: u64,
    pub name: String,
}

pub struct VariableOverride {
    pub name: String,
    pub value: ScriptValue,
    pub clamp_range: Option<(f32, f32)>,
}

impl ScriptContext {
    /// Enumerate all local variables for the selected entity.
    pub fn snapshot_variables(&self) -> HashMap<String, ScriptValue>;
    
    /// Inject live override; clamped to valid ranges.
    /// Returns Err if variable doesn't exist or entity despawned.
    pub fn apply_override(
        &mut self,
        override_request: VariableOverride,
    ) -> Result<(), ScriptError>;
    
    /// Clear all active overrides; restore simulation authority.
    pub fn clear_overrides(&mut self);
    
    /// Capture RPN stack state for debugging (B+).
    pub fn snapshot_rpn_stack(&self) -> Vec<f32>;
}
```

**Integration Points:**
- `omsi-vehicle`, `omsi-scenery`: Expose script contexts for variable enumeration.
- `omsi-app`: Call `apply_override()` after user edits inspector UI fields.

---

#### 9.3.6 `omsi-scenery` (Tile Pin Leases)

**New Public API:**
```rust
// crates/omsi-scenery/src/inspector.rs
pub struct TilePinLease {
    pub tile_x: i32,
    pub tile_y: i32,
    pub lease_id: u64,
}

pub struct ScenerySnapshot {
    pub key: SceneryKey,
    pub tile: (i32, i32),
    pub world_position: Vec3,
    pub world_rotation: Quat,
    pub model_path: String,
    pub config_path: String,
}

impl SceneryManager {
    /// Request a pin lease for the tile containing the selected scenery object.
    /// Prevents streaming unload while inspector is active.
    /// Strict maximum: ONE pinned tile at any time.
    pub fn request_tile_pin(
        &mut self,
        key: SceneryKey,
    ) -> Result<TilePinLease, SceneryError>;
    
    /// Release pin lease; allows tile to stream out normally.
    pub fn release_tile_pin(&mut self, lease: TilePinLease);
    
    /// Snapshot selected scenery object without holding locks.
    pub fn snapshot_scenery(&self, key: SceneryKey) -> Option<ScenerySnapshot>;
}
```

**Integration Points:**
- `omsi-app`: Manage `TilePinLease` lifecycle; release on selection change or inspector exit.
- `omsi-map`: Respect active pin leases during tile unload decisions.

---

#### 9.3.7 `omsi-map` (Spline Geometry & Traffic Paths)

**New Public API:**
```rust
// crates/omsi-map/src/spline.rs
pub struct SplineSnapshot {
    pub key: SplineKey,
    pub profile_path: String,
    pub length_m: f32,
    pub gradient_pct: f32,
    pub banking_deg: f32,
    pub curvature_radius_m: Option<f32>,
    pub traffic_paths: Vec<TrafficPath>, // D+
}

pub struct TrafficPath {
    pub group: TrafficGroup,
    pub speed_limit_kmh: u32,
    pub nodes: Vec<Vec3>,
}

pub enum TrafficGroup {
    Cars,
    BusesOnly,
    Trucks,
    Trams,
    Pedestrians,
}

impl SplineManager {
    /// Raycast against extruded spline geometry.
    pub fn raycast_spline(
        &self,
        ray: &Ray,
    ) -> Option<(SplineKey, f32)>;
    
    /// Snapshot spline metadata for inspector display.
    pub fn snapshot_spline(&self, key: SplineKey) -> Option<SplineSnapshot>;
}
```

**Integration Points:**
- `omsi-geometry`: Register spline AABBs into broadphase.
- `omsi-sim`: Traffic controller queries for signal phase inspection (D+).

---

#### 9.3.8 `omsi-audio` (Spatial Emitters)

**New Public API:**
```rust
// crates/omsi-audio/src/inspector.rs
pub struct AudioSnapshot {
    pub emitters: Vec<SoundEmitter>,
}

pub struct SoundEmitter {
    pub id: usize,
    pub position: Vec3,
    pub source_name: String,
    pub volume_db: f32,
    pub pitch_factor: f32,
    pub inner_radius_m: f32,
    pub outer_radius_m: f32,
    pub cone_direction: Option<Vec3>, // H+
    pub cone_angle_deg: Option<f32>,  // H+
}

impl AudioManager {
    /// Enumerate all active spatial audio sources.
    pub fn snapshot_emitters(&self) -> Vec<SoundEmitter>;
}
```

**Integration Points:**
- `omsi-app`: Call `audio_manager.snapshot_emitters()` during frame sync.
- `omsi-render`: Render acoustic visualization geometry (spheres, cones).

---

#### 9.3.9 `omsi-ui` (Panel Rendering & i18n)

**New Public API:**
```rust
// crates/omsi-ui/src/inspector_panel.rs
pub struct InspectorPanel {
    pub geometry: PanelGeometry,
    pub active_tab: InspectorTab,
    pub watch_table: Vec<WatchExpression>,
    pub filter_query: String,
}

pub struct PanelGeometry {
    pub screen_x: i32,
    pub screen_y: i32,
    pub width: u32,
    pub height: u32,
    pub collapsed: bool,
}

pub enum InspectorTab {
    Overview,
    Transform,
    Material,
    ScriptVM,
    RenderPipeline,
    Audio,
    Splines,
}

impl InspectorPanel {
    /// Render UI panel using egui; consumes snapshot data.
    /// Strict input capture: panel rect consumes all mouse events.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        snapshot: &InspectorSnapshot,
        locale: &Locale,
    );
    
    /// Serialize panel state for persistence (I1).
    pub fn serialize(&self) -> PanelState;
    
    /// Restore panel state from disk.
    pub fn deserialize(state: PanelState) -> Self;
}
```

**Integration Points:**
- `omsi-app`: Instantiate `InspectorPanel` and call `render()` each frame when active.
- `crates/omsi-app/locales/app.yml`: Add inspector UI string keys for all 24 languages.

---

### 9.4 Data Flow Diagrams by Subsystem

#### 9.4.1 Subsystem E (Humans/Pedestrians) Data Flow

```
USER RAYCAST                    FRAME SYNC BOUNDARY              UI RENDER
     |                                  |                              |
     v                                  v                              v
+----------+                   +------------------+         +----------------+
| Click 3D | ---- query ----> | omsi-geometry    | ----->  | omsi-app       |
| viewport |                  | BVH broadphase   |         | Inspector      |
+----------+                  +------------------+         | orchestrator   |
                                       |                   +----------------+
                                       v                            |
                              +------------------+                  |
                              | Candidate humans |                  v
                              | (bounding cyls)  |         +----------------+
                              +------------------+         | omsi-ui        |
                                       |                   | InspectorPanel |
                                       v                   +----------------+
                              +------------------+                  ^
                              | omsi-sim::human  |                  |
                              | Narrowphase hit  |                  |
                              | test (triangles) |                  |
                              +------------------+                  |
                                       |                            |
                                       v                            |
                              +------------------+                  |
                              | HumanKey         |                  |
                              | generation check |                  |
                              +------------------+                  |
                                       |                            |
                                       v                            |
                              +------------------+        snapshot  |
                              | HumanSnapshot    | -----------------+
                              | - position       |
                              | - velocity       |
                              | - behavior state |
                              | - skeleton bones |
                              +------------------+
```

---

#### 9.4.2 Subsystem B (Script VM) Data Flow

```
USER EDIT                      FRAME SYNC BOUNDARY              SIMULATION
     |                                  |                              |
     v                                  v                              v
+----------+                   +------------------+         +----------------+
| Modify   | ---- override --> | omsi-app         | ----->  | omsi-script    |
| L.throttle|                  | Inspector        |         | ScriptContext  |
| value    |                  | apply_overrides()|         | apply_override()|
+----------+                  +------------------+         +----------------+
                                       ^                            |
                                       |                            v
                              +------------------+         +----------------+
                              | Clamp validation |         | Mutate L.var   |
                              | [0.0..1.0]       |         | in VM state    |
                              +------------------+         +----------------+
                                       ^                            |
                                       |                            v
                              +------------------+         +----------------+
                              | Generation check |         | Physics update |
                              | (vehicle alive?) |         | uses overridden|
                              +------------------+         | throttle value |
                                       ^                   +----------------+
                                       |                            |
                                       |                            v
                              READ SNAPSHOT              +----------------+
                                       |                 | Vehicle        |
                                       +---------------- | acceleration   |
                                                         | changes        |
                                                         +----------------+

RESET BUTTON                   FRAME SYNC BOUNDARY              SIMULATION
     |                                  |                              |
     v                                  v                              v
+----------+                   +------------------+         +----------------+
| Click    | ---- clear -----> | omsi-app         | ----->  | omsi-script    |
| "Reset   |                   | Inspector        |         | ScriptContext  |
| Overrides|                   | clear_overrides()|         | clear_overrides|
+----------+                   +------------------+         +----------------+
                                                                     |
                                                                     v
                                                            +----------------+
                                                            | Restore        |
                                                            | simulation     |
                                                            | authority      |
                                                            +----------------+
```

---

#### 9.4.3 Subsystem A (Materials/Textures) Data Flow

```
SELECTION                      FRAME SYNC BOUNDARY              GPU READBACK
     |                                  |                              |
     v                                  v                              v
+----------+                   +------------------+         +----------------+
| Selected | ---- query -----> | omsi-render      | ----->  | Async staging  |
| mesh     |                   | query_texture_   |         | pool           |
| material |                   | metadata()       |         +----------------+
+----------+                   +------------------+                  |
                                       |                             v
                                       v                   +----------------+
                              +------------------+         | wgpu::Buffer   |
                              | TextureQuery     |         | readback       |
                              | - handle         |         | (2-3 frames)   |
                              | - mip_level      |         +----------------+
                              +------------------+                  |
                                       |                            v
                                       v                   +----------------+
                              +------------------+         | PNG encoder    |
                              | MaterialSnapshot | <------ | Vec<u8> pixels |
                              | - diffuse_tex    |         +----------------+
                              | - normal_tex     |                  |
                              | - pbr_params     |                  v
                              | - vram_bytes     |         +----------------+
                              | - mipmap_count   |         | Write to       |
                              +------------------+         | Screenshots/   |
                                       |                   | inspector_dump/|
                                       v                   +----------------+
                              +------------------+
                              | omsi-ui          |
                              | Render thumbnail |
                              | Display metadata |
                              +------------------+

CRITICAL: No synchronous Device::poll(Wait) calls.
All GPU readbacks use async staging; UI polls completion.
```

---

### 9.5 Snapshot Boundary Definitions Across Crates

The Inspector architecture enforces strict **snapshot isolation** at the frame synchronization boundary in `crates/omsi-app/src/app_events.rs`. This is the **only** location where inspector snapshots are constructed.

#### 9.5.1 Snapshot Construction Algorithm

```rust
// Pseudocode: crates/omsi-app/src/app_events.rs

fn frame_sync_boundary(
    world: &World,
    render_state: &RenderState,
    sim_state: &SimState,
    inspector: &mut Inspector,
) {
    // === SNAPSHOT CONSTRUCTION (Single-Pass, Lock-Free) ===
    
    let snapshot = if let Some(entity_key) = inspector.selected_entity() {
        // Step 1: Validate generational key
        let valid = match entity_key {
            EntityKey::Vehicle(key) => world.vehicles.validate_key(key),
            EntityKey::Scenery(key) => world.scenery.validate_key(key),
            EntityKey::Human(key) => sim_state.humans.validate_key(key),
            EntityKey::Spline(key) => world.map.validate_spline_key(key),
        };
        
        if !valid {
            // Entity despawned; clear selection and release leases
            inspector.clear_selection();
            inspector.release_tile_pin();
            None
        } else {
            // Step 2: Atomic snapshot construction (no locks held)
            Some(InspectorSnapshot {
                frame_id: world.frame_id,
                
                // Subsystem E: Human kinematics
                entity: match entity_key {
                    EntityKey::Human(key) => {
                        sim_state.humans.snapshot_human(key)
                    },
                    EntityKey::Vehicle(key) => {
                        world.vehicles.snapshot_vehicle(key)
                    },
                    EntityKey::Scenery(key) => {
                        world.scenery.snapshot_scenery(key)
                    },
                    EntityKey::Spline(key) => {
                        world.map.snapshot_spline(key)
                    },
                },
                
                // Subsystem A: Material/texture metadata
                render_info: if matches!(entity_key, EntityKey::Vehicle(_) | EntityKey::Scenery(_)) {
                    Some(render_state.snapshot_material(entity_key))
                } else {
                    None
                },
                
                // Subsystem B: Script VM state
                script_info: if matches!(entity_key, EntityKey::Vehicle(_) | EntityKey::Scenery(_)) {
                    Some(sim_state.scripts.snapshot_variables(entity_key))
                } else {
                    None
                },
                
                // Subsystem H: Audio emitters
                audio_info: Some(sim_state.audio.snapshot_emitters()),
                
                // Subsystem C: Render pipeline state
                pipeline_info: Some(render_state.snapshot_frame_graph()),
            })
        }
    } else {
        None
    };
    
    // Step 3: Atomically swap snapshot into inspector
    inspector.update_snapshot(snapshot);
    
    // === MUTATION APPLICATION (Separate Phase) ===
    
    // Step 4: Apply live script overrides (fail-closed)
    if let Err(e) = inspector.apply_script_overrides(&mut sim_state.scripts) {
        eprintln!("Inspector override failed: {:?}", e);
        inspector.clear_overrides();
    }
}
```

#### 9.5.2 Snapshot Data Ownership

All snapshot structs are **owned, immutable, and UI-safe**:

```rust
// Good: Owned data, safe to hold across frames
pub struct InspectorSnapshot {
    pub frame_id: u64,
    pub entity: Option<EntitySnapshot>,  // Owned
    pub render_info: Option<RenderSnapshot>,  // Owned
    pub script_info: Option<ScriptSnapshot>,  // Owned (HashMap clone)
}

// Bad: Never do this
pub struct BadSnapshot<'a> {
    pub entity: &'a Vehicle,  // ❌ Borrowed reference
    pub script_lock: MutexGuard<'a, ScriptContext>,  // ❌ Lock held across frames
}
```

#### 9.5.3 Cross-Crate Snapshot Boundary Rules

| Crate | Snapshot Method | Locking Strategy | Data Ownership |
|:------|:----------------|:-----------------|:---------------|
| `omsi-sim` | `snapshot_human(key) -> HumanSnapshot` | No locks held; atomic reads of position/velocity | Owned `Vec3`, `f32`, enums |
| `omsi-script` | `snapshot_variables(key) -> HashMap<String, f32>` | Clone `HashMap`; no locks held across frames | Owned `HashMap` (cloned) |
| `omsi-render` | `snapshot_material(key) -> MaterialSnapshot` | Async GPU queries; metadata only (no VRAM locks) | Owned metadata structs |
| `omsi-scenery` | `snapshot_scenery(key) -> ScenerySnapshot` | Reference-counted strings; no tile locks held | Owned `String`, `Vec3`, `Quat` |
| `omsi-audio` | `snapshot_emitters() -> Vec<SoundEmitter>` | Clone active emitter list; no audio mixer locks | Owned `Vec` (cloned) |

---

### 9.6 Thread Safety Considerations for Cross-Crate Inspector Calls

The Inspector operates across three primary threads in openOMSI:

```
MAIN THREAD              RENDER THREAD           SIMULATION THREAD
     |                        |                         |
     v                        v                         v
+----------+         +------------------+      +----------------+
| Input    |         | wgpu rendering   |      | Physics step   |
| handling |         | Frame graph      |      | Script VM tick |
| UI render|         | GPU commands     |      | AI pathfinding |
+----------+         +------------------+      +----------------+
     |                        |                         |
     +------------------------+-------------------------+
                              |
                              v
                   +---------------------+
                   | FRAME SYNC BOUNDARY |
                   | (app_events.rs)     |
                   | Inspector snapshot  |
                   | construction        |
                   +---------------------+
```

#### 9.6.1 Thread Safety Guarantees

1. **Snapshot Construction (Frame Sync Thread):**
   - Runs on the **main thread** at frame boundary.
   - No locks held across frames.
   - All snapshot methods are **non-blocking** and return owned data.

2. **UI Rendering (Main Thread):**
   - `omsi-ui::InspectorPanel` renders using **owned snapshot data**.
   - No direct access to simulation or render state.
   - Mouse/keyboard input captured within panel bounds; never interferes with world interaction.

3. **Script Override Application (Main Thread → Sim Thread):**
   - Overrides queued in inspector state.
   - Applied during frame sync via `apply_script_overrides()`.
   - Uses **message-passing** or **atomic flags** to communicate with simulation thread.

4. **GPU Texture Queries (Main Thread → Render Thread):**
   - `request_texture_readback()` submits async request.
   - **No blocking waits** on GPU completion.
   - `poll_readback()` checks completion without stalling.

#### 9.6.2 Lock-Free Communication Patterns

```rust
// Pattern 1: Atomic snapshot query (omsi-sim)
impl HumanSimulation {
    pub fn snapshot_human(&self, key: HumanKey) -> Option<HumanSnapshot> {
        // No locks acquired; reads are atomic or use lock-free structures
        let human = self.humans.get(key.id)?;
        
        // Early generation check
        if human.generation != key.generation {
            return None;
        }
        
        // Clone minimal data for snapshot
        Some(HumanSnapshot {
            key,
            position: human.position.load(Ordering::Relaxed),  // AtomicVec3
            velocity_mps: human.velocity.load(Ordering::Relaxed),
            behavior: human.behavior.clone(),  // Small enum, cheap clone
            animation_phase: human.animation_phase.load(Ordering::Relaxed),
            skeleton: None,  // Deferred to separate call for E+
        })
    }
}

// Pattern 2: Message-passing override (omsi-script)
impl ScriptContext {
    pub fn apply_override(&mut self, override_req: VariableOverride) -> Result<(), ScriptError> {
        // Validate entity still exists
        if !self.validate_entity(override_req.entity_key) {
            return Err(ScriptError::EntityDespawned);
        }
        
        // Clamp to valid range
        let clamped_value = if let Some((min, max)) = override_req.clamp_range {
            override_req.value.clamp(min, max)
        } else {
            override_req.value
        };
        
        // Queue override; simulation thread applies on next tick
        self.override_queue.push(OverrideCommand {
            entity_key: override_req.entity_key,
            variable_name: override_req.name,
            value: clamped_value,
        });
        
        Ok(())
    }
}

// Pattern 3: Async GPU query (omsi-render)
impl RenderState {
    pub fn request_texture_readback(&mut self, handle: TextureHandle, mip: u8) -> ReadbackId {
        let id = self.next_readback_id();
        
        // Submit GPU copy command (non-blocking)
        let staging_buffer = self.staging_pool.allocate(size);
        self.encoder.copy_texture_to_buffer(texture, staging_buffer, size);
        
        // Register pending readback
        self.pending_readbacks.insert(id, PendingReadback {
            buffer: staging_buffer,
            submitted_frame: self.frame_id,
        });
        
        id  // Return immediately; poll later
    }
    
    pub fn poll_readback(&mut self, id: ReadbackId) -> Option<Vec<u8>> {
        let pending = self.pending_readbacks.get(&id)?;
        
        // Non-blocking poll
        if pending.buffer.is_ready() {
            let data = pending.buffer.read();
            self.pending_readbacks.remove(&id);
            Some(data)
        } else {
            None  // Still pending; try again next frame
        }
    }
}
```

---

### 9.7 Performance Budget Enforcement Across Crates

The **< 1.0 ms total overhead** budget is distributed across crates:

| Crate | Operation | Budget | Enforcement Strategy |
|:------|:----------|:-------|:---------------------|
| `omsi-geometry` | BVH broadphase query | < 0.4 ms | Spatial acceleration; early-out on miss |
| `omsi-geometry` | Narrowphase triangle raycast | < 0.2 ms | SIMD triangle intersection; candidate limit |
| `omsi-sim` | Human snapshot construction | < 0.1 ms | Atomic reads; no skeleton query in core path |
| `omsi-script` | Variable enumeration | < 0.1 ms | Cached HashMap clone; no disk I/O |
| `omsi-render` | Material metadata query | < 0.05 ms | Metadata only; no GPU sync |
| `omsi-ui` | Panel rendering (egui) | < 0.15 ms | Incremental layout; text caching |
| **Total** | **Full inspector update** | **< 1.0 ms** | Measured via `tracing` spans |

**Measurement Infrastructure:**
```rust
// crates/omsi-app/src/inspector/mod.rs
#[tracing::instrument(name = "inspector_update", skip_all)]
pub fn update_snapshot(&mut self, world: &World, render: &RenderState, sim: &SimState) {
    let _span = tracing::trace_span!("inspector_snapshot").entered();
    
    // Each subsystem wrapped in tracing spans
    let entity_span = tracing::trace_span!("snapshot_entity").entered();
    let entity = self.snapshot_entity(world);
    drop(entity_span);
    
    let render_span = tracing::trace_span!("snapshot_render").entered();
    let render_info = render.snapshot_material(self.selected);
    drop(render_span);
    
    // ... etc for each subsystem
    
    // Total time measured and logged; CI fails if > 1.0 ms
}
```

---

### 9.8 Integration Testing Strategy

Each cross-crate integration point must have corresponding tests:

```rust
// crates/omsi-app/tests/inspector_integration.rs

#[test]
fn test_human_raycast_snapshot_e2e() {
    let mut app = TestApp::new();
    app.spawn_human(Vec3::new(0.0, 0.0, 0.0));
    
    // Simulate raycast
    app.inspector.raycast(Ray { origin: Vec3::ZERO, direction: Vec3::Z });
    app.step_frame();
    
    // Verify snapshot constructed
    let snapshot = app.inspector.snapshot().unwrap();
    assert!(matches!(snapshot.entity, Some(EntitySnapshot::Human(_))));
}

#[test]
fn test_script_override_fail_closed() {
    let mut app = TestApp::new();
    let vehicle = app.spawn_vehicle();
    
    // Apply override
    app.inspector.select(EntityKey::Vehicle(vehicle));
    app.inspector.override_variable("L.throttle", 0.5);
    app.step_frame();
    
    // Despawn vehicle
    app.despawn_vehicle(vehicle);
    app.step_frame();
    
    // Verify override cleared automatically
    assert!(app.inspector.snapshot().is_none());
    assert_eq!(app.inspector.active_overrides().len(), 0);
}

#[test]
fn test_texture_readback_async() {
    let mut app = TestApp::new();
    let vehicle = app.spawn_vehicle();
    
    // Request texture dump
    app.inspector.select(EntityKey::Vehicle(vehicle));
    let readback_id = app.inspector.request_texture_dump();
    
    // Verify non-blocking
    assert!(app.inspector.poll_texture_dump(readback_id).is_none());
    
    // Advance 3 frames (GPU latency)
    app.step_frame();
    app.step_frame();
    app.step_frame();
    
    // Verify completion
    let pixels = app.inspector.poll_texture_dump(readback_id).unwrap();
    assert!(pixels.len() > 0);
}

#[test]
fn test_performance_budget() {
    let mut app = TestApp::new();
    
    // Stress test: 30 AI vehicles, 500 pedestrians, Spandau map
    app.load_map("maps/Spandau/global.cfg");
    for _ in 0..30 {
        app.spawn_ai_vehicle();
    }
    for _ in 0..500 {
        app.spawn_human();
    }
    
    // Measure inspector overhead
    let start = std::time::Instant::now();
    app.inspector.raycast(Ray { origin: Vec3::ZERO, direction: Vec3::Z });
    app.step_frame();
    let elapsed = start.elapsed();
    
    // Enforce < 1.0 ms budget
    assert!(elapsed.as_secs_f64() < 0.001, "Inspector overhead: {:?}", elapsed);
}
```

---

### 9.9 Migration Path from MVP to Full Scope-Creep

The MVP (Tasks 1–10) established core infrastructure. The scope-creep features build incrementally:

```
MVP (Baseline)              Phase 1                 Phase 2                 Phase 3
     |                          |                       |                       |
     v                          v                       v                       v
+----------+            +--------------+        +----------------+      +-----------------+
| Vehicle  |            | Human raycast|        | Script VM      |      | Material/Texture|
| Scenery  | ---------> | (Subsystem E)|------> | (Subsystem B)  |----> | (Subsystem A)   |
| raycast  |            | BVH accel    |        | Variable watch |      | Async GPU query |
| snapshot |            +--------------+        | Live overrides |      | PBR sandbox     |
+----------+                    |               +----------------+      +-----------------+
                                |                       |                       |
                                v                       v                       v
                         +--------------+        +----------------+      +-----------------+
                         | Penetration  |        | Time-travel    |      | Dynamic textures|
                         | stack (G)    |        | scrubbing (B+) |      | ScriptTexture   |
                         +--------------+        +----------------+      | live preview(A+)|
                                                                         +-----------------+

Phase 4                 Phase 5
     |                       |
     v                       v
+----------------+    +-----------------+
| Splines/Traffic|    | glTF export     |
| (Subsystem D)  |--> | WebSocket (I+)  |
| Audio/Env (H)  |    | Editor bridge(F)|
+----------------+    +-----------------+
```

**Migration Strategy:**
1. **Phase 1:** Add `omsi-geometry` BVH + human raycasting → No breaking changes.
2. **Phase 2:** Extend `omsi-script` with snapshot APIs → Additive API surface.
3. **Phase 3:** Add `omsi-render` async staging pool → Parallel infrastructure.
4. **Phase 4:** Integrate `omsi-map` spline queries → New subsystem, no conflicts.
5. **Phase 5:** Add export/telemetry → Pure additions, no core changes.

---

### 9.10 Summary: Integration Surface Checklist

For each new inspector feature, verify:

- [ ] **Generational key validation:** All entity snapshots check generation before access.
- [ ] **Lock-free snapshot:** No `Mutex`/`RwLock` guards held across frame boundaries.
- [ ] **Async GPU queries:** Texture/VRAM reads use staging pools; no blocking waits.
- [ ] **Fail-closed mutations:** Script overrides clamp ranges; despawn revokes edits.
- [ ] **Single tile pin:** At most one scenery tile pinned at any time.
- [ ] **Performance budget:** < 1.0 ms total overhead measured via `tracing`.
- [ ] **Thread safety:** Snapshot construction serialized at frame sync; UI thread never blocks sim/render.
- [ ] **Integration tests:** Cross-crate interaction tested in `crates/omsi-app/tests/`.

---

**End of Section 9: Cross-Crate Dependencies & Integration Surface**
