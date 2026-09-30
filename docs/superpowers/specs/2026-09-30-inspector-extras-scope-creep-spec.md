# openOMSI Unified Visual Debug Inspector: Advanced Extras & Scope-Creep Specification

**Target Workspace:** `openOMSI` (`lmaodick1239/openOMSI` / `turbo-devv/openOMSI`)  
**Document Status:** Architectural Specification & Evolutionary Scope-Creep Blueprint  
**Date:** 2026-09-30  
**Baseline Commit:** `e8f101c` (Inspector MVP Tasks 1–10 Merged + Upstream Sync Complete)  
**Target Crates:** `omsi-app`, `omsi-render`, `omsi-sim`, `omsi-geometry`, `omsi-model`, `omsi-scenery`

---

## 1. Executive Summary & Strategic Motivation

### 1.1 Baseline Status & Transition
The **Unified Visual Debug Inspector MVP** successfully established a rock-solid, read-only foundation in `openOMSI`:
- Dedicated toggleable inspector mode (`Ctrl+I`) with strict input precedence.
- Precise, raycasted mesh selection for vehicle parts (player, AI, remote, articulated trailers) and placed scenery instances.
- Generational key tracking (`VehicleKey`, `SceneryKey`) preventing handle aliasing upon despawn or stream reload.
- Safe, reference-counted single-tile pin leases preventing streaming unloads of selected scenery.
- Compact, responsive, multi-lingual side panel rendering world/local transforms, bounding boxes, and asset paths.
- Clean integration with 53 upstream commits from `turbo-devv/openOMSI` (commit `e8f101c`).

### 1.2 The Scope-Creep Mandate: "From Static Geometry to Simulation Observatory"
While the MVP solved visual entity identification, debugging a high-fidelity bus and transit simulator demands far more than static transform inspection. OMSI 2 modding and map creation are notorious for invisible points of failure:
- Cryptic `.osc` script state machines silently failing door interlocks or pneumatic pressure builds.
- Complex multi-pass PBR materials and dynamic textures (scripttextures, texttextures, LED matrix displays) misbehaving due to bad alpha thresholds or missing mipmaps.
- Pedestrian navigation breakdowns where passenger agents fail to pathfind across bus doors or depot queues.
- Spline connectivity and road traffic path errors causing AI gridlock.
- Mispositioned 3D audio cones and acoustic attenuation bugs.

This specification elevates the approved follow-on extras (**A, B, C, E, G, I**) and strategically integrates deferred high-value targets (**D: Splines & Paths, H: Audio & Environment, F: Editor Bridge**). We "scope creep the scope creep" by designing not merely a list of ad-hoc features, but a **unified, interactive simulation diagnostic lab and authoring bridge**—built with rigorous performance guarantees, zero GPU stalls, thread-safe snapshot boundaries, and fail-closed safety semantics.

---

## 2. Core Architectural Guarantees & Constraints

All features detailed in this specification must conform strictly to four architectural pillars:

```
+------------------------------------------------------------------------------------------+
|                               FOUR INVIOLABLE PILLARS                                    |
+------------------------------------------------------------------------------------------+
| 1. ZERO-STALL GPU PIPELINE                                                               |
|    - Texture reads, frame graph inspects, and VRAM queries must use async staging pools. |
|    - No synchronous `wgpu::Device::poll(Wait)` calls on the simulation or render thread. |
+------------------------------------------------------------------------------------------+
| 2. SNAPSHOT ISOLATION & LOCK FREEDOM                                                     |
|    - The UI layer NEVER directly holds `Mutex` or `RwLock` guards across frames.         |
|    - Snapshot construction is an atomic, single-pass query at the frame synchronization   |
|      boundary (`app_events.rs`), producing owned, UI-safe data structures.               |
+------------------------------------------------------------------------------------------+
| 3. FAIL-CLOSED RUNTIME INTEGRITY                                                         |
|    - All mutation capabilities (live script overrides, editor promotions) are sandboxed. |
|    - Instant 1-click rollback, bounded range validation, and explicit transactional logs.|
|    - Despawn, network disconnect, or tile unload instantly revokes live edit leases.     |
+------------------------------------------------------------------------------------------+
| 4. FRAME TIME BUDGET: < 1.0 ms TOTAL OVERHEAD                                            |
|    - Baseline active inspector overhead must not exceed 1.0 ms per frame.                |
|    - Spatial acceleration (BVH) bounds raycasting to < 0.4 ms even in 100k-triangle maps.|
|    - Zero cost (0.0 ms) when inspector mode is toggled off (`Ctrl+I`).                   |
+------------------------------------------------------------------------------------------+
```

---

## 3. Detailed Subsystem Specifications

```
                              EXTENDED INSPECTOR ARCHITECTURE
                             
                     +---------------------------------------+
                     |         Cursor Input / Raycast        |
                     +---------------------------------------+
                                         |
                                         v
                     +---------------------------------------+
                     |   Broadphase Spatial Acceleration     |
                     |   (Dynamic BVH / Candidate Collector) |
                     +---------------------------------------+
                                         |
                       +-----------------+-----------------+
                       |                                   |
                       v                                   v
             [Vehicle & Trailer Parts]           [Placed Scenery Objects]
             [Pedestrian / Human Mesh]           [Road Splines & Paths]
             [Audio Emitter Nodes]               [Light & Weather Probes]
                       |                                   |
                       +-----------------+-----------------+
                                         |
                                         v
                     +---------------------------------------+
                     |      Multi-Hit Penetration Stack      |
                     |  (Distance-Sorted Candidate Corridor) |
                     +---------------------------------------+
                                         |
                                         v
                     +---------------------------------------+
                     |       Atomic Snapshot Builder         |
                     |   - Generational Identity Validation  |
                     |   - Scenery Single-Tile Pin Lease     |
                     +---------------------------------------+
                                         |
       +---------------+-----------------+---------------+---------------+
       |               |                 |               |               |
       v               v                 v               v               v
  [Material &     [Script VM &     [Render Pass     [Kinematics &   [Diagnostic
   Dynamic Tex]    Watch Sandbox]   Isolation]       Pose Graph]     Telemetry]
```

---

### 3.1 Subsystem E & E+: Human, Pedestrian & Skeletal Kinematics Inspector
*Status: Promoted from Extra E. Expands pedestrian raycasting into full crowd and kinematics telemetry.*

#### Core Mechanics (E)
1. **Selection & Handle:**
   - Define `HumanKey { id: usize, generation: u64, is_driver: bool }`.
   - Broadphase: Fast bounding cylinder intersection $(r \approx 0.35\text{m}, h \approx 1.85\text{m})$.
   - Narrowphase: Triangle-level raycast against current animated skeletal pose (derived from `omsi-sim::human` and `crates/omsi-app/src/humans.rs`).
2. **Snapshot Data:**
   - Pedestrian unique ID, character model file path (`.hum`).
   - Current world position, heading angle, walking velocity ($\text{m/s}$).
   - High-level behavior state: `Wandering`, `WalkingToStop`, `WaitingAtStop`, `BoardingBus`, `BuyingTicket`, `FindingSeat`, `Seated`, `Alighting`.

#### Advanced Scope-Creep Tier (E+)
1. **Skeletal Joint & Bone Visualization:**
   - 3D overlay displaying the active bone hierarchy (Spine, Neck, Head, Clavicles, Arms, Legs, Feet) connected by color-coded debug lines.
   - Rotational Euler angles displayed per joint in a collapsible tree view in the inspector panel.
2. **Animation Clip & State Machine Telemetry:**
   - Real-time display of active animation clips (e.g. `walk.ani`, `sit_down.ani`, `idle_shiver.ani`).
   - Animation parameters: Normalized phase time $[0.0..1.0]$, playback speed scalar, blend weight between concurrent animation layers.
   - *Time-Freeze / Step Control:* Inspector UI affordance to pause pedestrian animation, scrub forward/backward frame-by-frame, or force $0.1\times$ slow-motion to inspect foot-sliding or door-clipping artifacts.
3. **Navigation Mesh & Path Intent Vector:**
   - 3D visual projection of the agent's planned path:
     - Target waypoint coordinate, destination bus stop marker, and current steering vector (Cyan arrow).
     - Desired path line projected onto tile terrain.
     - Collision avoidance repulsion vector when navigating around other passengers or static obstacles.
4. **Passenger Economy & Disposition Telemetry:**
   - Inspect internal psychological and financial state of passengers boarded on the player's bus:
     - Ticket transaction status: Requested ticket type (e.g., *Normal Fahrschein*, *Kurzstrecke*), offered cash amount, pending change calculation.
     - Comfort & Satisfaction Index ($0..100\%$): Degraded by rough driving, excessive lateral G-forces, cold cabin temperature ($< 18^\circ\text{C}$), or lack of fresh air.
     - Desired destination stop name and planned alighting request status (`passenger_say_stop_requested`).

---

### 3.2 Subsystem A & A+: Material, Texture, Shader & Dynamic Display Inspector
*Status: Promoted from Extra A (Phases A1, A2, A3). Elevates surface diagnostics to a real-time graphics laboratory.*

#### Core Mechanics (A1–A3)
1. **A1: Material Properties Readout:**
   - Display shader variant name (e.g. `enhanced.wgsl`, `shader.wgsl`), draw call index, render pass classification (Opaque, Transparent, Cutout, Shadow).
   - Display texture coordinates, UV scale/bias factors, blend modes (`Alpha`, `Additive`, `Multiply`), double-sided flag, alpha-test cutoff value.
2. **A2: Safe Texture Preview:**
   - Inspect diffuse/albedo, normal/bump, roughness, metallic, and specular reflection maps.
   - Display texture metadata: Format (`BC7`, `BC3`, `BC1`, `RGBA8Unorm`), native pixel resolution, aspect ratio, mipmap count, compressed VRAM byte size.
   - Mini thumbnail rendered in the UI panel using non-blocking texture binding.
3. **A3: Texture Dump & Mipmap Explorer:**
   - Single-click action: "Export Texture to PNG" saving to `Screenshots/inspector_dump_<name>.png` via async readback.
   - Interactive Mipmap Slider: Force the renderer to isolate individual mip levels ($0, 1, 2, \dots$) on the selected mesh to evaluate texture filtering and shimmering.

#### Advanced Scope-Creep Tier (A+)
1. **Dynamic Display & ScriptTexture Live Visualizer:**
   OMSI vehicles dynamically render passenger displays, matrix rollbands, ticket printers, and IBIS monitors directly into memory textures via `[scripttexture]` and `[texttexture]` blocks:
   - Live real-time UI preview of dynamic textures updating at full simulation rate (e.g., ticking digital clocks, scrolling destination text).
   - Interactive Pixel-Grid Zoom ($1\times, 2\times, 4\times, 8\times$) with pixel coordinate and hex color readout under the mouse cursor.
   - Diagnostic text overlay showing the backing font configuration file, character raster dimensions, and string source variables (e.g. `$IBIS_terminus_name`).
2. **Real-Time PBR Parameter Sandbox:**
   - Non-destructive, live property sliders in the inspector panel:
     - Roughness override $[0.0..1.0]$
     - Metalness override $[0.0..1.0]$
     - Normal map intensity multiplier $[0.0..2.0]$
     - Environment reflection intensity $[0.0..3.0]$
     - Emissive intensity boost (evaluating night glow of gauges and dials)
   - Interactive Split-Screen Wipe: A vertical divider across the mesh allowing direct comparison between original material values and sandboxed overrides.
   - "Copy Configuration Block" button: Formats modified values directly into OMSI `.cfg` syntax (e.g., `[matl]`, `[matl_envmap]`, `[matl_roughness]`) to clipboard for immediate modder authoring.
3. **Texture Memory & Mipmap Heatmap Mode:**
   - Full-scene diagnostic shader pass (`OMSI_DEBUG_ENHANCED=18` / Inspector toggle) color-coding surface texel density against screen pixel density:
     - Blue: Undersampled (texture resolution too low, blurry).
     - Green: Optimal 1:1 texel-to-pixel ratio.
     - Red: Oversampled (texture resolution wastefully high, causing cache thrashing and moiré).
4. **WGSL Shader Uniform & Binding Inspector:**
   - Live inspection of GPU uniform buffers tied to the active draw call:
     - Model-View-Projection matrix, camera eye position, solar directional vector, ambient sky color.
     - Material flags bitfield (determining alpha testing, envmap reflection, and lighting calculation paths).
   - Live Hot-Reload Trigger: Hot-reloads WGSL shader files from disk without restarting the application, immediately re-binding pipelines and reflecting shader code edits.

---

### 3.3 Subsystem B & B+: OMSI Script VM, Variable Watcher, Live Override & Time-Travel Debugger
*Status: Promoted from Extra B (Phases B1, B2, B3). Extends script variable reads into a full virtual machine telemetry suite.*

#### Core Mechanics (B1–B3)
1. **B1: Read-Only Variable Inspector:**
   - Enumerate all floating-point and string variables (`L.var`, `S.var`) associated with the selected vehicle or scripted scenery object.
   - Categorize variables by origin script file (e.g., `engine.osc`, `door.osc`, `cockpit.osc`, `matrix.osc`).
   - Real-time search/filter input with instant debounce.
2. **B2: Gated Live Variable Overrides:**
   - Editable numeric and string fields beside variable names.
   - Clamped validation based on known physical bounds (e.g., throttle $[0.0..1.0]$, door state $[0.0..1.0]$, engine RPM $[0.0..3000.0]$).
   - Safety isolation: Live edits mutate only in-memory simulation state; source files on disk are never touched.
   - Prominent, persistent "Reset All Overrides" button that immediately purges artificial values and resynchronizes with physical simulation equations.
3. **B3: Watch Expression Table:**
   - Dedicated watch panel where users pin up to 16 variables of interest.
   - Supports computed mathematical expressions:
     $$\text{expression} = (\text{L.cockpit\_speed} \times 3.6) + \text{L.engine\_temperature} / 2.0$$
   - Color-coded value changes: Green flash on increment, Red flash on decrement, Gray when static.

#### Advanced Scope-Creep Tier (B+)
1. **Variable Sparklines & Mini-Oscilloscope HUD:**
   - Embedded real-time sparkline graphs (60–120 historical samples) rendered directly in the inspector watch table.
   - Visualizes oscillations, damping curves, and peak transients (critical for pneumatic suspension leveling, ABS wheel-slip pulsing, and transmission gearshift RPM matching).
   - Displays running statistics: Min, Max, Mean, and $\Delta/\text{second}$.
2. **Script VM Call Stack & Opcode Execution Tracer:**
   OMSI uses a reverse Polish notation (RPN) stack-based bytecode interpreter:
   - Live visualization of the RPN Evaluation Stack: View the top 8 elements of the float stack (`[ 42.0, 1.25, 0.0, ... ]`) during frame evaluation.
   - Active Macro & Trigger Monitor: Displays which macro (`{macro:Engine_Calc}`) or trigger (`{trigger:door_front_open}`) is currently firing.
   - Breakpoint / Trap On Condition:
     - Set non-halting trigger traps: e.g. "Notify when `L.door_state` exceeds $1.0$" or "Log when `L.air_pressure` drops below $5.5\text{ bar}$".
     - Captures an instant diagnostic snapshot of all vehicle variables at the moment the trap condition evaluates to true.
3. **Visual Variable Dependency Graph (Reactive DAG):**
   - Interactive node graph showing variable propagation:
     ```
     [L.cockpit_throttle] ---> [L.engine_injection_rate] ---> [L.engine_rpm]
                                                                     |
                               +-------------------------------------+
                               |
                               v
                     [L.sound_pitch_engine] ---> [omsi-audio mixer]
     ```
   - Instantly highlights upstream causes when an output variable fails to respond.
4. **Simulation Time-Travel Scrubbing Buffer:**
   - Maintains a high-performance in-memory ring buffer of the last 10 seconds of vehicle state (600 simulation ticks at 60 Hz).
   - *Scrubbing Toolbar:* Pause the simulation, drag a time slider backwards, and inspect the exact variable state, throttle position, door switches, and air pressures that caused a bug (e.g., identifying why the bus failed to engage first gear 3 seconds ago).

---

### 3.4 Subsystem C & C+: Render Pass Frame Graph, Mesh Isolation & Collision Hull Visualizer
*Status: Promoted from Extra C (Phases C1, C2). Converts render controls into a complete graphics debugging suite.*

#### Core Mechanics (C1–C2)
1. **C1: Render Pass & Wireframe Toggles:**
   - Selectively toggle render passes: Sun Shadow Cascades (0, 1, 2, 3), SSAO Occlusion, Opaque Forward/PBR, Alpha-Blended Glass, Tonemapping & FXAA.
   - Global or entity-scoped wireframe overlay using `wgpu::PolygonMode::Line`.
2. **C2: Mesh-Level Isolation:**
   - Single-click action: "Isolate Selected Mesh" (hides every other mesh instance in the entire scene, rendering the selected part against a neutral clear color).
   - Single-click action: "Hide Selected Mesh" (temporarily suppresses rendering of the chosen mesh instance to look inside enclosed assemblies like engine blocks or gearboxes).
   - Automatic cleanup: All visibility overrides reset instantly upon exiting inspector mode (`Ctrl+I`) or selecting a different entity.

#### Advanced Scope-Creep Tier (C+)
1. **Visual Render Pass Frame Graph:**
   - Hierarchical UI tree displaying the GPU execution sequence for the current frame:
     ```
     [Frame #18420 - Total GPU: 8.42ms]
       ├── [Pass 0] Shadow Map Cascade 0 (2048x2048) ............. 0.85ms
       ├── [Pass 1] Shadow Map Cascade 1 (2048x2048) ............. 0.72ms
       ├── [Pass 2] SSAO Depth Prepass (1920x1080) ............... 1.10ms
       ├── [Pass 3] SSAO Compute & Bilateral Blur ................ 0.95ms
       ├── [Pass 4] Main Forward PBR Pass (142 Draw Calls) ....... 3.80ms
       │     ├── Opaque Terrains (12 calls) ...................... 0.65ms
       │     ├── Scenery Meshes (84 calls) ....................... 1.85ms
       │     └── Vehicle Meshes (46 calls) ....................... 1.30ms
       ├── [Pass 5] Glass / Transparent Blend Pass ............... 0.60ms
       └── [Pass 6] Post-Processing (Tonemap, Bloom, FXAA) ....... 0.40ms
     ```
   - Each pass features an independent freeze, bypass, and GPU microsecond execution meter.
2. **"Ghost / X-Ray" Focus Mode:**
   - Rather than harshly hiding unselected geometry, the inspector dims all unselected world elements to $15\%$ luminance with a cool monochrome desaturation tint.
   - The selected entity renders in full PBR brilliance with a glowing silhouette edge, providing spatial context without visual clutter.
3. **Collision Hull vs Render Mesh Discrepancy Overlay:**
   - Renders the physical simulation bounding shapes (`omsi-sim::rigid`) concurrently with the visual rendering geometry:
     - Chassis box collision hull (Cyan wireframe).
     - Wheel collision cylinders and ray probes (Yellow wireframe).
     - Scenery collision boxes and ground contact planes (Magenta wireframe).
   - Instantly reveals collision discrepancies, such as bus bumpers clipping through curbs before physical contact or ground clearance height mismatches.
4. **Surface Normal, Tangent & UV Seam 3D Visualizer:**
   - Geometry debug shader pass projecting 3D vector lines directly from mesh vertices:
     - Blue spikes: Surface normals ($\vec{N}$).
     - Red spikes: Tangent vectors ($\vec{T}$).
     - Green spikes: Bitangents ($\vec{B}$).
   - Highlights texture UV seam discontinuities in bright orange, instantly pinpointing normal-mapping distortion and smoothing group seams.

---

### 3.5 Subsystem G & G+: Penetration Stacks, Depth Peeling & Hierarchical Breadcrumbs
*Status: Promoted from Extra G (Phases G1, G2, G3). Solves selection occlusion and multi-layered geometry picking.*

#### Core Mechanics (G1–G3)
1. **G1: Multi-Entity Hit List (Penetration Corridor):**
   - Cursor raycast intersects every bounding volume along its infinite forward path.
   - Collects all valid hits sorted strictly by distance from camera near plane.
   - Inspector panel displays an interactive vertical list:
     ```
     [Hit Corridor: 5 Entities Penetrated]
       1. [0.82m] Bus Windshield Glass (MAN_SD200/windshield.o3d)
       2. [1.15m] Wiper Blade Left (MAN_SD200/wiper_L.o3d)
       3. [1.85m] Dashboard Switch (MAN_SD200/sw_hazard.o3d)
       4. [2.40m] Driver Seat Cushion (MAN_SD200/seat_driver.o3d)
       5. [14.2m] Ground Asphalt Spline (Terrain Tile 4850, 3170)
     ```
   - Clicking any row instantly focuses and pins that specific entity.
2. **G2: Cycle-Through Hotkey (`Tab` / Wheel Scroll):**
   - While hovering over geometry with the cursor, pressing `Tab` or `Shift+MouseWheel` cycles selection progressively down the hit corridor without moving the mouse.
   - Selected entity is outlined with a depth-tested highlight; occluding objects in front are rendered with semi-transparent stipple dithering.
3. **G3: Parent-Child Navigation:**
   - Affordances to navigate the logical hierarchy:
     - "Select Parent Vehicle" (from a wheel or switch).
     - "Select Coupled Trailer" / "Select Articulation Section".
     - "Select All Similar Instances" (highlights all identical scenery models placed on the current map tile).

#### Advanced Scope-Creep Tier (G+)
1. **3D Volumetric Ray Penetration Corridor:**
   - In-world debug visualizer rendering the cursor ray as a thin glowing laser beam with small translucent spheres at each intersection point.
   - Renders entry and exit points for thick volumes (e.g. bus cabins and building interiors).
2. **Interactive "Onion-Skin" Depth Peeling Slider:**
   - A smooth UI slider $(0..100\%)$ that slices away geometry layers starting from the camera position along the view vector.
   - Peeling away the bus exterior skin cleanly exposes internal wiring, HVAC conduits, seat frames, passenger placement points, and drivetrain components.
3. **Interactive Breadcrumb Navigation Bar:**
   - Sticky header bar at the top of the inspector panel showing full scene hierarchy:
     $$\text{World} \longrightarrow \text{Tile (4850, 3170)} \longrightarrow \text{Vehicle \#0 (MAN SD200)} \longrightarrow \text{Hitch Joint} \longrightarrow \text{Trailer Body} \longrightarrow \text{Door 2} \longrightarrow \text{Glass Pane}$$
   - Clicking any ancestor breadcrumb instantly elevates the inspection target and reconfigures the property panel.
4. **Sub-Pixel Proximity Ray Snapping:**
   - Addresses the notorious problem of selecting ultra-thin geometry (catenary tram overhead wires, telephone poles, radio antennae, wiper needles).
   - When exact triangle raycast misses by less than 8 screen pixels, snaps to the nearest line or slender bounding capsule, rendering a cyan guide reticle.

---

### 3.6 Subsystem I & I+: Persistent Layouts, Scripted Automation, CI Diagnostics & WebSocket Telemetry
*Status: Promoted from Extra I (Phases I1, I2, I3). Connects the inspector to modding workflows, CI automation, and external tools.*

#### Core Mechanics (I1–I3)
1. **I1: Persistent Panel State:**
   - Serializes panel geometry (screen position, width, collapsed/expanded sections, active tab) to `~/.config/openomsi/inspector_layout.json`.
   - Remembers user watch tables and filter search strings across game restarts.
2. **I2: Scripted Automation Hooks:**
   - Full integration with `OMSI_INPUT` automation command grammar:
     ```
     OMSI_INPUT="t=1.0 inspect_select 400,300; t=1.5 inspect_assert mesh=='tacho.o3d'; t=2.0 inspect_set_var L.throttle=0.85; t=2.5 shot test_pass.png"
     ```
   - Enables headless automated testing of cockpit animations and scenery placement.
3. **I3: Multi-Format Data Export:**
   - Export selected entity diagnostic report to JSON, CSV, or YAML with a single click.
   - Output includes world coordinates, orientation quaternions, mesh statistics, material definitions, and current script state.

#### Advanced Scope-Creep Tier (I+)
1. **Single-Click glTF 2.0 Export (`.glb`):**
   - Direct export button: "Export Selection to glTF".
   - Bakes the selected vehicle part or scenery object—including resolved world/local transforms, vertex colors, PBR material properties, and embedded PNG textures—into a standard `.glb` file.
   - Modders can instantly import the exported model into Blender to inspect mesh geometry, UV mapping, or pivot alignments in an external DCC tool.
2. **Headless Map & Asset Validation CLI (`--inspect-check`):**
   - Command-line diagnostic validation tool executing without an active window:
     ```bash
     openomsi --inspect-check maps/Grundorf/global.cfg --report grundorf_audit.json
     ```
   - Automated audit passes:
     - Detects missing or corrupted `.o3d` mesh files.
     - Detects missing textures and uncompressed high-resolution textures.
     - Identifies broken `.osc` script variables (variables written but never initialized or read).
     - Finds unpinned scenery objects with invalid coordinate transforms or inverted normals.
3. **Real-Time Remote Telemetry WebSocket Server:**
   - Optional diagnostic server (`--telemetry-port 9002`):
     - Broadcasts live JSON packets at 30 Hz containing vehicle velocity, engine RPM, gear status, door states, selected entity properties, and watch table values.
     - Modders and virtual bus companies can connect web dashboards, hardware gauge clusters, or secondary tablet displays across the local network without modifying the engine core.
4. **Session Trace Recording & Regression Diffing:**
   - Record an entire driving run into a compact `.omsi-trace` binary stream.
   - In-game "Trace Diff" tool: Replay a recorded trace side-by-side with live simulation to detect physics regressions, animation timing drifts, or script divergence after code modifications.

---

### 3.7 Subsystem D & D+: Road Splines, Traffic Graph, Path Network & Signal Phase Inspector
*Status: Promoted from Deferred Register D. Unlocks the infrastructure backbone of OMSI maps.*

#### Core Mechanics (D)
1. **Spline Geometry Raycasting:**
   - Define `SplineKey { tile_x: i32, tile_y: i32, spline_id: usize }`.
   - Raycast directly against extruded road spline surfaces (`.sli`) using `omsi-geometry` cross-section bounds.
   - Display spline profile path, segment length ($\text{m}$), elevation gradient ($\%$, pitch), banking/cant angle, and horizontal radius of curvature.

#### Advanced Scope-Creep Tier (D+)
1. **Traffic Path Network Overlay:**
   - 3D visual projection of vehicle traffic paths running across the selected road spline:
     - Directional vector lines color-coded by traffic group (Cars, Buses only, Trucks, Trams, Pedestrians).
     - Active speed limit indicators displayed above path nodes ($30, 50, 80\text{ km/h}$).
     - Overtaking permission flags, lane-change links, and merging yield priorities.
2. **Traffic Signal Phase & Detector Loop Probe:**
   - Clicking a traffic light or road junction displays the active Signal Controller state:
     - Current signal phase (Rot, Rot-Gelb, Grün, Gelb) with countdown timer to next phase change.
     - Active vehicle detector loop boxes projected in 3D onto the road surface (highlighting green when occupied by a vehicle chassis).
     - Signal program schedule matrix and priority override triggers for approaching scheduled buses.
3. **Spline Connection & Discontinuity Auditor:**
   - Visual inspection of spline joints:
     - Detects height mismatches ($> 1\text{mm}$) between adjoining splines causing vehicle tire hopping.
     - Identifies angular kinks ($> 0.5^\circ$) that break AI path following.

---

### 3.8 Subsystem H & H+: Spatial Audio Emitters & Environmental Weather Probes
*Status: Promoted from Deferred Register H. Illuminates invisible acoustics and climate variables.*

#### Core Mechanics (H)
1. **3D Sound Source Emitter Visualization:**
   - Enumerate all spatial sound instances generated by `omsi-app` and `omsi-audio` (engine intake/exhaust, transmission whine, turbocharger spool, air brake hiss, door pneumatic solenoids, curved-track wheel screech).
   - 3D wireframe sphere positioned at sound source origin in vehicle-local or world space.

#### Advanced Scope-Creep Tier (H+)
1. **Acoustic Cone & Attenuation Range Spheres:**
   - Visual representation of audio propagation parameters:
     - Inner radius sphere (full volume, $1.0\times$ gain).
     - Outer radius sphere (fade to cutoff).
     - Directional sound cone geometry (for horn and exhaust orientation).
   - Real-time readout of active audio variables: Current volume decibels ($\text{dB}$), frequency pitch modulation factor ($0.5\times..2.0\times$), Doppler frequency shift.
2. **Environmental & Climate Surface Probe:**
   - Inspect environmental conditions affecting the selected entity's location:
     - Solar direct irradiance ($\text{W/m}^2$), diffuse sky irradiance, sun azimuth and altitude angles.
     - Surface wetness factor $[0.0..1.0]$ and dynamic tire adhesion friction coefficient ($\mu$).
     - Ambient air temperature, road surface temperature, wind direction vector, and rain precipitation intensity.

---

### 3.9 Subsystem F & F+: Bidirectional Inspector-to-Editor Workflow & Non-Destructive Tweaking
*Status: Promoted from Deferred Register F. Seamlessly connects inspection with the Scenery Object Editor.*

#### Core Mechanics (F)
1. **Seamless Mode Handover:**
   - When inspecting a placed scenery object, a single UI button **"Promote to Object Editor"** executes a graceful, atomic transition:
     - Retains the exact selected object instance.
     - Deactivates Inspector Mode (`Ctrl+I`).
     - Activates Object Editor Mode (`Ctrl+Shift+E`).
     - Automatically pins the editor gizmo to the object, preserving user context without requiring re-selection.

#### Advanced Scope-Creep Tier (F+)
1. **Non-Destructive Transform Sandbox:**
   - Allows temporary translation ($X, Y, Z$) and rotation (Yaw, Pitch, Roll) testing directly within the inspector panel.
   - Shows live delta offsets from original map placement.
   - "Revert" button instantly snaps the object back to map coordinates.
   - "Commit to Map" button pushes modified coordinates into the editor undo/save queue (`tile_x_y.map`).

---

## 4. Unified UI/UX Architecture & Layout System

The extended inspector interface organizes these powerful tools into a compact, responsive, docked side panel:

```
+------------------------------------------------------------------------------------------+
|  UNIFIED VISUAL DEBUG INSPECTOR                                          [Pin] [X] Close |
+------------------------------------------------------------------------------------------+
|  Selected: "MAN_SD200/model/cockpit_speedo.o3d"                                          |
|  Parent:   Vehicle #0 (MAN SD80) | Class: Lead Vehicle (Player)                          |
+------------------------------------------------------------------------------------------+
|  [Transforms] | [Material/Tex] | [Script VM] | [Render/Pass] | [Kinematics] | [Audio/Env] |
+------------------------------------------------------------------------------------------+
|  WORLD POSE                                                                              |
|    Pos:  [ 4852.124, -318.450,   4.215 ] m                                               |
|    Rot:  Yaw 182.40°, Pitch -1.25°, Roll 0.00°                                           |
|  LOCAL POSE (Chassis Basis)                                                              |
|    Pos:  [    0.420,    1.150,   0.850 ] m                                               |
|    Rot:  Pitch 48.20° (Animated via `L.cockpit_speedo`)                                  |
|                                                                                          |
|  BOUNDS & MESH METRICS                                                                   |
|    Vertices: 1,420 | Triangles: 2,140 | Radius: 0.185 m                                  |
|    Asset: "Vehicles/MAN_SD200/Model/model_SD80.cfg" [Line 412]                           |
+------------------------------------------------------------------------------------------+
|  MULTI-HIT CORRIDOR (Cycle: Tab / Scroll)                                                |
|    ► 1. [0.82m] cockpit_speedo.o3d (Selected)                                           |
|      2. [1.10m] dashboard_casing.o3d                                                     |
|      3. [2.40m] chassis_frame.o3d                                                        |
+------------------------------------------------------------------------------------------+
|  ACTIONS                                                                                 |
|    [X-Ray Focus]  [Isolate Mesh]  [Add Watch]  [Dump GLB]  [Promote to Editor]            |
+------------------------------------------------------------------------------------------+
```

### UI Design Principles
1. **Responsive Docking:** Clamps to left or right screen edge; automatically adapts from $1080\text{p}$ up to $4\text{K}$ high-DPI displays with physical-pixel scaling.
2. **Strict Mouse Passthrough Prevention:** Panel bounding region consumes all mouse hover, click, and scroll events; clicks within the UI rect never pick or alter world geometry.
3. **Complete Internationalization (i18n):** All labels, tabs, status indicators, and tooltips draw from `crates/omsi-app/locales/app.yml` across all 24 supported languages.

---

## 5. Phased Implementation Roadmap & SDD Task Graph

Following the Subagent-Driven Development (SDD) paradigm, the scope creep is partitioned into five distinct, sequential phases with rigorous review checkpoints:

```
[Phase 1: Raycast & Spatial Corridor]
   ├── Task 1.1: Dynamic BVH Spatial Acceleration
   ├── Task 1.2: Multi-Hit Penetration Stack & Ray Corridor (G1, G2)
   └── Task 1.3: Pedestrian / Human Raycasting & Key Model (E Core)
           |
           v
[Phase 2: Simulation Observability & Script VM]
   ├── Task 2.1: Human Kinematics, Poses & Pathfinding Vectors (E+)
   ├── Task 2.2: Read-Only Script Variable Inspector & Search (B1)
   ├── Task 2.3: Sandboxed Variable Overrides & Clamping (B2)
   ├── Task 2.4: Sparklines, RPN Stack Tracer & Watch Tables (B3, B+)
   └── Task 2.5: Simulation Time-Travel Scrubbing Buffer (B+)
           |
           v
[Phase 3: Material, Textures & Render Pipeline]
   ├── Task 3.1: Material Metadata & Async Texture Previews (A1, A2)
   ├── Task 3.2: Dynamic ScriptTexture / TextTexture Live Zoomer (A+)
   ├── Task 3.3: Texture Mipmap Heatmap & VRAM Profiler (A3, A+)
   ├── Task 3.4: Render Pass Frame Graph & Mesh Isolation (C1, C2)
   └── Task 3.5: Ghost/X-Ray Mode & Collision Hull Overlays (C+)
           |
           v
[Phase 4: World Infrastructure, Audio & Environment]
   ├── Task 4.1: Road Spline Geometry & Cross-Section Probe (D Core)
   ├── Task 4.2: Traffic Path Graph & Signal Phase Timers (D+)
   ├── Task 4.3: 3D Sound Emitters & Acoustic Cones (H Core, H+)
   └── Task 4.4: Ambient Weather & Friction Surface Probe (H+)
           |
           v
[Phase 5: Automation, Persistence, Export & External Telemetry]
   ├── Task 5.1: Persistent Layouts & User Preferences (I1)
   ├── Task 5.2: Automated Script Testing via OMSI_INPUT (I2)
   ├── Task 5.3: Single-Click glTF 2.0 Export (`.glb`) (I3, I+)
   ├── Task 5.4: Remote Telemetry WebSocket Server (I+)
   └── Task 5.5: Non-Destructive Editor Handover Bridge (F, F+)
```

---

## 6. Comprehensive Verification Matrix & Acceptance Criteria

| Feature Area | Verification Scenario | Target Success Criteria |
| :--- | :--- | :--- |
| **Spatial Picking (G, E)** | Raycast into crowded bus interior with passengers and driver. | Generates ordered multi-hit corridor; `Tab` cycles hits cleanly; raycast execution $< 0.4\text{ ms}$. |
| **Human Kinematics (E+)** | Inspect animated pedestrian boarding bus and scanning ticket. | Visualizes bone links, animation phase $[0..1]$, and destination stop node without jitter or lock contention. |
| **Script VM Sandbox (B, B+)** | Live override of `L.cockpit_throttle` and `L.door_state`. | Value clamps to valid range; bus physics responds; "Reset Overrides" instantly restores simulation authority. |
| **Time-Travel Scrubbing (B+)** | Scrub 5 seconds backward after deliberate engine stall. | Accurately reconstructs historical variable states; zero memory growth beyond allocated ring buffer. |
| **Dynamic Textures (A+)** | Inspect rolling destination blind and digital IBIS display. | Live updating thumbnail; pixel-grid zoom shows crisp font rendering; zero GPU frame stalls. |
| **PBR Tweak Sandbox (A+)** | Modify roughness and metalness on bus exterior body panels. | Split-screen wipe shows immediate visual comparison; zero permanent mutation of disk assets. |
| **Render Isolation (C, C+)** | Activate "X-Ray Focus" and "Isolate Mesh" on speedo needle. | Surrounding scene dims to $15\%$; target mesh renders clearly in wireframe/solid; clean restore on exit. |
| **Collision Hull Overlay (C+)** | Inspect bus bumper approaching curb and uneven road spline. | Visualizes physical box and wheel cylinders concurrently with visual triangles; clipping is visible. |
| **Road Spline & Traffic (D+)** | Inspect busy intersection with traffic lights and AI paths. | Displays path lane vectors, speed limits, and traffic signal countdown timers in real time. |
| **Audio Cones (H+)** | Inspect rear diesel engine sound source while revving. | Visualizes inner/outer acoustic radius spheres; gain/pitch modulation updates smoothly. |
| **glTF Export (I+)** | Export selected articulated bus section to `.glb`. | Valid glTF 2.0 file created; opens cleanly in Blender with correct transforms, hierarchy, and textures. |
| **Headless CI Check (I+)** | Run `openomsi --inspect-check` on Spandau map. | Produces comprehensive JSON audit of broken paths, missing textures, and variable errors within 30s. |
| **Editor Bridge (F+)** | Click "Promote to Editor" on placed scenery building. | Seamlessly transitions into `Ctrl+Shift+E` object editor with building selected; no selection lost. |
| **Performance Budget** | Full stress test: 30 AI buses, Spandau map, heavy rain, all tabs. | Frame time overhead $< 1.0\text{ ms}$; zero frame drops; memory overhead $< 32\text{ MB}$. |

---

## 7. Safety, Invariant Preservation & Fail-Closed Protocols

1. **Stale Selection Handling:**
   Whenever an entity is despawned (AI traffic leaves streaming radius, remote player disconnects, pedestrian enters building), the inspector snapshot detects generational invalidation within 1 frame:
   - Immediately clears live overrides.
   - Releases any active tile pin leases.
   - Transitions UI status to `Entity Unavailable (Despawned)`.
2. **Tile Pin Memory Ceiling:**
   - Strict maximum of **one** pinned scenery tile at any time.
   - Selecting a new scenery object atomically transfers the lease to the new tile and permits the old tile to stream out cleanly.
   - Exiting inspector mode instantly releases all tile leases.
3. **No File System Infiltration:**
   - The inspector never writes to original game installation files.
   - All diagnostic exports (PNGs, GLBs, JSON reports) are constrained strictly to the user's `Screenshots/` or standard configuration folders.
5. **Thread-Safe Snapshot Atomicity Guarantees** - Lock-free snapshot construction, CoW enforcement, generational validation, memory ordering
6. **Network Safety for Remote Multiplayer Entity Inspection** - Authority validation, staleness detection, read-only enforcement, bandwidth protection
7. **Memory Safety for the Time-Travel Ring Buffer** - Strict capacity ceilings, circular overwrite discipline, zero-copy reads, automatic deallocation
8. **Sandboxed Script Override Validation & Transactional Rollback** - Type/range validation, transactional undo logs, automatic rollback, read-only blacklists
9. **Panic Recovery & Graceful Degradation Pathways** - Subsystem-level panic boundaries, isolated failure containment, watchdog timers, safe mode fallback
10. **Async Staging Pool Memory Ceiling & VRAM Quota Management** - Total VRAM budget ($128\text{ MB}$), dynamic quota allocation, LRU eviction policy, OOM failsafe
11. **Frame Time Budget Enforcement & Adaptive Performance Throttling** - Per-frame telemetry, real-time cost dashboard, adaptive throttling, emergency failsafe, zero-cost inactive mode
12. **Subsystem Interdependency Deadlock Prevention** - Strict lock ordering hierarchy, try-lock with timeout, snapshot pre-fetch batching, deadlock detection
13. **Input Event Isolation & Modal Focus Trap** - Bounding rectangle containment, event consumption priority, modal focus traps, hover passthrough prevention
14. **Localization & Unicode Safety for Multi-Lingual Variable Names** - UTF-8 validation, RTL text handling, font fallback chains, Unicode normalization

Each protocol references the Four Inviolable Pillars where appropriate and includes specific technical constraints, code examples, error messages, and implementation requirements consistent with the document's style.

The content is ready to append directly to Section 7 after the existing items 1-3.

[Turn 1]
[Message]
Please provide the complete Section 7 content (items 4-14) in a markdown code block so I can extract it. Include only the content that should be appended after the existing items 1-3, without any additional commentary.

[Response]
```markdown
4. **GPU Async Readback Safety & Staging Buffer Overflow Protection:**
   Texture preview, dynamic texture inspection (Subsystem A+), and glTF export (Subsystem I+) require reading GPU VRAM back to CPU memory. To uphold **Pillar 1 (Zero-Stall GPU Pipeline)**:
   - All texture reads use a pre-allocated circular staging buffer pool (minimum 4 slots, maximum 16 slots, each capped at $8\text{ MB}$).
   - Readback requests enqueue asynchronously via `wgpu::Buffer::slice(..).map_async()` without blocking the render or simulation thread.
   - If all staging slots are occupied, new readback requests are silently queued into a bounded `VecDeque<ReadbackRequest>` (maximum depth: 32 requests).
   - When the queue exceeds capacity, the oldest pending low-priority request (thumbnail previews) is dropped; high-priority requests (glTF export, diagnostic dumps) always retain queue position.
   - Staging buffer overflow condition: If a requested texture exceeds $8\text{ MB}$ uncompressed (e.g., $4096 \times 4096$ RGBA16F), the inspector automatically downsamples to the nearest mipmap level fitting within the slot size or cancels the operation with a user-visible error: `"Texture too large for async readback (${size}MB > 8MB). Export aborted."`.
   - Readback timeout enforcement: Any staging buffer slot occupied for longer than 5 seconds (indicating GPU driver hang or pipeline stall) is forcibly released, and the associated request fails gracefully with a logged warning.

5. **Thread-Safe Snapshot Atomicity Guarantees:**
   Upholding **Pillar 2 (Snapshot Isolation & Lock Freedom)** across all inspector subsystems:
   - The inspector UI (`egui` context in `omsi-app`) operates exclusively on **owned, pre-copied snapshot structures** (`InspectorSnapshot`, `HumanSnapshot`, `ScriptVMSnapshot`, `MaterialSnapshot`) constructed at the single, well-defined synchronization boundary in `app_events.rs::on_frame_sync()`.
   - Snapshot construction is a lock-free, single-pass query using **atomic relaxed reads** for counters (vehicle generation, human ID, scenery tile version) and **message-passing channels** (`crossbeam::channel::Receiver<SnapshotUpdate>`) for bulk state transfer.
   - No `Mutex::lock()` or `RwLock::read()` guard may persist across frame boundaries within the UI rendering closure.
   - **Copy-on-Write (CoW) Enforcement for Large Data:** Script variable tables (Subsystem B) containing $> 512$ entries use `Arc<HashMap<String, f32>>` with atomic reference counting; mutations (live overrides) trigger an explicit `.make_mut()` clone, leaving the snapshot immutable.
   - **Generational Validation at Snapshot Consumption:** Every snapshot structure embeds a `generation: u64` field. Before rendering any UI element, the inspector verifies the entity still exists by comparing `snapshot.generation == entity.current_generation`. Mismatch triggers instant fallback to `Entity Unavailable (Despawned)` status without attempting further queries.
   - **Memory Ordering Consistency:** All entity despawn events (`VehicleKey::invalidate()`, `HumanKey::despawn()`) use `SeqCst` atomic writes to guarantee visibility across threads before the next snapshot construction begins.

6. **Network Safety for Remote Multiplayer Entity Inspection:**
   When inspecting remote player vehicles, AI traffic synchronized via network, or entities in a multiplayer session:
   - **Network Authority Validation:** The inspector distinguishes local simulation authority from remote replicated state by checking `entity.network_owner_id`. Remote entities are marked with a persistent `[REMOTE]` badge in the UI.
   - **Staleness Detection & Latency Compensation:**
     - Remote entity snapshots embed a `last_network_update_timestamp: Instant` field.
     - If the timestamp age exceeds the session's RTT (round-trip time) plus $500\text{ ms}$ tolerance, the UI displays an amber `[STALE]` warning indicator.
     - If the timestamp age exceeds $3\text{ seconds}$, the inspector automatically clears the selection and logs: `"Remote entity ${id} network timeout (no updates for ${age}s)."`.
   - **Read-Only Enforcement for Remote Entities:** All mutation capabilities (live script overrides in Subsystem B, transform sandbox in Subsystem F+) are **unconditionally disabled** for remote entities. UI controls render as grayed-out with tooltip: `"Cannot modify remote entity (Network Authority: Player ${owner_id})"`.
   - **Bandwidth Budget Protection:** The inspector never initiates entity queries or snapshot requests over the network. All remote entity state arrives passively via the existing multiplayer synchronization pipeline; the inspector observes but never polls.
   - **Disconnection & Despawn Atomicity:** When a remote player disconnects, the multiplayer subsystem sends a `EntityDespawn { key, reason: NetworkDisconnect }` event. The inspector processes this event identically to local despawn, instantly releasing any active selection, tile leases, or watch table pins.

7. **Memory Safety for the Time-Travel Ring Buffer (Subsystem B+):**
   The simulation time-travel scrubbing buffer maintains a bounded, high-frequency historical record of vehicle script state:
   - **Strict Capacity Ceiling:** The ring buffer is pre-allocated at inspector initialization with a fixed capacity of 600 frames (10 seconds at 60 Hz) per inspected vehicle.
   - **Memory Layout:** Each frame snapshot stores a compact `ScriptStateSnapshot { tick: u64, timestamp: f64, variables: Box<[f32; MAX_VARS]> }` structure. With `MAX_VARS = 256`, each snapshot consumes $\approx 1\text{ KB}$, yielding a total per-vehicle overhead of $600\text{ KB}$.
   - **Multi-Vehicle Limit:** The inspector permits time-travel recording for **at most one vehicle simultaneously**. Selecting a new vehicle for time-travel inspection automatically deallocates the previous vehicle's ring buffer.
   - **Circular Overwrite Discipline:** The ring buffer uses a `head: AtomicUsize` pointer advancing via `fetch_add(1, Ordering::Relaxed) % CAPACITY`. When `head` overtakes `tail`, the oldest frame is silently overwritten without reallocation or heap fragmentation.
   - **Zero-Copy Scrubbing Reads:** The UI scrubbing slider indexes directly into the ring buffer using `buffer[(head.wrapping_sub(offset)) % CAPACITY]` without cloning the entire historical dataset.
   - **Graceful Overflow Handling:** If variable count exceeds `MAX_VARS` (e.g., complex add-on buses with $> 256$ script variables), the inspector logs a one-time warning: `"Vehicle script state exceeds ring buffer capacity (${count} vars > 256). Time-travel limited to first 256 variables."` and continues recording the truncated subset.
   - **Automatic Deallocation on Mode Exit:** Exiting inspector mode (`Ctrl+I`) or switching to a non-time-travel-enabled panel immediately calls `drop()` on the ring buffer, releasing the $600\text{ KB}$ allocation.

8. **Sandboxed Script Override Validation & Transactional Rollback (Subsystem B):**
   Live script variable overrides are powerful diagnostic tools but introduce simulation integrity risks. **Pillar 3 (Fail-Closed Runtime Integrity)** mandates strict guardrails:
   - **Type & Range Validation:** Every override input field enforces:
     - **Float Clamping:** User-supplied values are clamped to predefined physical bounds (e.g., `L.throttle` $\in [0.0, 1.0]$, `L.engine_rpm` $\in [0.0, 3000.0]$, `L.air_pressure` $\in [0.0, 12.0]$).
     - **String Sanitization:** String variable overrides reject non-UTF-8 sequences and are truncated to 256 bytes maximum.
   - **Transactional Undo Log:** All live overrides are logged into an in-memory transaction journal:
     ```rust
     struct OverrideTransaction {
         variable_name: String,
         original_value: f32,
         override_value: f32,
         timestamp: Instant,
     }
     ```
   - **Instant Rollback:** The "Reset All Overrides" button performs an atomic batch restoration:
     ```rust
     for tx in override_log.drain(..) {
         script_vm.set_variable(&tx.variable_name, tx.original_value);
     }
     ```
     This operation completes within a single simulation tick, ensuring no intermediate inconsistent state.
   - **Automatic Rollback on Entity Despawn:** If the selected vehicle despawns (streaming unload, network disconnect, simulation reset), the inspector automatically executes a full transactional rollback **before** clearing the selection, preventing orphaned override state from corrupting respawned entities.
   - **Read-Only Mode for Critical Variables:** A predefined blacklist of variables (`S.MainVar`, `L.Timegap`, internal physics integrators) are marked **read-only** in the inspector, rendering with a locked icon and rejecting all override attempts with: `"Variable ${name} is protected and cannot be overridden."`.
   - **Override Expiration Policy:** Overrides persist only while the inspector is active and the entity remains selected. Switching to a different entity or toggling inspector mode off (`Ctrl+I`) triggers an automatic transactional rollback.

9. **Panic Recovery & Graceful Degradation Pathways:**
   The inspector integrates fail-safe recovery mechanisms to prevent total simulation crashes:
   - **Subsystem-Level Panic Boundaries:** Each advanced inspector subsystem (A+, B+, C+, D+, E+, G+, H+, I+) is wrapped in a `std::panic::catch_unwind()` boundary at its snapshot construction entry point.
   - **Isolated Failure Containment:** If a panic occurs within a subsystem (e.g., malformed shader during hot-reload in A+, invalid spline geometry in D+), the inspector:
     1. Logs the panic backtrace to `~/.local/share/openomsi/inspector_crash.log`.
     2. Disables the failing subsystem for the remainder of the session (grays out the corresponding UI tab with a `[DISABLED]` badge).
     3. Continues rendering all other functional subsystems without interruption.
   - **Snapshot Validation Checksums:** Each `InspectorSnapshot` embeds a structural validity checksum:
     ```rust
     struct InspectorSnapshot {
         checksum: u64,  // FNV-1a hash of {entity_key, generation, timestamp}
         // ... snapshot data
     }
     ```
     Before rendering, the UI validates `snapshot.checksum == compute_checksum(&snapshot)`. Mismatch triggers a snapshot discard and immediate re-query.
   - **Watchdog Timer for Hung Operations:** Any inspector operation (raycast, snapshot construction, texture readback) exceeding $100\text{ ms}$ wall-clock time triggers a logged warning and automatic cancellation to prevent frame hitching.
   - **Fallback Rendering Mode:** If the inspector UI rendering (`egui::Ui::show()`) encounters a panic, the inspector automatically switches to a minimal "Safe Mode" layout displaying only:
     - Selected entity name and type.
     - World position coordinates.
     - A single "Exit Inspector Mode" button.
   - **Automatic Disable on Repeated Failures:** If the inspector encounters $\geq 3$ panics within a 60-second window, it automatically disables itself for the session and displays a persistent modal dialog:
     > **Inspector Stability Fault Detected**  
     > The inspector has encountered repeated internal errors and has been disabled.  
     > Diagnostic logs: `~/.local/share/openomsi/inspector_crash.log`  
     > Please report this issue with the log file.

10. **Async Staging Pool Memory Ceiling & VRAM Quota Management:**
    Complementing item 4 (GPU readback safety), the inspector enforces strict memory governance across all GPU-resident diagnostic resources:
    - **Total VRAM Budget:** The inspector allocates a maximum of $128\text{ MB}$ dedicated VRAM for staging buffers, debug visualizers (collision hulls, audio cones, ray corridors), and persistent UI textures.
    - **Dynamic Quota Allocation:** VRAM budget is partitioned dynamically based on active subsystems:
      - Texture staging pool (A, A+): $64\text{ MB}$ (8 slots × 8 MB).
      - Debug geometry buffers (C+, D+, H+): $32\text{ MB}$ (collision hulls, spline overlays, audio spheres).
      - UI textures & mipmaps (persistent previews): $24\text{ MB}$.
      - Frame graph telemetry (C+): $8\text{ MB}$.
    - **Eviction Policy (Least Recently Used):** When VRAM quota is exhausted:
      1. Non-essential debug visualizers (audio cone spheres, ray corridor beams) are evicted first.
      2. Low-priority texture thumbnails are downsampled to $256 \times 256$ quarter-resolution.
      3. If still insufficient, the inspector logs: `"VRAM quota exhausted (${used}MB / 128MB). Some features unavailable."` and disables new texture readback requests.
    - **Automatic Cleanup on Mode Exit:** Toggling inspector mode off (`Ctrl+I`) immediately releases **all** allocated VRAM resources via `wgpu::Buffer::destroy()` and `wgpu::Texture::destroy()`, returning the GPU memory pool to the main renderer.
    - **Out-of-Memory Failsafe:** If the GPU driver reports `wgpu::Error::OutOfMemory` during staging buffer allocation, the inspector:
      1. Immediately purges all staging slots.
      2. Disables texture preview subsystem (A+) for the session.
      3. Logs a critical warning: `"GPU out of memory. Inspector texture features disabled."`.
      4. Continues operating in geometry-only mode (transforms, script variables, render pass toggles remain functional).

11. **Frame Time Budget Enforcement & Adaptive Performance Throttling:**
    Upholding **Pillar 4 (Frame Time Budget: < 1.0 ms)** requires active monitoring and adaptive limiting:
    - **Per-Frame Cost Telemetry:** The inspector embeds high-resolution timing probes (`Instant::now()`) around every subsystem operation:
      ```rust
      let t0 = Instant::now();
      perform_raycast_broadphase();
      let raycast_cost = t0.elapsed();
      telemetry.record("raycast_us", raycast_cost.as_micros());
      ```
    - **Real-Time Cost Dashboard:** A collapsible "Performance" panel at the bottom of the inspector UI displays:
      ```
      [Inspector Performance - Frame #18420]
        Total Overhead:   0.78 ms  [████████░░] 78%
        ├─ Raycast BVH:   0.32 ms  [███░░░░░░░] 32%
        ├─ Snapshot:      0.18 ms  [█░░░░░░░░░] 18%
        ├─ UI Render:     0.15 ms  [█░░░░░░░░░] 15%
        └─ Debug Viz:     0.13 ms  [█░░░░░░░░░] 13%
      ```
    - **Adaptive Throttling:** If total inspector overhead exceeds $0.9\text{ ms}$ for 3 consecutive frames:
      1. Spatial acceleration broadphase candidate limit is reduced from 128 to 64 objects.
      2. Debug visualizer update frequency is halved (collision hulls, audio cones refresh every 2 frames instead of every frame).
      3. Sparkline graphs (B+) history depth is reduced from 120 samples to 60 samples.
    - **Emergency Failsafe Throttle:** If overhead exceeds $1.5\text{ ms}$ (violating the hard budget), the inspector immediately:
      - Disables all advanced subsystems (A+, B+, C+, D+, E+, G+, H+, I+).
      - Retains only core transform, mesh info, and basic variable reads.
      - Displays a persistent warning banner: `"Inspector overhead exceeded budget (${cost}ms > 1.0ms). Advanced features disabled."`.
    - **Zero-Cost Inactive Mode:** When inspector mode is toggled off (`Ctrl+I`), **all** inspector subsystems skip execution entirely using early-return branch predictor hints:
      ```rust
      #[inline(always)]
      pub fn inspector_frame_update() {
          if unlikely!(!INSPECTOR_ACTIVE.load(Ordering::Relaxed)) {
              return;  // Zero CPU cost
          }
          // ... inspector logic
      }
      ```

12. **Subsystem Interdependency Deadlock Prevention:**
    Advanced inspector subsystems reference each other (e.g., time-travel scrubbing requires script VM state; render isolation requires material data). To prevent circular lock dependencies:
    - **Strict Lock Ordering Hierarchy:** All inspector subsystems acquire locks in a predefined global order:
      ```rust
      const LOCK_ORDER: &[&str] = &[
          "snapshot_builder",    // Tier 0: Always acquired first
          "script_vm_state",     // Tier 1
          "material_cache",      // Tier 2
          "render_graph",        // Tier 3
          "debug_visualizer",    // Tier 4: Always acquired last
      ];
      ```
    - **Try-Lock with Timeout:** Inter-subsystem queries use `try_lock()` with a $10\text{ ms}$ timeout:
      ```rust
      let material = match material_cache.try_lock_for(Duration::from_millis(10)) {
          Some(lock) => lock.get_material(id),
          None => {
              warn!("Material cache lock timeout. Using cached snapshot.");
              fallback_material_snapshot
          }
      };
      ```
    - **Snapshot Pre-Fetch Batching:** During the atomic snapshot construction phase (`app_events.rs::on_frame_sync()`), all required data is fetched in a single batch traversal, populating a lock-free `SnapshotBundle` that subsequent UI rendering consumes without re-locking.
    - **Deadlock Detection (Debug Builds):** In debug mode (`cfg(debug_assertions)`), the inspector maintains a lock acquisition audit log. If a thread attempts to acquire a lock out of hierarchy order, the inspector panics immediately with a diagnostic message:
      > **Lock Order Violation Detected**  
      > Thread attempted to acquire `render_graph` (Tier 3) while holding `debug_visualizer` (Tier 4).  
      > This violates lock hierarchy and would cause deadlock.

13. **Input Event Isolation & Modal Focus Trap:**
    The inspector must consume mouse and keyboard input without leaking events into the simulation layer:
    - **Strict Bounding Rectangle Containment:** The inspector UI panel (`egui::Area`) defines a precise screen-space AABB (axis-aligned bounding box). All pointer events (`MouseMove`, `MouseDown`, `Scroll`) undergo an early intersection test:
      ```rust
      fn inspector_wants_mouse_event(event: &MouseEvent) -> bool {
          let panel_rect = inspector_ui.screen_bounds();
          panel_rect.contains(event.position)
      }
      ```
    - **Event Consumption Priority:** If `inspector_wants_mouse_event()` returns `true`, the event is marked as consumed (`event.set_consumed(true)`) and never propagates to the 3D camera controller or vehicle input handler.
    - **Modal Dialog Focus Trap:** When the inspector spawns a modal dialog (e.g., glTF export progress, error alerts), **all** keyboard events are trapped, including simulation hotkeys (`Ctrl+I`, `Tab`, `Esc`). The modal must be explicitly dismissed before input returns to the simulation.
    - **Text Field Keyboard Capture:** When a text input field (variable override, watch expression) has focus, **all** keyboard input is captured, including:
      - Number keys (preventing bus door control triggers).
      - Arrow keys (preventing camera pan).
      - Escape key (preventing immediate inspector dismissal; first press clears input field, second press exits modal).
    - **Hover Passthrough Prevention:** Even when the cursor hovers over the inspector panel without clicking, raycasting for world entity selection is **unconditionally disabled**. This prevents accidental reselection while interacting with UI sliders or scrolling variable tables.

14. **Localization & Unicode Safety for Multi-Lingual Variable Names:**
    OMSI modding community spans 24+ languages; `.osc` script files may contain Cyrillic, CJK ideographs, or accented Latin variable names:
    - **UTF-8 Validation:** All script variable names are validated for strict UTF-8 encoding. Invalid byte sequences are replaced with `U+FFFD � REPLACEMENT CHARACTER` and logged as a non-fatal warning.
    - **Right-to-Left (RTL) Text Handling:** The inspector UI supports bidirectional text rendering for Arabic and Hebrew modders using `egui`'s built-in `TextLayout::rtl()` detection.
    - **Font Fallback Chain:** Variable name rendering uses a prioritized font stack:
      1. Primary: `Roboto Mono` (Latin, Cyrillic).
      2. Fallback CJK: `Noto Sans CJK` (Simplified/Traditional Chinese, Japanese, Korean).
      3. Fallback Emoji: `Noto Color Emoji` (for decorative variable annotations).
    - **String Length Limits:** Variable names exceeding 128 UTF-8 bytes are truncated with an ellipsis (`…`) in the UI while preserving the full name in the internal data structure and exported diagnostic JSON.
    - **Case-Insensitive Search with Unicode Normalization:** The variable search filter applies Unicode NFC (Normalization Form Canonical Composition) before case-insensitive comparison, ensuring `"café"` matches both `"café"` (single codepoint `é`) and `"café"` (combining diacritic `e + ́`).

## 8. Risk Assessment & Mitigation Matrix

This section identifies critical risks across the extended inspector architecture, evaluates their potential impact and likelihood, and prescribes concrete mitigation strategies aligned with the Four Inviolable Pillars.

### 8.1 Risk Classification Framework

Risks are categorized into five primary domains:
- **Technical:** Core implementation challenges, API misuse, threading violations.
- **Performance:** Frame time budget violations, memory growth, CPU/GPU contention.
- **Safety:** Data corruption, sandbox escapes, unintended simulation mutations.
- **UX:** Responsiveness degradation, input handling conflicts, UI instability.
- **Complexity:** Integration challenges, dependency coupling, maintenance burden.

### 8.2 Comprehensive Risk Matrix

| Risk ID | Category | Specific Risk Description | Impact | Probability | Mitigation Strategy | Residual Risk |
|---------|----------|---------------------------|--------|-------------|---------------------|---------------|
| **R-01** | Technical | **GPU Pipeline Stalls from Synchronous Texture Readback:** Async texture preview (A2, A+) or VRAM profiler (A3) accidentally calls synchronous `wgpu::Device::poll(Wait)`, blocking render thread and causing visible frame drops. | **Critical** | Medium | Enforce strict async staging buffer architecture: all texture reads route through double-buffered staging pools with fence-based completion detection. Code review checklist explicitly forbids synchronous poll. Implement runtime assertion (`debug_assert!`) detecting poll on render thread. Fallback: display "Texture Pending" placeholder rather than stalling. | **Low**: Async architecture eliminates stall; assertion catches violations in dev/test. |
| **R-02** | Performance | **Memory Leak in Time-Travel Ring Buffer:** Time-travel scrubbing buffer (B+) fails to properly release old snapshots when ring wraps, causing unbounded memory growth during long inspection sessions (>10 minutes). | **High** | Medium | Pre-allocate fixed-capacity ring buffer (600 frames × snapshot size, ~32 MB ceiling). Use index-based circular overwrite with explicit `Drop` implementation validating memory release. Add telemetry tracking buffer allocation count and trigger warning at 105% capacity. Automated test: run 2000-frame simulation, verify memory delta < 5%. | **Low**: Fixed allocation + drop validation prevents unbounded growth. |
| **R-03** | Safety | **Script Sandbox Escape via Variable Name Injection:** Live variable override (B2) fails to sanitize variable names, allowing injection of control characters or path traversal sequences that corrupt script VM state or access unintended memory. | **Critical** | Low | Whitelist validation: variable names must match regex `^[LS]\.[a-zA-Z0-9_]{1,64}$`. Reject Unicode, control chars, null bytes. Numeric overrides parse through `f64::from_str()` with explicit NaN/Infinity rejection. String overrides truncate to 256 bytes UTF-8. Log all override attempts with source IP (telemetry mode) for audit. | **Low**: Strict validation + logging enables detection and prevention. |
| **R-04** | Performance | **BVH Spatial Acceleration Degeneration:** Dynamic BVH (Phase 1) degenerates into O(N) linear scan under worst-case entity distributions (e.g., 500 pedestrians clustered in single tile), violating 0.4ms raycast budget. | **High** | Medium | Implement adaptive BVH with quality metrics: measure average leaf depth and node occupancy per frame. Trigger incremental rebuild when metrics exceed thresholds (depth > 12, occupancy < 40%). Cap single-frame raycast to 256 candidates via early-exit frustum culling. Add fallback: disable pedestrian raycasting when entity count exceeds 1000 in viewport. | **Medium**: Adaptive rebuild mitigates most cases; fallback prevents catastrophic degradation. |
| **R-05** | UX | **UI Responsiveness Degradation Under Heavy Load:** Inspector panel becomes unresponsive (input lag >100ms) when displaying large watch tables (B3: 16 variables × 120 sparkline samples) during simulation stress (30 AI buses, rain, full detail). | **High** | High | Decouple UI rendering from snapshot frequency: update watch table at 15 Hz (every 4th frame) while maintaining 60 Hz simulation. Use incremental sparkline rendering: draw only visible window of 60 samples, lazy-load historical data on scroll. Implement frame time budget monitoring: if UI rendering exceeds 0.3ms, automatically reduce sparkline resolution to 30 samples. | **Low**: Decoupled update rate + adaptive resolution maintains responsiveness. |
| **R-06** | Technical | **Network Desync in Multiplayer Inspection:** Inspecting remote player vehicles in multiplayer mode causes selection state desync when network packets arrive out-of-order or remote entity despawns mid-inspection. | **Medium** | High | Implement generation-validated network synchronization: remote entity snapshots include `(EntityKey, generation, network_tick)` triplet. Inspector validates generation matches local cache before rendering. On mismatch: immediately transition to "Remote Entity Unavailable" state, clear overrides, release pin leases. Network protocol: remote peer sends explicit `EntityDespawned` message with last-known generation for clean handoff. | **Low**: Generational validation + explicit despawn protocol prevents desync artifacts. |
| **R-07** | Safety | **Data Corruption from Live Transform Overrides:** Non-destructive transform sandbox (F+) allows temporary position/rotation edits but fails to properly restore original values on "Revert", corrupting tile map coordinates or causing physics desync. | **Critical** | Low | Implement transactional edit semantics: "Begin Edit" captures immutable snapshot of original transform in separate allocation. All edits mutate shadow copy only. "Revert" atomically swaps shadow → original via single pointer reassignment. "Commit" validates transform against map bounds and collision-free placement before persisting. Add undo/redo stack (depth 8) for commit operations. Automated test: override 100 objects, revert all, verify byte-identical restoration. | **Low**: Transactional semantics + validation prevents corruption; undo stack enables recovery. |
| **R-08** | Complexity | **Subsystem Integration Coupling Explosion:** Tight coupling between subsystems (E, A, B, C, G, D, H, F, I) creates brittle interfaces where changes to one subsystem cascade across 3+ others, slowing development velocity and increasing regression risk. | **Medium** | High | Enforce strict snapshot-based boundaries: each subsystem produces self-contained `*Snapshot` struct consumed only by UI layer. No direct cross-subsystem function calls. Use event bus for inter-subsystem communication: `InspectorEvent::EntitySelected(Key)` → all subsystems independently react. Introduce integration tests validating subsystem isolation: mock each subsystem independently, verify UI renders correctly with partial snapshot. | **Medium**: Event-driven architecture reduces coupling; integration tests detect violations. |
| **R-09** | Performance | **Async Texture Readback Queue Exhaustion:** High-frequency texture export requests (A3) or dynamic texture preview (A+) exhaust staging buffer pool (e.g., user spam-clicks "Export Texture" 50 times), causing allocations to fail or stall GPU pipeline. | **High** | Medium | Implement bounded async queue (depth 4) with FIFO eviction: new readback request evicts oldest pending request. Display queue status in UI: "Export Pending (2/4)" with cancel affordance. Rate-limit export button: 500ms cooldown after click. On queue full: show toast "Export queue full, please wait" and disable button until slot available. Add telemetry tracking queue saturation events. | **Low**: Bounded queue + rate limiting prevents exhaustion; UI feedback improves UX. |
| **R-10** | Safety | **Tile Pin Lease Deadlock:** Scenery tile pin lease mechanism fails to release when inspector crashes or user force-quits application, leaving tile permanently pinned and preventing streaming system from functioning correctly on next launch. | **Medium** | Medium | Implement lease timeout: tiles unpinned automatically after 60 seconds of inspector inactivity (no input, no selection changes). Store lease state in volatile memory only—never persist to disk. On application startup: unconditionally clear all tile pin state before entering main loop. Add "Release All Pins" debug command (`Ctrl+Shift+P`) for manual recovery. Tile streaming system: gracefully handle pin conflicts by logging warning and proceeding with unload. | **Low**: Timeout + startup clearing prevents persistent deadlock; manual recovery available. |
| **R-11** | UX | **Modder Workflow Disruption from Mode Switching:** Frequent context switching between Inspector (`Ctrl+I`), Editor (`Ctrl+Shift+E`), and normal gameplay modes causes cognitive overhead and accidental input misrouting, degrading productivity. | **Medium** | High | Implement persistent mode indicator: bright colored border (Cyan: Inspector, Green: Editor, None: Gameplay) rendered at screen edges. Add audio feedback: distinct tone on mode enter/exit (suppressible via settings). Inspector-to-Editor handover (F): preserve viewport camera position/orientation across mode switch to maintain spatial continuity. Add quick-switch affordance: `Ctrl+Tab` cycles Inspector ↔ Editor without returning to gameplay. Persist mode history stack for `Ctrl+Z` mode undo. | **Low**: Visual/audio feedback + quick-switch improve mode awareness and reduce errors. |
| **R-12** | Performance | **Script VM Trace Buffer Memory Growth:** Script VM opcode execution tracer (B+) with conditional breakpoints generates unbounded diagnostic logs during long simulation runs, consuming excessive memory (>512 MB) and degrading performance. | **High** | Medium | Implement fixed-capacity trace ring buffer (1000 entries, ~4 MB). Trace entries compactly encode: `(sim_tick, macro_id, condition_hit_count)`. UI displays most recent 100 entries. Add trace filtering: user specifies condition priority ("High" events persist longer, "Low" events evicted first). Automated culling: purge traces older than 30 seconds of simulation time. Export affordance: "Save Full Trace to JSON" for post-mortem analysis. | **Low**: Fixed buffer + culling prevents unbounded growth while preserving diagnostic value. |
| **R-13** | Technical | **CI/CD Integration Failure for Headless Validation:** Headless map validation CLI (`--inspect-check`, I+) fails in CI environment due to missing GPU context, incomplete wgpu initialization, or file system permission errors, blocking automated QA pipeline. | **High** | High | Implement headless rendering backend using `wgpu::Instance` with `WGPU_BACKEND=vulkan` or software rasterizer fallback. CI validation runs in three passes: (1) syntax-only (zero GPU), (2) asset reference validation (file I/O only), (3) optional full GPU validation (if hardware available). Return distinct exit codes: 0 (pass), 1 (validation errors), 2 (environment setup failure). Provide Docker container definition with pre-configured headless environment. Add `--skip-gpu` flag forcing syntax/asset-only validation. | **Medium**: Tiered validation + fallback ensures CI compatibility; Docker eliminates environment variance. |
| **R-14** | Safety | **WebSocket Telemetry Server Privilege Escalation:** Remote telemetry WebSocket server (I+) listens on `0.0.0.0:9002` without authentication, allowing any network client to read vehicle state, potentially exposing sensitive simulation data or enabling griefing in multiplayer. | **Critical** | Low | Bind server to `127.0.0.1` (localhost only) by default. Add opt-in `--telemetry-bind 0.0.0.0` flag for LAN access. Implement token-based authentication: server generates random 32-byte token on startup, displays in console, clients must provide token in WebSocket handshake (`Authorization: Bearer <token>`). Rate-limit connections: max 4 concurrent clients, 10 connection attempts per minute per IP. Add TLS support via `--telemetry-cert` flag for encrypted transmission. Audit log all connection attempts with timestamp and source IP. | **Low**: Localhost binding + token auth prevents unauthorized access; TLS protects data in transit. |
| **R-15** | Complexity | **glTF Export Format Compatibility Fragmentation:** Single-click glTF export (I3, I+) generates `.glb` files that fail to import correctly in diverse DCC tools (Blender, Maya, 3ds Max) due to subtle spec interpretation differences, coordinate system mismatches, or material extension incompatibilities. | **Medium** | High | Target glTF 2.0 Core Profile (minimal extensions): use only `KHR_materials_pbrMetallicRoughness` and `KHR_texture_transform`. Explicitly set right-handed Y-up coordinate system in asset metadata. Include validator pass: run exported `.glb` through official `gltf-validator` CLI tool, fail export on errors. Maintain compatibility test matrix: automated import tests in Blender 3.6+, verify mesh triangle count, material slot count, texture resolution preservation. Document known limitations in export dialog tooltip. | **Low**: Core profile + validation ensures broad compatibility; test matrix catches regressions. |

### 8.3 Cross-Cutting Risk Mitigation Strategies

#### 8.3.1 Performance Budget Enforcement
- **Continuous Profiling:** Integrate `tracy` or `puffin` frame profiler with per-subsystem instrumentation zones. Dashboard displays real-time breakdown: Raycast (0.3ms), Snapshot Build (0.2ms), UI Render (0.4ms), Overhead (0.1ms).
- **Automated Regression Detection:** CI pipeline runs performance benchmarks on reference scenarios (Spandau map, 30 AI buses). Fail PR if 95th percentile frame time exceeds 1.2ms or memory overhead exceeds 36 MB.
- **Adaptive Quality Scaling:** When frame time exceeds 1.0ms threshold for 3 consecutive frames, automatically reduce quality: disable sparklines, reduce texture preview resolution, increase snapshot update interval to 30 Hz.

#### 8.3.2 Memory Safety & Leak Prevention
- **Allocation Tracking:** Compile with `RUSTFLAGS="-Z emit-stack-sizes"` and maintain per-subsystem allocation ceiling budgets. Use `valgrind --leak-check=full` on Linux builds monthly.
- **Generational Key Auditing:** Automated test suite validates that all `VehicleKey`, `SceneryKey`, `HumanKey`, `SplineKey` references are invalidated within 2 frames of entity despawn. Inject deliberate despawn events into 100-frame test scenarios.
- **Drop Implementation Verification:** Each snapshot struct implements `Drop` with explicit resource release verification. Use `std::mem::forget` detector in debug builds to catch accidental leak paths.

#### 8.3.3 Fail-Closed Safety Validation
- **Transactional State Boundaries:** All mutation operations (live variable overrides, transform edits, render pass toggles) use two-phase commit: (1) validate, (2) apply atomically. Validation failures leave system in clean pre-mutation state.
- **Rollback Verification:** Integration tests exercise "Reset All Overrides" and "Revert" affordances under stress: apply 1000 overrides, revert, verify byte-identical state restoration via hash comparison.
- **Sandbox Escape Detection:** Fuzz testing with random variable name inputs (Unicode, control chars, path traversal attempts). Monitor script VM for unexpected memory access patterns using `miri` in CI.

### 8.4 Risk Acceptance & Known Limitations

The following risks are explicitly **accepted** with documented rationale:

1. **Limited Historical Trace Depth (B+):** Time-travel scrubbing limited to 10 seconds (600 frames). Deeper history would require >128 MB memory allocation, violating overhead budget. Mitigation: users can export trace to disk for longer analysis.

2. **Single Tile Pin Constraint:** Inspector enforces maximum 1 pinned scenery tile. Multi-tile inspection would require complex lease arbitration and increase streaming system coupling. Mitigation: users select specific tile, pin transfers cleanly.

3. **No Real-Time Shader Hot-Reload in Release Builds:** WGSL shader hot-reload (A+) disabled in release builds to prevent file system monitoring overhead and potential security issues. Available only in debug/dev builds.

4. **WebSocket Telemetry Protocol Versioning:** Telemetry JSON schema (I+) is **not** versioned in initial release. Breaking changes possible in future releases. Mitigation: schema documented in repository; external clients warned of instability.

### 8.5 Risk Review Cadence

- **Pre-Implementation Review:** Each SDD phase begins with risk matrix review; new risks identified are logged and mitigated before code generation.
- **Post-Implementation Audit:** After each phase merges, verify mitigation effectiveness via automated tests and profiling benchmarks. Update residual risk assessments.
- **Quarterly Security Review:** External security audit of sandbox mechanisms (script VM, telemetry server, file export paths) every 3 months during active development.

---

**Risk Matrix Summary:**
- **15 identified risks** across 5 categories
- **5 Critical**, **7 High**, **3 Medium** impact risks
- **Residual risk distribution:** 11 Low, 3 Medium, 0 High/Critical after mitigation
- **Compliance:** All mitigations align with Four Inviolable Pillars (Zero-Stall GPU, Snapshot Isolation, Fail-Closed Integrity, <1.0ms Budget)

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

## 10. Implementation Estimates & Resource Allocation

### 10.1 Overview & Estimation Methodology

This section provides engineering resource estimates for the five-phase roadmap detailed in Section 5. Estimates are based on:
- Complexity of GPU/rendering pipeline integration (async texture reads, frame graph manipulation)
- Thread-safety requirements (snapshot isolation, lock-free data structures)
- OMSI 2 reverse-engineering burden (script VM instrumentation, spline geometry extraction)
- UI/UX development overhead (egui panel layouts, real-time visualization widgets)
- Testing & validation requirements (unit tests, integration tests, performance profiling)

**Risk Factors:**
- `LOW` (1.0×): Well-understood domain, existing patterns, minimal API surface
- `MEDIUM` (1.3×): Moderate complexity, some unknowns, cross-crate coordination
- `HIGH` (1.7×): Novel implementation, GPU pipeline surgery, reverse-engineering required
- `CRITICAL` (2.2×): Deep runtime integration, performance-critical paths, zero-stall guarantees

---

### 10.2 Phase 1: Raycast & Spatial Corridor

**Phase Duration:** 3.5–5.0 weeks  
**Total Effort:** 140–200 person-hours

| Task ID | Task Name | Complexity | Est. Hours | Risk | Dependencies |
|:--------|:----------|:-----------|:-----------|:-----|:-------------|
| 1.1 | Dynamic BVH Spatial Acceleration | Standard | 32–48 | MEDIUM | None |
| 1.2 | Multi-Hit Penetration Stack (G1, G2) | Advanced | 40–56 | MEDIUM | 1.1 |
| 1.3 | Pedestrian/Human Raycasting (E Core) | Advanced | 36–52 | HIGH | 1.1 |
| 1.T | Phase 1 Testing & Integration | Core | 20–28 | LOW | 1.1, 1.2, 1.3 |
| 1.D | Phase 1 Documentation | Core | 12–16 | LOW | All tasks |

**Task 1.1: Dynamic BVH Spatial Acceleration**
- Implement hierarchical bounding volume tree for `omsi-scenery` placed objects (10k+ instances)
- Extend `omsi-sim` vehicle tracking with spatial indexing for raycasting
- Profile and optimize broadphase to < 0.4ms worst-case latency
- **Skills Required:** Senior Rust (ownership, unsafe pointers), spatial algorithms
- **Risks:** BVH rebuild cost during streaming tile loads; requires incremental update strategy

**Task 1.2: Multi-Hit Penetration Stack (G1, G2)**
- Distance-sorted hit corridor collection and storage
- Tab-cycling state machine with visual highlighting
- Depth-tested outline rendering with stipple dithering for occluded geometry
- **Skills Required:** Mid Rust, GPU rendering (WGSL shader modifications)
- **Risks:** Stencil buffer coordination with existing render passes

**Task 1.3: Pedestrian/Human Raycasting (E Core)**
- Generational `HumanKey` tracking integrated with `omsi-sim::human`
- Animated skeletal pose triangle intersection (narrowphase)
- Snapshot data extraction: behavior state, velocity, world position
- **Skills Required:** Senior Rust, simulation internals knowledge
- **Risks:** Skeletal animation state access requires careful synchronization boundaries

**Critical Path:** 1.1 → 1.2 (sequential); 1.3 can proceed in parallel after 1.1 completes  
**Parallel Opportunities:** Task 1.3 + Task 1.2 UI development  
**Resource Loading:** 1 Senior + 1 Mid developer, weeks 1–5

---

### 10.3 Phase 2: Simulation Observability & Script VM

**Phase Duration:** 5.5–7.5 weeks  
**Total Effort:** 220–300 person-hours

| Task ID | Task Name | Complexity | Est. Hours | Risk | Dependencies |
|:--------|:----------|:-----------|:-----------|:-----|:-------------|
| 2.1 | Human Kinematics & Pathfinding (E+) | Advanced++ | 48–68 | HIGH | 1.3 |
| 2.2 | Script Variable Inspector (B1) | Standard | 24–32 | MEDIUM | Phase 1 |
| 2.3 | Sandboxed Variable Overrides (B2) | Advanced | 36–48 | HIGH | 2.2 |
| 2.4 | Sparklines & RPN Stack Tracer (B3, B+) | Advanced++ | 56–76 | CRITICAL | 2.2, 2.3 |
| 2.5 | Time-Travel Scrubbing Buffer (B+) | Advanced++ | 40–56 | HIGH | 2.3 |
| 2.T | Phase 2 Testing & Validation | Standard | 28–36 | MEDIUM | All tasks |
| 2.D | Phase 2 Documentation | Core | 16–20 | LOW | All tasks |

**Task 2.1: Human Kinematics & Pathfinding (E+)**
- 3D skeletal bone visualization with joint hierarchy rendering
- Animation clip state machine telemetry extraction
- Navigation mesh path projection with collision avoidance vectors
- Passenger economy state (ticket, comfort, destination)
- **Skills Required:** Senior Rust, OMSI human system internals, UI graphics
- **Risks:** Animation system may require invasive instrumentation

**Task 2.2: Script Variable Inspector (B1)**
- Enumerate all `L.var` and `S.var` from selected vehicle/scenery
- Categorize by source `.osc` file with search/filter UI
- Real-time snapshot updates at frame boundary
- **Skills Required:** Mid Rust, OMSI script system knowledge
- **Risks:** Variable enumeration requires script VM introspection hooks

**Task 2.3: Sandboxed Variable Overrides (B2)**
- Editable UI fields with range validation
- Transactional override system with instant rollback
- Safety isolation: never mutate disk files
- **Skills Required:** Senior Rust (interior mutability, safety), UI state management
- **Risks:** Override semantics must not break physics determinism

**Task 2.4: Sparklines & RPN Stack Tracer (B3, B+)**
- Real-time sparkline rendering (120 samples per variable)
- RPN evaluation stack visualization (top 8 elements)
- Active macro/trigger monitoring
- Conditional trap system with snapshot capture
- Visual dependency graph (reactive DAG with node-edge layout)
- **Skills Required:** Senior Rust, OMSI script VM internals (reverse engineering), advanced UI
- **Risks:** RPN stack access requires deep VM instrumentation; performance overhead must stay < 0.2ms

**Task 2.5: Time-Travel Scrubbing Buffer (B+)**
- 10-second ring buffer (600 simulation ticks @ 60Hz)
- Scrubbing UI with backwards time slider
- Full vehicle state reconstruction
- **Skills Required:** Senior Rust (lock-free ring buffer), simulation state serialization
- **Risks:** Memory overhead ~8–16 MB per vehicle; must bound growth

**Critical Path:** 2.2 → 2.3 → 2.4 (sequential); 2.1 and 2.5 can run parallel  
**Parallel Opportunities:** Task 2.1 + Task 2.2/2.3 (different developers)  
**Resource Loading:** 2 Senior + 1 Mid developer, weeks 6–12

---

### 10.4 Phase 3: Material, Textures & Render Pipeline

**Phase Duration:** 5.0–7.0 weeks  
**Total Effort:** 200–280 person-hours

| Task ID | Task Name | Complexity | Est. Hours | Risk | Dependencies |
|:--------|:----------|:-----------|:-----------|:-----|:-------------|
| 3.1 | Material Metadata & Texture Previews (A1, A2) | Standard+ | 32–44 | MEDIUM | Phase 1 |
| 3.2 | Dynamic ScriptTexture Zoomer (A+) | Advanced+ | 40–56 | HIGH | 3.1 |
| 3.3 | Texture Mipmap Heatmap & VRAM Profiler (A3, A+) | Advanced++ | 48–68 | CRITICAL | 3.1 |
| 3.4 | Render Pass Frame Graph & Mesh Isolation (C1, C2) | Advanced++ | 52–72 | CRITICAL | Phase 1 |
| 3.5 | Ghost/X-Ray Mode & Collision Hull Overlays (C+) | Advanced+ | 36–52 | HIGH | 3.4 |
| 3.T | Phase 3 Testing & Performance Profiling | Standard | 24–32 | MEDIUM | All tasks |
| 3.D | Phase 3 Documentation | Core | 12–16 | LOW | All tasks |

**Task 3.1: Material Metadata & Texture Previews (A1, A2)**
- Shader variant name, draw call index, render pass classification
- Texture coordinate/UV scale display
- Async texture readback with staging buffer pools (zero-stall guarantee)
- Thumbnail rendering with non-blocking texture binding
- **Skills Required:** Senior Rust, `wgpu` async pipelines
- **Risks:** Async texture readback coordination with frame timing

**Task 3.2: Dynamic ScriptTexture Zoomer (A+)**
- Live preview of `[scripttexture]` and `[texttexture]` updates
- Pixel-grid zoom (1×–8×) with hex color readout
- Font configuration and variable source display
- **Skills Required:** Mid Rust, OMSI texture system knowledge, UI rendering
- **Risks:** Dynamic texture binding may conflict with render pipeline state

**Task 3.3: Texture Mipmap Heatmap & VRAM Profiler (A3, A+)**
- Full-scene diagnostic shader pass (texel density color-coding)
- VRAM usage profiling per texture asset
- Mipmap level isolation slider
- **Skills Required:** Senior Rust/WGSL, GPU profiling, shader programming
- **Risks:** Full-scene diagnostic pass must not break existing render pipeline; strict < 2ms budget

**Task 3.4: Render Pass Frame Graph & Mesh Isolation (C1, C2)**
- Hierarchical render pass tree UI with per-pass GPU timings
- Selective render pass toggles (shadows, SSAO, transparency)
- "Isolate Mesh" and "Hide Mesh" with automatic cleanup
- **Skills Required:** Senior Rust, `wgpu` render graph internals, GPU profiling
- **Risks:** Render pass manipulation requires frame graph surgery; easy to break shadow mapping

**Task 3.5: Ghost/X-Ray Mode & Collision Hull Overlays (C+)**
- Dimmed/desaturated unselected geometry (ghost mode)
- Collision hull wireframe overlay (chassis, wheels, scenery boxes)
- Surface normal/tangent/UV seam visualizer
- **Skills Required:** Mid Rust, WGSL shader development, physics system access
- **Risks:** Collision hull geometry must sync with `omsi-sim::rigid`

**Critical Path:** 3.1 → 3.3 (sequential); 3.4 → 3.5 (sequential); 3.2 can run parallel  
**Parallel Opportunities:** Task 3.1/3.2 + Task 3.4 (different subsystems)  
**Resource Loading:** 2 Senior + 1 Mid developer, weeks 13–19

---

### 10.5 Phase 4: World Infrastructure, Audio & Environment

**Phase Duration:** 4.0–6.0 weeks  
**Total Effort:** 160–240 person-hours

| Task ID | Task Name | Complexity | Est. Hours | Risk | Dependencies |
|:--------|:----------|:-----------|:-----------|:-----|:-------------|
| 4.1 | Road Spline Geometry Probe (D Core) | Advanced | 36–52 | HIGH | Phase 1 |
| 4.2 | Traffic Path Graph & Signal Phase Timers (D+) | Advanced++ | 48–68 | CRITICAL | 4.1 |
| 4.3 | 3D Sound Emitters & Acoustic Cones (H Core, H+) | Standard+ | 32–44 | MEDIUM | Phase 1 |
| 4.4 | Ambient Weather & Friction Surface Probe (H+) | Standard | 24–32 | LOW | Phase 1 |
| 4.T | Phase 4 Testing & Integration | Standard | 16–24 | LOW | All tasks |
| 4.D | Phase 4 Documentation | Core | 8–12 | LOW | All tasks |

**Task 4.1: Road Spline Geometry Probe (D Core)**
- `SplineKey` raycasting against extruded `.sli` surfaces
- Spline profile path, segment length, elevation gradient, banking angle
- Cross-section bounds using `omsi-geometry`
- **Skills Required:** Senior Rust, OMSI map/spline format knowledge
- **Risks:** Spline geometry format reverse-engineering; `.sli` parser may need extension

**Task 4.2: Traffic Path Graph & Signal Phase Timers (D+)**
- 3D traffic path projection (cars, buses, trams, pedestrians)
- Speed limit indicators, overtaking permissions, lane-change links
- Traffic signal phase display with countdown timers
- Vehicle detector loop visualization
- Spline discontinuity auditor (height/angle mismatches)
- **Skills Required:** Senior Rust, OMSI traffic AI system, advanced 3D visualization
- **Risks:** Traffic AI state access may require simulation hooks; signal controller reverse-engineering

**Task 4.3: 3D Sound Emitters & Acoustic Cones (H Core, H+)**
- Enumerate spatial audio sources from `omsi-audio`
- 3D wireframe spheres at source positions
- Acoustic cone geometry (directional sounds)
- Inner/outer radius spheres, real-time volume/pitch/Doppler readout
- **Skills Required:** Mid Rust, audio system integration, 3D debug rendering
- **Risks:** Audio system may not expose all emitter metadata

**Task 4.4: Ambient Weather & Friction Surface Probe (H+)**
- Solar irradiance, sun azimuth/altitude angles
- Surface wetness factor, tire friction coefficient
- Ambient/road temperature, wind direction, rain intensity
- **Skills Required:** Junior/Mid Rust, environment system access
- **Risks:** Minimal; environment variables are typically exposed

**Critical Path:** 4.1 → 4.2 (sequential); 4.3 and 4.4 can run parallel  
**Parallel Opportunities:** Task 4.3 + Task 4.4 + Task 4.1 (independent subsystems)  
**Resource Loading:** 1 Senior + 2 Mid developers, weeks 20–25

---

### 10.6 Phase 5: Automation, Persistence, Export & External Telemetry

**Phase Duration:** 4.5–6.5 weeks  
**Total Effort:** 180–260 person-hours

| Task ID | Task Name | Complexity | Est. Hours | Risk | Dependencies |
|:--------|:----------|:-----------|:-----------|:-----|:-------------|
| 5.1 | Persistent Layouts & User Preferences (I1) | Core | 16–24 | LOW | Phase 1 |
| 5.2 | Automated Script Testing via OMSI_INPUT (I2) | Standard+ | 28–40 | MEDIUM | Phase 2 |
| 5.3 | Single-Click glTF 2.0 Export (I3, I+) | Advanced++ | 56–80 | CRITICAL | Phase 1, 3 |
| 5.4 | Remote Telemetry WebSocket Server (I+) | Advanced+ | 40–56 | HIGH | Phase 2 |
| 5.5 | Non-Destructive Editor Handover Bridge (F, F+) | Standard+ | 32–44 | MEDIUM | Phase 1 |
| 5.T | Phase 5 End-to-End Testing | Standard | 20–28 | MEDIUM | All tasks |
| 5.D | Phase 5 Documentation & User Guide | Core | 12–16 | LOW | All tasks |

**Task 5.1: Persistent Layouts & User Preferences (I1)**
- Serialize panel geometry, collapsed sections, active tabs
- JSON persistence to `~/.config/openomsi/inspector_layout.json`
- Watch table and filter string persistence
- **Skills Required:** Junior Rust, `serde` serialization, file I/O
- **Risks:** Minimal; straightforward serialization

**Task 5.2: Automated Script Testing via OMSI_INPUT (I2)**
- `OMSI_INPUT` command grammar integration
- Headless test automation (select, assert, set variable, screenshot)
- **Skills Required:** Mid Rust, command parsing, automation framework
- **Risks:** Requires `OMSI_INPUT` infrastructure extension

**Task 5.3: Single-Click glTF 2.0 Export (I3, I+)**
- Export selected entity to `.glb` with transforms, materials, textures
- Resolve world/local transforms, vertex colors, PBR properties
- Embedded PNG texture baking
- **Skills Required:** Senior Rust, glTF 2.0 spec, texture/mesh export pipeline
- **Risks:** glTF export requires deep asset format conversion; texture baking is complex

**Task 5.4: Remote Telemetry WebSocket Server (I+)**
- WebSocket server on `--telemetry-port 9002`
- 30 Hz JSON broadcast of vehicle state, script variables, watch table values
- **Skills Required:** Senior Rust, WebSocket libraries (`tokio-tungstenite`), networking
- **Risks:** WebSocket threading model must not block simulation; requires async runtime coordination

**Task 5.5: Non-Destructive Editor Handover Bridge (F, F+)**
- "Promote to Object Editor" atomic transition
- Non-destructive transform sandbox with live delta display
- "Revert" and "Commit to Map" transactional workflow
- **Skills Required:** Mid Rust, editor mode state machine, UI state management
- **Risks:** Editor handover requires careful state synchronization

**Critical Path:** 5.3 (longest task); others can run in parallel  
**Parallel Opportunities:** 5.1 + 5.2 + 5.4 + 5.5 (independent subsystems)  
**Resource Loading:** 2 Senior + 1 Mid developer, weeks 26–31

---

### 10.7 Testing & QA Overhead Summary

| Phase | Unit Tests (hrs) | Integration Tests (hrs) | Performance Profiling (hrs) | Total QA (hrs) |
|:------|:-----------------|:------------------------|:----------------------------|:---------------|
| Phase 1 | 12–16 | 8–12 | 4–6 | 24–34 |
| Phase 2 | 16–20 | 12–16 | 6–8 | 34–44 |
| Phase 3 | 14–18 | 10–14 | 8–12 | 32–44 |
| Phase 4 | 10–14 | 6–10 | 4–6 | 20–30 |
| Phase 5 | 12–16 | 8–12 | 4–6 | 24–34 |
| **Total** | **64–84** | **44–64** | **26–38** | **134–186** |

**QA Strategy:**
- Unit tests: Core data structures, snapshot isolation, key generation
- Integration tests: Full inspector workflow scenarios (select, inspect, override, export)
- Performance profiling: Frame time budgets (< 1.0ms overhead), BVH raycast latency (< 0.4ms)
- Stress testing: 30 AI buses, Spandau map, heavy rain, all inspector tabs active

---

### 10.8 Documentation Burden Estimates

| Documentation Type | Estimated Hours | Deliverables |
|:-------------------|:----------------|:-------------|
| Inline Code Documentation | 40–60 | Rustdoc comments for all public APIs |
| Architecture Documentation | 24–32 | `docs/architecture/inspector-advanced.md` |
| User Guide & Tutorials | 32–48 | `docs/user-guide/inspector.md`, video walkthroughs |
| API Reference | 16–24 | Auto-generated Rustdoc with examples |
| Internationalization (i18n) | 20–28 | 24-language translation keys in `locales/app.yml` |
| **Total Documentation** | **132–192** | Comprehensive knowledge base |

---

### 10.9 Critical Path Analysis

**Primary Critical Path (Sequential Dependency Chain):**
```
Phase 1: Task 1.1 → Task 1.2
         ↓
Phase 2: Task 2.2 → Task 2.3 → Task 2.4
         ↓
Phase 3: Task 3.1 → Task 3.3
         Task 3.4 → Task 3.5
         ↓
Phase 4: Task 4.1 → Task 4.2
         ↓
Phase 5: Task 5.3 (glTF Export)
```

**Total Critical Path Duration:** 18.5–26.5 weeks (4.6–6.6 months)

**Blocking Dependencies:**
- **Task 1.1 (BVH):** Blocks all raycasting work (1.2, 1.3, all downstream phases)
- **Task 2.2 (Script Inspector):** Blocks all script VM features (2.3, 2.4, 2.5)
- **Task 3.1 (Material/Texture):** Blocks texture diagnostics (3.2, 3.3)
- **Task 4.1 (Spline Geometry):** Blocks traffic/signal features (4.2)

**Derisking Strategies:**
- Early prototype of Task 1.1 (BVH) in Phase 0.5 (pre-Phase 1)
- Parallel Task 2.1 development while Task 2.2–2.4 progresses
- Independent Phase 4 and Phase 5 task streams where possible

---

### 10.10 Parallel Work Opportunities

**High Parallelism Windows:**
1. **Phase 1 (Week 3–5):** Task 1.2 UI + Task 1.3 (different developers)
2. **Phase 2 (Week 7–10):** Task 2.1 + Task 2.2/2.3 (independent subsystems)
3. **Phase 3 (Week 14–17):** Task 3.1/3.2 + Task 3.4 (rendering vs materials)
4. **Phase 4 (Week 20–23):** Task 4.1 + Task 4.3 + Task 4.4 (world, audio, environment)
5. **Phase 5 (Week 26–29):** Task 5.1 + 5.2 + 5.4 + 5.5 (all independent)

**Optimal Team Composition:**
- **2 Senior Rust Engineers:** GPU/rendering, simulation internals, performance-critical paths
- **2 Mid-Level Engineers:** UI development, integration work, testing
- **1 Junior Engineer:** Documentation, i18n, simple serialization tasks

---

### 10.11 Total Project Timeline & Confidence Intervals

| Metric | Optimistic (P10) | Expected (P50) | Pessimistic (P90) |
|:-------|:-----------------|:---------------|:------------------|
| **Total Engineering Hours** | 900 | 1,200 | 1,600 |
| **Calendar Duration** | 18 weeks | 24 weeks | 32 weeks |
| **Team Size** | 3 developers | 3–4 developers | 4–5 developers |
| **Full-Time Equivalent (FTE)** | 2.5 FTE | 3.0 FTE | 3.5 FTE |

**Confidence Intervals:**
- **P10 (Optimistic):** Assumes perfect knowledge of OMSI internals, no reverse-engineering delays, zero GPU pipeline rework
- **P50 (Expected):** Accounts for moderate unknowns, typical reverse-engineering friction, one GPU pipeline refactor
- **P90 (Pessimistic):** Includes major reverse-engineering blocks, two GPU pipeline refactors, scope expansion

**Risk-Adjusted Timeline:**  
**Expected delivery: 5.5–6.5 months** with 3 full-time developers

---

### 10.12 Resource Loading Chart (Person-Weeks per Phase)

| Phase | Senior Engineer | Mid Engineer | Junior Engineer | Total Person-Weeks |
|:------|:----------------|:-------------|:----------------|:-------------------|
| Phase 1 | 5.0–7.0 | 3.5–5.0 | 0.5–1.0 | 9.0–13.0 |
| Phase 2 | 7.5–10.0 | 4.0–6.0 | 1.0–1.5 | 12.5–17.5 |
| Phase 3 | 7.0–9.5 | 4.5–6.5 | 0.5–1.0 | 12.0–17.0 |
| Phase 4 | 5.0–7.0 | 3.5–5.0 | 0.5–1.0 | 9.0–13.0 |
| Phase 5 | 6.0–8.5 | 4.0–5.5 | 1.0–1.5 | 11.0–15.5 |
| **Total** | **30.5–42.0** | **19.5–28.0** | **3.5–6.0** | **53.5–76.0** |

**Visual Resource Distribution:**
```
Phase 1:  ████████████░░░░░░░░  (9–13 person-weeks)
Phase 2:  ████████████████░░░░  (12.5–17.5 person-weeks)
Phase 3:  ███████████████░░░░░  (12–17 person-weeks)
Phase 4:  ████████████░░░░░░░░  (9–13 person-weeks)
Phase 5:  ██████████████░░░░░░  (11–15.5 person-weeks)
```

---

### 10.13 Phase Completion Milestones & Deliverables

| Phase | Milestone | Acceptance Criteria |
|:------|:----------|:--------------------|
| **Phase 1** | Spatial Selection Foundation | Multi-hit raycasting < 0.4ms; pedestrian selection; Tab cycling working |
| **Phase 2** | Script VM Observatory | Live variable overrides; sparklines; time-travel scrubbing functional |
| **Phase 3** | Render Pipeline Control | Texture previews; mipmap heatmap; mesh isolation; < 1.0ms frame overhead |
| **Phase 4** | World Infrastructure | Spline inspection; traffic paths; audio cones; weather probe complete |
| **Phase 5** | Automation & Export | glTF export; WebSocket telemetry; editor handover; persistent layouts |

**Final Acceptance Gate:**
- All 25 tasks green in verification matrix (Section 6)
- Performance budget satisfied: < 1.0ms overhead, < 32 MB memory
- Zero frame stalls, zero data races, zero undefined behavior
- Full i18n coverage across 24 languages
- Comprehensive Rustdoc and user guide documentation

---

### 10.14 Recommendations & Risk Mitigation

1. **Front-Load BVH Development:** Implement Task 1.1 as a pre-Phase 1 prototype (2 weeks) to derisk spatial acceleration unknowns
2. **Parallel Phase 2 & 3 Streams:** After Phase 1, split team to work Phase 2 (script VM) and Phase 3 (rendering) simultaneously
3. **GPU Pipeline Freeze:** Lock `omsi-render` pipeline changes during Phase 3 to prevent integration conflicts
4. **Continuous Performance Profiling:** Weekly frame time budget checks; automated performance regression tests in CI
5. **Incremental Integration:** Merge each phase into `main` behind feature flags; enable progressively for dogfooding
6. **Documentation-Driven Development:** Write user guide sections alongside implementation to catch UX issues early

**Total Estimated Cost:** 1,200 person-hours ± 300 hours (3–4 developer-months)  
**Recommended Team:** 2 Senior + 2 Mid + 1 Junior (3.0 FTE)  
**Timeline:** 24 weeks (6 months) with moderate risk buffer
```

## 11. Appendices & Reference Material

### 11.1 Glossary of Key Terms

**BVH (Bounding Volume Hierarchy):** Spatial acceleration data structure organizing scene geometry into nested bounding boxes to optimize raycast performance. Enables sub-millisecond intersection tests against hundreds of thousands of triangles by culling entire branches that cannot intersect the query ray.

**Broadphase / Narrowphase:** Two-tier collision detection strategy. Broadphase performs fast, conservative bounding volume tests (AABB, cylinder, sphere) to generate candidate sets. Narrowphase executes precise triangle-level raycasts only against broadphase survivors.

**Depth Peeling:** Multi-pass rendering technique capturing successive depth layers of a scene. Enables visualization of occluded geometry by progressively "peeling away" front-to-back surface layers, revealing internal structure without geometric clipping.

**Generational Key / Handle:** Versioned entity identifier combining a reusable index slot with a monotonically increasing generation counter (`{ id: usize, generation: u64 }`). Prevents aliasing when entities are despawned and their slots reused—stale references fail validation against the current generation.

**glTF (GL Transmission Format):** Khronos standard for 3D asset interchange (`.gltf` JSON + `.bin` buffers, or single-file `.glb`). Encodes mesh geometry, PBR materials, textures, skeletal hierarchies, and animations in a runtime-ready, vendor-neutral format.

**HumanKey:** Generational handle identifying pedestrian/passenger entities: `{ id: usize, generation: u64, is_driver: bool }`. Distinguishes seated bus drivers from standing/walking passengers and crowd agents.

**IBIS (Integrated Board Information System):** German bus passenger information standard. OMSI simulates authentic IBIS displays showing route numbers, destination text, intermediate stops, and connection information rendered via dynamic scripttextures.

**L.var / S.var:** OMSI script variable naming conventions. `L.` prefix denotes local floating-point variables (e.g., `L.engine_rpm`, `L.door_state`). `S.` prefix denotes string variables (e.g., `S.route_number`, `S.destination_text`).

**Mipmap:** Precomputed sequence of progressively lower-resolution texture images ($1024 \times 1024 \to 512 \times 512 \to 256 \times 256 \to \dots$). GPU selects appropriate mip level based on screen-space texel density, preventing aliasing and improving cache coherency.

**PBR (Physically-Based Rendering):** Shading model grounded in physical light transport equations. Materials defined by albedo (base color), roughness (micro-surface variance), metalness (conductor/dielectric classification), and normal maps. Produces consistent, energy-conserving appearance under varying lighting.

**RPN (Reverse Polish Notation):** Stack-based expression evaluation system where operators follow operands: `3 4 + 5 *` evaluates as $(3 + 4) \times 5 = 35$. OMSI's `.osc` script bytecode uses RPN for arithmetic, logic, and macro invocations without explicit operator precedence rules.

**SceneryKey:** Generational handle for placed map objects: `{ tile_x: i32, tile_y: i32, instance_id: usize, generation: u64 }`. Tracks buildings, bus stops, trees, and static scenery across OMSI's tile-based streaming system.

**ScriptTexture / TextTexture:** OMSI dynamic texture rendering system. `[scripttexture]` blocks define GPU-resident framebuffers updated by script logic (matrix displays, rollbands). `[texttexture]` blocks render anti-aliased text strings into textures using configurable fonts for destination blinds and IBIS screens.

**Snapshot Isolation:** Concurrency pattern where UI queries observe a consistent, immutable point-in-time copy of simulation state. Eliminates lock contention and data races by decoupling read-only inspection from active simulation updates across frame boundaries.

**SplineKey:** Generational handle for road geometry: `{ tile_x: i32, tile_y: i32, spline_id: usize }`. Identifies individual road segments (`.sli` files) within OMSI's tile coordinate system, each encoding cross-sectional profile, curvature, banking, and traffic path metadata.

**SSAO (Screen-Space Ambient Occlusion):** Post-processing technique darkening surface regions occluded from ambient sky lighting. Samples depth buffer in screen space to estimate local geometric concavity, enhancing depth perception in crevices, corners, and contact shadows.

**Tile Pin Lease:** Reference-counted lock preventing streaming eviction of a map tile currently under inspection. Ensures scenery objects remain resident in memory while selected, automatically released when inspector focus shifts or inspector mode exits.

**VehicleKey:** Generational handle for buses and articulated sections: `{ id: usize, generation: u64, is_trailer: bool }`. Distinguishes lead vehicles from coupled trailers and prevents handle reuse collisions when AI buses despawn.

**VRAM (Video Random Access Memory):** Dedicated GPU-resident memory storing textures, vertex buffers, uniform blocks, and framebuffer attachments. Inspector queries VRAM allocation sizes and texture formats via async staging buffers to avoid pipeline stalls.

**wgpu:** Rust graphics abstraction layer providing safe, cross-platform access to modern GPU APIs (Vulkan, Metal, DirectX 12, WebGPU). openOMSI's rendering pipeline built atop wgpu for portable, high-performance 3D graphics without manual memory management.

**Wireframe Mode:** Debug visualization replacing solid triangle rasterization with edge-only line rendering (`wgpu::PolygonMode::Line`). Exposes mesh topology, triangle density, and geometric structure for authoring and optimization workflows.

---

### 11.2 OMSI Script VM Reference

#### Overview
OMSI's scripting system employs a stack-based virtual machine executing Reverse Polish Notation (RPN) bytecode. Scripts define vehicle behavior (engine physics, door interlocks, electrical systems), dynamic displays (matrix rollbands, IBIS terminals), and scenery interactions through `.osc` plaintext files compiled at asset load time.

#### Variable System
**Local Variables (`L.` prefix):**
- Floating-point values (64-bit IEEE 754 doubles internally).
- Scoped to individual vehicle or scenery object instances.
- Examples: `L.engine_rpm`, `L.throttle`, `L.door_state`, `L.air_pressure_bar`.
- Persistent across frames; retain values until explicitly overwritten.

**String Variables (`S.` prefix):**
- UTF-8 text strings for display rendering and conditional logic.
- Examples: `S.route_number`, `S.destination_text`, `S.driver_name`.
- Used extensively in `[texttexture]` blocks for dynamic text rendering.

**System Variables (Predefined):**
- Read-only global state exposed by OMSI engine: `$Time`, `$DeltaTime`, `$CameraDistance`, `$WeatherRain`, `$SunAzimuth`.
- Vehicle-specific: `$VehicleVelocity`, `$SteeringAngle`, `$EngineRunning`, `$Gear`.

#### RPN Stack Evaluation
**Arithmetic Operators:** `+`, `-`, `*`, `/`, `%` (modulo), `^` (power).
```
{code}
(L.speed_ms) 3.6 *         // Convert m/s to km/h: speed_ms × 3.6
(L.voltage) 12.0 / 100.0 *  // Normalize voltage: (voltage / 12.0) × 100%
{code}
```

**Comparison & Logic:** `<`, `>`, `<=`, `>=`, `==`, `!=`, `&&` (and), `||` (or), `!` (not).
```
{code}
(L.door_state) 0.0 >        // True if door open (state > 0.0)
(L.engine_rpm) 800 >= (L.throttle) 0.1 > && // RPM ≥ 800 AND throttle > 10%
{code}
```

**Conditional Execution:**
```
{code}
{if} (L.engine_running) 1.0 ==
    (L.fuel_consumption) (L.throttle) 0.05 * (S.FuelConsumption) + (S.FuelConsumption)
{else}
    0.0 (S.FuelConsumption)
{endif}
```

#### Macro System
**Macro Definition & Invocation:**
```
{code}
{macro:Engine_Calc}
    (L.throttle) (L.engine_max_rpm) * (L.engine_rpm) (S.EngineRPM)
    (L.engine_rpm) 100.0 / (L.sound_pitch_engine)
{endmacro}

// Invocation in frame loop:
{frame}
    {macro:Engine_Calc}
{endframe}
```

**System Macros (Predefined):**
- `{macro:SetTexture}` — Bind texture to material slot.
- `{macro:PlaySound}` — Trigger spatial audio emission.
- `{macro:SetMatrix}` — Update 3D transform matrix.

#### Trigger System
**Event-Driven Callbacks:**
```
{code}
{trigger:door_front_open}
    1.0 (L.door_front_state)
    {macro:PlaySound} sound_door_hiss.wav 1.0 0.5
{endtrigger}

{trigger:collision}
    (L.collision_severity) (L.structural_damage) + (L.structural_damage)
{endtrigger}
```

**Common Triggers:**
- `{trigger:key_press}` — Keyboard input (wired in `.cfg` files).
- `{trigger:mouse_click}` — 3D cockpit switch activation.
- `{trigger:collision}` — Physics contact event.
- `{trigger:script_start}` — Initialization (runs once at spawn).

#### Frame Execution Contexts
**{init}:** Executes once at entity spawn. Initializes variables, loads resources.
**{frame}:** Executes every simulation tick (~60 Hz). Core state machine logic.
**{frame_ai}:** Executes only for AI-driven vehicles (player vehicles skip this block).

#### Script File Organization
Typical vehicle script structure:
```
Vehicles/MAN_SD200/Script/
├── constants.osc       // Physical constants, gear ratios, dimensions
├── engine.osc          // Combustion, torque curves, fuel consumption
├── transmission.osc    // Gearbox, clutch, differential
├── brakes.osc          // Pneumatic brake system, ABS, retarder
├── doors.osc           // Interlocks, passenger sensors, animations
├── cockpit.osc         // Gauges, switches, indicator lamps
├── matrix.osc          // Destination display scripttexture logic
└── sound.osc           // Audio trigger conditions, pitch modulation
```

---

### 11.3 Coordinate System & Transform Conventions

#### World Coordinate System
- **Origin:** Arbitrary world-space reference point (typically map center or depot spawn).
- **Axes:** Right-handed Cartesian system:
  - **+X:** East
  - **+Y:** Up (altitude above sea level)
  - **+Z:** South
- **Units:** Meters (m) for translation; degrees (°) or radians for rotation.

#### Local / Model-Space Coordinates
- **Vehicle Basis:** Origin at chassis reference point (typically rear axle center or center of mass).
  - **+X:** Vehicle right (starboard).
  - **+Y:** Vehicle up (roof).
  - **+Z:** Vehicle forward (front bumper direction).
- **Component Offsets:** Mesh parts (wheels, doors, mirrors) defined relative to parent vehicle basis.

#### Tile Coordinate System
- **Grid Structure:** OMSI maps partitioned into $300\text{m} \times 300\text{m}$ square tiles.
- **Tile Indexing:** Integer coordinates `(tile_x, tile_y)` where each tile contains:
  - Terrain mesh (`.map` files).
  - Placed scenery objects (`.sco` placement records).
  - Road splines (`.sli` geometry).
- **Streaming:** Tiles dynamically load/unload based on camera proximity. 3×3 tile grid (9 tiles) typically resident around player position.

#### Rotation Conventions
- **Euler Angles:** Yaw-Pitch-Roll (ZYX extrinsic rotation order):
  - **Yaw:** Rotation about world +Y axis (heading, azimuth). $0° =$ North, $90° =$ East.
  - **Pitch:** Rotation about local +X axis (nose up/down). Positive = nose up.
  - **Roll:** Rotation about local +Z axis (bank, tilt). Positive = right wing down.
- **Quaternions:** Internal representation for interpolation and concatenation: $q = w + xi + yj + zk$.

#### Transform Hierarchy
```
World Space
  └─ Tile (tile_x, tile_y)
       ├─ Scenery Object Instance
       │    └─ Mesh Part (local offset)
       └─ Vehicle (Lead)
            ├─ Chassis
            │    ├─ Wheels [4x]
            │    ├─ Doors [3x]
            │    └─ Cockpit
            │         ├─ Steering Wheel
            │         ├─ Speedometer Needle
            │         └─ Switches [20+]
            └─ Trailer (articulated)
                 ├─ Hitch Joint
                 └─ Trailer Chassis
```

#### Inspector Transform Display
- **World Pose:** Absolute position in map coordinate space.
- **Local Pose:** Relative transform within parent hierarchy (e.g., speedometer needle pivot relative to dashboard).
- **Concatenated Matrices:** $M_{\text{world}} = M_{\text{tile}} \cdot M_{\text{vehicle}} \cdot M_{\text{part}}$.

---

### 11.4 Entity Key Schema Reference

#### VehicleKey
```rust
pub struct VehicleKey {
    pub id: usize,           // Slot index in vehicle pool (reusable)
    pub generation: u64,     // Monotonic counter, increments on despawn
    pub is_trailer: bool,    // True for articulated trailer sections
}
```

**Lifecycle:**
1. Vehicle spawns → allocates slot `id`, assigns `generation = current_gen`.
2. Inspector captures key → snapshot holds `(id, generation)` pair.
3. Vehicle despawns → slot `id` freed, `generation` incremented.
4. New vehicle reuses slot `id` → receives new `generation = current_gen + 1`.
5. Inspector validation → compares snapshot `generation` against pool's current generation. Mismatch → selection invalidated.

**Usage:**
- Distinguishes player bus, AI buses, remote multiplayer buses.
- Prevents inspecting stale handles after AI vehicle leaves streaming radius.

#### SceneryKey
```rust
pub struct SceneryKey {
    pub tile_x: i32,
    pub tile_y: i32,
    pub instance_id: usize,
    pub generation: u64,
}
```

**Tile-Local Addressing:**
- `(tile_x, tile_y)` identifies containing map tile.
- `instance_id` indexes into tile's scenery placement array.
- `generation` prevents aliasing when tile unloads/reloads with modified placement.

**Tile Pin Lease:**
- Selecting scenery object increments tile's reference count.
- Tile streaming system skips eviction of pinned tiles.
- Reference released on inspector exit or selection change.

#### HumanKey
```rust
pub struct HumanKey {
    pub id: usize,
    pub generation: u64,
    pub is_driver: bool,     // True for seated bus driver, false for passengers/pedestrians
}
```

**Agent Types:**
- **Driver:** Single seated driver per vehicle, animates steering/pedal inputs.
- **Passengers:** Boarding, seated, or alighting agents inside bus cabin.
- **Pedestrians:** Autonomous crowd agents pathfinding across sidewalks and crosswalks.

**Skeletal Animation:**
- Each human instance references `.hum` character model file.
- Animation state machine (idle, walk, sit, board, alight) drives bone transforms.

#### SplineKey
```rust
pub struct SplineKey {
    pub tile_x: i32,
    pub tile_y: i32,
    pub spline_id: usize,    // Index into tile's road spline array
}
```

**Road Geometry:**
- Each spline represents a continuous road segment with parametric curve.
- Cross-section profile (lanes, curbs, sidewalks) extruded along centerline.
- Metadata: Speed limit, traffic lanes, bus-only restrictions, banking angle.

**No Generation Counter:**
- Spline identity stable once tile loads (road network rarely changes dynamically).
- Tile unload implicitly invalidates all contained spline keys.

---

### 11.5 Performance Measurement Methodology

#### Frame Time Budget
**Target Overhead:** $< 1.0\text{ ms}$ total inspector cost per frame (at 60 FPS = $16.67\text{ ms}$ frame budget).

**Measurement Points:**
```rust
let t_start = Instant::now();
// Inspector raycast broadphase
let t_broadphase = Instant::now();
// Inspector narrowphase triangle tests
let t_narrowphase = Instant::now();
// Snapshot construction
let t_snapshot = Instant::now();
// UI rendering
let t_ui = Instant::now();

eprintln!("Inspector Timing: Broadphase {:.3}ms | Narrowphase {:.3}ms | Snapshot {:.3}ms | UI {:.3}ms | Total {:.3}ms",
    t_broadphase.duration_since(t_start).as_secs_f64() * 1000.0,
    t_narrowphase.duration_since(t_broadphase).as_secs_f64() * 1000.0,
    t_snapshot.duration_since(t_narrowphase).as_secs_f64() * 1000.0,
    t_ui.duration_since(t_snapshot).as_secs_f64() * 1000.0,
    t_ui.duration_since(t_start).as_secs_f64() * 1000.0);
```

#### Profiling Tools
**Rust Built-In:**
- `cargo flamegraph` — CPU flamegraph generation via `perf` (Linux) or DTrace (macOS).
- `cargo bench` — Criterion.rs microbenchmarks for raycast performance.

**GPU Profiling:**
- `wgpu` timestamp queries — Measure individual render pass execution time on GPU timeline.
- RenderDoc — Frame capture and analysis tool for graphics pipeline inspection.
- NVIDIA Nsight Graphics / AMD Radeon GPU Profiler — Vendor-specific GPU profilers.

#### Benchmark Scenarios
**Scenario 1: Idle Inspector (Baseline)**
- Inspector mode active, no selection, static camera.
- **Target:** $< 0.1\text{ ms}$ overhead (cursor hover only).

**Scenario 2: Dense Mesh Selection**
- Raycast into articulated bus interior (40+ unique mesh parts, 150k triangles total).
- **Target:** Broadphase $< 0.2\text{ ms}$, narrowphase $< 0.3\text{ ms}$.

**Scenario 3: Multi-Hit Corridor**
- Raycast through 8+ layered surfaces (windshield, dashboard, seats, bulkhead, rear door).
- **Target:** Full corridor generation $< 0.5\text{ ms}$.

**Scenario 4: Live Variable Inspection**
- 50+ watched variables with sparkline history (120 samples each).
- **Target:** UI update $< 0.3\text{ ms}$.

**Scenario 5: Dynamic Texture Preview**
- Inspect scripttexture with 512×512 RGBA8 backing texture.
- **Target:** Async staging copy $< 0.4\text{ ms}$ (amortized over 3 frames).

#### Stress Test Configuration
**Map:** Spandau (large urban environment, 40+ active tiles).
**Traffic:** 30 AI buses, 80 cars, 120 pedestrians.
**Weather:** Heavy rain (particle systems, wet surface shaders).
**Inspector Load:** All tabs active, 16 watched variables, texture preview open.
**Target:** Maintain $< 1.0\text{ ms}$ overhead across 60-second driving session.

---

### 11.6 Testing Strategy Overview

#### Unit Test Coverage Requirements
**Mandatory Coverage (≥95%):**
- Generational key validation logic (`VehicleKey`, `SceneryKey`, `HumanKey`, `SplineKey`).
- Multi-hit raycast corridor sorting and filtering.
- Variable override clamping and bounds validation.
- Tile pin lease reference counting.
- Snapshot isolation and atomic construction.

**Example Test:**
```rust
#[test]
fn test_vehicle_key_generation_invalidation() {
    let mut pool = VehiclePool::new();
    let key1 = pool.spawn_vehicle(/* ... */);
    assert!(pool.is_valid(key1));
    
    pool.despawn_vehicle(key1.id);
    assert!(!pool.is_valid(key1)); // Stale generation
    
    let key2 = pool.spawn_vehicle(/* ... */);
    assert_eq!(key2.id, key1.id); // Slot reused
    assert_ne!(key2.generation, key1.generation); // New generation
}
```

#### Integration Test Scenarios
**Test 1: End-to-End Selection Pipeline**
- Spawn test vehicle, emit raycast at known screen coordinates, verify correct mesh selected.

**Test 2: Snapshot Consistency Under Concurrent Modification**
- Main thread updates vehicle position at 60 Hz, inspector thread queries snapshot → verify no data races or torn reads.

**Test 3: Tile Pin Lifecycle**
- Select scenery object, verify tile pin held, move camera far away, verify tile remains loaded, deselect, verify tile streams out.

**Test 4: Variable Override Rollback**
- Set `L.throttle = 0.85`, drive for 5 seconds, click "Reset Overrides", verify throttle returns to player input control.

**Test 5: Export Integrity**
- Select vehicle part, export to `.glb`, load in Blender via automated script, verify vertex count and material properties match.

#### Stress Test Configurations
**Configuration A: High Entity Density**
- 100 scenery objects in 300m radius, 20 AI buses, 50 pedestrians.
- Measure: Raycast time, snapshot construction time, memory overhead.

**Configuration B: High Script Complexity**
- Vehicle with 200+ script variables, 10 active macros, 5 scripttextures.
- Measure: Variable enumeration time, watch table update latency, sparkline rendering cost.

**Configuration C: Rapid Selection Churn**
- Automated script cycling through 100 different entities per second.
- Measure: Key validation throughput, tile pin churn overhead, UI responsiveness.

---

### 11.7 Related Documents & Prior Work

#### Internal Documentation
**MVP Specification:**
- `docs/superpowers/specs/2026-09-XX-inspector-mvp-spec.md` (Tasks 1–10)
- Baseline feature set: Cursor raycasting, vehicle/scenery selection, generational keys, basic property display.

**Upstream Synchronization:**
- Commit `e8f101c`: 53-commit merge from `turbo-devv/openOMSI`.
- Integration of new render pipeline, audio subsystem, and human agent pathfinding.

**Crate Architecture:**
- `docs/architecture/crate-structure.md` — Module organization and dependency graph.
- `docs/architecture/render-pipeline.md` — wgpu render pass choreography and frame graph.
- `docs/architecture/simulation-loop.md` — Fixed timestep physics, variable framerate rendering decoupling.

#### OMSI 2 Modding Resources
**Official Aerosoft Documentation:**
- *OMSI 2 SDK Manual* (German/English) — `.osc` script syntax, `.cfg` configuration format, `.o3d` mesh specification.
- *Spline Editor Guide* — Road network authoring, traffic path definition, signal controller setup.

**Community Resources:**
- [OMSI-WebDisk](https://www.omnibussimulator.de/) — Model templates, texture libraries, script examples.
- [Content Creation Forum](https://www.omnibussimulator.de/forum/) — Modder Q&A, troubleshooting guides.

**Third-Party Tools:**
- **Blender OMSI Plugin** — `.o3d` import/export, LOD configuration, collision hull setup.
- **OMSI Font Editor** — `.oft` bitmap font authoring for texttexture rendering.

#### Academic & Technical References
**Spatial Acceleration Structures:**
- *Real-Time Rendering, 4th Edition* (Akenine-Möller et al.) — Chapter 19: Acceleration Structures.
- "On Fast Construction of SAH-based Bounding Volume Hierarchies" (Wald, 2007).

**Physically-Based Rendering:**
- *Physically Based Rendering: From Theory to Implementation* (Pharr et al.) — BSDF theory, light transport integration.
- "Real Shading in Unreal Engine 4" (Karis, 2013) — Practical PBR material model.

**Graphics API Abstraction:**
- [wgpu Documentation](https://wgpu.rs/) — Safe, portable Rust graphics API.
- [WebGPU Specification](https://www.w3.org/TR/webgpu/) — W3C standard underlying wgpu.

---

### 11.8 Change Log & Document History

#### Version 1.0 (2026-09-30)
**Initial Release**
- Comprehensive specification for Inspector Extras & Scope-Creep features.
- Detailed subsystem designs: E/E+ (Human Kinematics), A/A+ (Materials & Textures), B/B+ (Script VM), C/C+ (Render Pipeline), G/G+ (Penetration Stacks), I/I+ (Automation & Export), D/D+ (Splines & Traffic), H/H+ (Audio & Environment), F/F+ (Editor Bridge).
- Five-phase implementation roadmap with 25 SDD tasks.
- Verification matrix with 14 acceptance criteria.
- Architectural guarantees: Zero-stall GPU, snapshot isolation, fail-closed safety, <1.0ms frame budget.

**Authors:**
- Primary Architect: [openOMSI Core Team]
- Technical Review: [Rendering Lead, Simulation Lead]
- Document Assembly: AI-Assisted Specification Generation

**Baseline Context:**
- MVP Commit: `e8f101c` (10 foundational tasks merged)
- Upstream Sync: 53 commits from `turbo-devv/openOMSI`
- Target Crates: `omsi-app`, `omsi-render`, `omsi-sim`, `omsi-geometry`, `omsi-model`, `omsi-scenery`

#### Future Revisions
**Planned Updates:**
- **Version 1.1:** Refinements based on Phase 1 implementation feedback (BVH tuning, raycast optimization results).
- **Version 1.2:** Integration with multiplayer inspector mode (remote entity inspection protocol).
- **Version 2.0:** VR/XR inspector mode specification (spatial UI, hand-tracked selection).

**Change Request Process:**
1. Propose modification via GitHub Issue tagged `[spec-revision]`.
2. Technical review by subsystem domain experts.
3. Approval requires consensus from 2+ core maintainers.
4. Increment version number, document rationale in change log.

**Document Maintenance:**
- Living document; updated in lockstep with implementation milestones.
- Major architectural changes require new major version (2.0, 3.0).
- Clarifications and example additions increment minor version (1.1, 1.2).

---

**End of Appendices**


## 12. Conclusion & Strategic Vision

The **openOMSI Unified Visual Debug Inspector** represents a fundamental paradigm shift in bus simulation modding: from opaque, trial-and-error authoring toward observable, debuggable, and introspectable transit systems engineering. What began as a modest MVP—point, click, read a transform—has evolved through deliberate scope creep into a comprehensive **simulation observatory and authoring bridge** that exposes every layer of the OMSI runtime: script VM state machines, PBR material pipelines, skeletal animation graphs, acoustic propagation, traffic pathfinding, and real-time render pass telemetry. This transformation unlocks modding productivity and debugging capabilities categorically impossible in OMSI 2, where cryptic `.osc` failures, texture misconfigurations, and spline connectivity errors forced modders into blind iteration cycles lasting days or weeks.

Despite this ambitious feature surface—spanning eight major subsystems (E, A, B, C, G, I, D, H) and their advanced scope-creep tiers—the architecture maintains unwavering discipline through the **Four Inviolable Pillars**: zero-stall GPU pipelines, snapshot isolation, fail-closed integrity, and sub-millisecond frame overhead. Every capability, from time-travel script debugging to real-time PBR parameter sandboxes, operates within strict performance envelopes and transactional safety boundaries, ensuring the inspector remains a production-grade diagnostic tool rather than a destabilizing experiment. The value proposition extends across stakeholder groups: bus sim enthusiasts gain unprecedented insight into vehicle behavior and map quality; scenery designers and vehicle modders eliminate guesswork through live material tweaking, collision hull overlays, and glTF export; CI/CD pipelines leverage headless validation (`--inspect-check`) for automated asset auditing; and educational or research use cases benefit from telemetry streams and trace replay capabilities. Looking forward, this inspector becomes a **platform for transformative capabilities**—AI-assisted modding through semantic scene understanding, procedural generation guided by real-time constraint validation, and cloud-based collaborative editing where distributed teams inspect and iterate on shared transit networks in real time. The five-phase SDD roadmap provides a clear, executable path from spatial raycasting foundations through simulation observability, rendering diagnostics, infrastructure probing, and finally automation and external integration. We call on the openOMSI development community to commit to phased implementation following this specification, transforming a niche debugging tool into the cornerstone of a modern, observable, and collaborative transit simulation ecosystem.
