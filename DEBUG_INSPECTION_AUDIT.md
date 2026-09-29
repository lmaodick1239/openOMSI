# openOMSI Debugging, Object Selection & Inspection Systems Audit

> **Target Workspace:** `openOMSI` (`lmaodick1239/openOMSI`)  
> **Environment:** Linux x86_64, Rust 1.98.1 Stable, `wgpu` 24.0.1  
> **Date:** March 2025

---

## 1. Executive Summary & Environment Setup

### 1.1 Development Environment Setup
The development environment for `openOMSI` was initialized in accordance with the project requirements and documentation. Because the runtime environment operates in a non-root Linux user space without passwordless `sudo`, system-level C dependencies were extracted directly from distribution packages into `~/.local/usr` and integrated into the compilation search path:

1. **Rust Toolchain:**
   - Installed `rustup` with the latest stable toolchain (**Rust 1.98.1**, `x86_64-unknown-linux-gnu`).
2. **Native C/System Dependencies:**
   - `alsa-sys` (`libasound2-dev` / ALSA headers & libraries)
   - `libudev-dev`
   - `libgtk-3-dev`
   - `libxkbcommon-dev`
   - `libssl-dev`
   - `wayland-protocols`
3. **Environment Configuration (`~/.bashrc` & `~/.cargo/env`):**
   - `PKG_CONFIG_PATH="$HOME/.local/usr/lib/x86_64-linux-gnu/pkgconfig:$HOME/.local/usr/share/pkgconfig"`
   - `LIBRARY_PATH="$HOME/.local/usr/lib/x86_64-linux-gnu:$HOME/.local/usr/lib"`
   - `C_INCLUDE_PATH="$HOME/.local/usr/include"`
4. **Verification:**
   - Clean compilation of all workspace members:
     ```bash
     cargo check --workspace
     cargo check -p omsi-app
     ```
     *Result: Passed with 0 errors across all 15 crates.*

---

## 2. On-Screen Selection & Raycasting Capabilities

`openOMSI` possesses two distinct on-screen selection and picking implementations: the **Scenery Object Editor** (for world-placed scenery) and the **Cockpit Raycaster** (for interactive vehicle switches and dashboard controls).

### 2.1 In-Game Scenery Object Editor (`Ctrl+Shift+E`)
- **Location:** [`crates/omsi-app/src/editor.rs`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-app/src/editor.rs), [`crates/omsi-app/src/app_events.rs`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-app/src/app_events.rs)
- **Activation:** Press `Ctrl+Shift+E` during gameplay or select **"Object editor"** from the in-game Escape menu.
- **Selection Mechanism (`Editor::pick`):**
  - Iterates through all loaded map tiles (`world.loaded_tiles()`).
  - Evaluates each scenery object within a distance range of `1.0m` to `150.0m` along the camera's forward view vector.
  - Computes the angular deviation between the camera look direction and the vector to the object:
    $$\text{score} = \frac{\|(d - f \cdot (d \cdot f))\|}{d \cdot f}$$
  - The object with the lowest angular score ($\text{score} < 0.35$) is selected.
- **Visual Highlight:**
  - The selected object is rendered with a 3D **magenta corona marker** (`color: [1.0, 0.1, 0.9]`, brightness: `2.0`) positioned at the object's origin:
    ```rust
    scene.coronas.push(omsi_render::CoronaInstance {
        position: [p.x as f32, p.y as f32, p.z as f32],
        color: [1.0, 0.1, 0.9],
        size: 0.8,
        brightness: 2.0,
    });
    ```
- **HUD Readout:**
  - Displays object file path, model variant name, world position `(x, y, z)`, heading angle, and host tile coordinate `(tx, ty)`.
- **Manipulation Controls:**
  - **Left Click:** Selects object under crosshair.
  - **Left Drag:** Translates object across the horizontal ground plane ($XY$).
  - **Mouse Wheel:** Rotates object heading in $5^\circ$ increments.
  - **Shift + Mouse Wheel:** Lifts or lowers object along the vertical axis ($Z$) in $0.05\text{m}$ increments.
  - **`C`:** Duplicates selected object at current position.
  - **`V`:** Cycles through available model variants defined in the `.sco` file.
  - **`Delete`:** Removes object from world tile.
  - **`Backspace`:** Undoes modifications.
  - **`Ctrl+S`:** Commits and saves tile file modifications back to disk.

### 2.2 Vehicle Cockpit Raycaster (`hovered_part` & `pick_in`)
- **Location:** [`crates/omsi-app/src/player.rs`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-app/src/player.rs:1060), [`crates/omsi-app/src/input_script.rs`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-app/src/input_script.rs:675)
- **Selection Mechanism:**
  - Takes 2D screen cursor coordinates $(x, y)$, window dimensions, and camera parameters to construct an unprojected 3D ray in world space (`cursor_ray`).
  - Transforms ray into vehicle local coordinate space (accounting for chassis position, rotation, and articulated trailer hitch joint).
  - Performs hierarchical intersection:
    1. **Broadphase:** Checks ray against bounding sphere (`ray_may_hit`).
    2. **Narrowphase:** Tests ray against triangle geometry using Möller–Trumbore intersection in [`omsi_geometry::ray_mesh`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-geometry).
- **Classification & State:**
  - **Interactive Controls (`hover`):** If the intersected mesh possesses a `[mouse_event]` trigger binding in the vehicle configuration, it is classified as a switch/lever.
  - **Non-interactive Geometry (`hover_part`):** If the mesh lacks a mouse trigger, its mesh name is captured as `hover_part`.
- **UI Tooltip & Internationalization:**
  - [`crates/omsi-app/src/describe.rs`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-app/src/describe.rs) translates internal German OMSI script IDs into human-readable descriptions based on active language (`ENG`, `DEU`, `FRA`):
    - Example: `parking_brake_mouse` $\rightarrow$ *"Parking Brake On/Off"*
    - Example: `cp_kneeling_toggle` $\rightarrow$ *"Kneeling on/off"*
    - Example: `kw_batterietrennschalter` $\rightarrow$ *"Electricity On/Off"*
  - Rendered dynamically as a tooltip next to the cursor when enabled via `settings.cfg` (`tooltips=true`).

---

## 3. Rendering, Shader & GPU Debugging

`openOMSI`'s custom PBR and forward rendering engine (`omsi-render`) includes diagnostic hooks, GPU timestamp timers, and shader visualization channels.

### 3.1 PBR Shader Visualization Modes (`OMSI_DEBUG_ENHANCED`)
Setting the environment variable `OMSI_DEBUG_ENHANCED=<1..17>` alters the fragment output in [`crates/omsi-render/src/enhanced.wgsl`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-render/src/enhanced.wgsl:710) to visualize individual lighting and surface passes:

| Mode | Visualized Channel | Diagnostic Utility |
| :---: | :--- | :--- |
| **`1`** | **Sun Shadow Factor** | Visualizes shadow cascade splits, coverage, and shadow map depth bias acne/peter-panning. |
| **`2`** | **Screen-Space Ambient Occlusion (SSAO)** | Isolates the raw SSAO occlusion buffer before blur and ambient multiplication. |
| **`3`** | **World-Space Normals** | Displays surface normal vectors remapped to RGB (`0.5 * n + 0.5`). Verifies tangent/normal mapping and mesh smoothing. |
| **`4`** | **Atmospheric Transmittance** | Visualizes air absorption and transmittance curve over distance. |
| **`5`** | **Ambient Irradiance** | Displays hemisphere ambient lighting without direct directional light. |
| **`6`** | **Environment Reflections** | Displays specular reflection radiance from skybox and environment probes. |
| **`7`** | **Albedo / Base Color** | Displays raw diffuse texture/material color without any lighting or shading. |
| **`8`** | **Direct Sun Irradiance** | Isolates direct solar illumination contribution. |
| **`9`** | **In-Scattered Radiance** | Displays atmospheric haze and in-scattered sunlight. |
| **`10`** | **Alpha Mode / Glass / Envmap Factor** | Visualizes material classification (opaque, cutout alpha, glass, reflective). |
| **`11`** | **Linear Distance / Depth** | Visualizes camera-to-fragment distance normalized over 0–500m (grayscale depth). |
| **`12`** | **Alpha & Terrain Classification** | Color-codes alpha rendering path and terrain mesh flags. |
| **`13`** | **Cab & Specular Occlusion** | Visualizes interior vehicle cockpit shadowing and specular masking. |
| **`14`** | **Combined Direct + Ambient** | Shading before reflections, tone mapping, and emissive pass. |
| **`15`** | **Artificial Lights (Street Lamps & Headlights)** | Isolates point lights, street lanterns, and bus headlight cones. |
| **`16`** | **Emissive Surface Radiance** | Displays self-illuminated textures (backlit dashboards, illuminated buttons, destination displays). |
| **`17`** | **Roughness, F0 & Metalness** | Channels mapped as: **Red** = Roughness, **Green** = F0 specular reflectance, **Blue** = Metallic. |

### 3.2 Renderer Execution Diagnostics
- **`OMSI_DEBUG_DRAWS=1`:**
  - Emits real-time draw call statistics per frame: count of changed mesh instances, shadow pass draw calls & batches, and main pass split between opaque and blended geometry.
- **`OMSI_DEBUG_CULL=1`:**
  - Reports instances excluded by frustum or size culling along with the rejection cause (projected screen footprint vs `min_obj_size` or distance vs `max_obj_dist`).
- **`OMSI_DEBUG_FLICKER=1`:**
  - Monitors mesh instances that alternate between culled and drawn state on adjacent frames, pinpointing edge-case bounding box calculation bugs.
- **`OMSI_GPU_TIMERS=1`:**
  - Activates `wgpu::QuerySet` timestamp queries on supported hardware (Vulkan / Metal / DX12), printing microsecond-precise timings for:
    - Shadow map rendering pass
    - SSAO generation & bilateral blur passes
    - Opaque PBR main pass
    - Transparent / glass blended pass
    - Tonemapping and FXAA post-processing pass
- **`OMSI_PROFILE=1`:**
  - Prints per-frame CPU stage timings (world update, traffic tick, vehicle physics, audio mix, render dispatch).

---

## 4. Texture, Font & Asset Inspection Tools

### 4.1 Dynamic Script & Text Texture Dumping
OMSI vehicles dynamically render digital destination displays, ticket printer screens, and IBIS dot-matrix fonts directly into in-memory textures:
- **`OMSI_DUMP_SCRIPTTEX=<dir>`:**
  - Exports every generated `[scripttexture]` (IBIS LED matrices, ticket printer paper, LCD rollband matrices) to PNG files on disk on each update.
  - Implemented in [`crates/omsi-app/src/input_script.rs`](/mnt/Random/users/user/Documents/GitHub/openOMSI/crates/omsi-app/src/input_script.rs:2140) via `dump_display_textures`.
- **`OMSI_DUMP_SCENERY_TEXT=<dir>`:**
  - Dumps dynamic `[texttexture]` surfaces generated on scenery objects (e.g., bus stop schedule boards, digital platform clocks, highway variable message signs) to PNGs.
- **Interactive Console Command `dumptex <dir>`:**
  - Executable in `OMSI_INPUT` automation scripts to capture display textures at specific simulation timestamps.

### 4.2 Texture Compression & Memory Profiler
- **`OMSI_DEBUG_TEXTURES=1`:**
  - Logs texture compression statistics during map and vehicle loading.
  - Measures Peak Signal-to-Noise Ratio (PSNR) when compressing 32-bit RGBA source textures to GPU formats (BC1/BC3/BC7 or ASTC).
  - Summarizes GPU VRAM consumption grouped by texture dimensions and format.

### 4.3 Screenshot Capture
- **Keybindings:** `F12` or `Ctrl+Alt+P` captures the active frame to `Screenshots/omsi_<timestamp>.png`.
- **Scripted Capture:** `shot <filename.png>` in `OMSI_INPUT` or headless test runs.

---

## 5. Entity & Mesh Isolation Tools

When diagnosing rendering artifacts or broken geometry, individual assets can be isolated without altering configuration files:

- **`OMSI_ONLY_MESH=<substr>`:** Filters vehicle rendering to include *only* meshes whose `.o3d` filename contains the specified substring (e.g. `OMSI_ONLY_MESH=tacho` to isolate speedometer dials).
- **`OMSI_HIDE_MESH=<substr>`:** Selectively hides matching vehicle meshes.
- **`OMSI_DEBUG_MESHES=1`:** Dumps vertex counts, index buffers, bounding spheres, and bone hierarchy transformations during vehicle initialization.
- **`OMSI_ONLY_OBJECT=<substr>`:** Restricts tile loading to scenery objects matching the pattern.
- **`OMSI_SKIP_OBJECT=<substr>`:** Excludes scenery objects matching the pattern from tile generation.
- **`OMSI_DEBUG_OBJECTS=1`:** Emits detailed object placement coordinates and configuration parsing messages.

---

## 6. Simulation & Script Variable Tracing

OMSI scripts (`.osc`) run an interpreted stack-based virtual machine simulating vehicle electricals, pneumatics, engine physics, and sounds.

- **`OMSI_WATCH_VARS=var1,var2,...`:**
  - Monitors specific variables and emits a log message immediately when their numeric value changes across frames:
    ```
    [WATCH] engine_temperature changed: 82.4 -> 83.1
    ```
- **`OMSI_TRACE_VARS=var1,$strvar2,...`:**
  - Dumps the current values of specified float variables and string variables (prefixed with `$`) periodically (every 0.5s).
- **`OMSI_DEBUG_VARS=1`:**
  - Verbose logging of all variable declarations, initial values, and macro registration during vehicle loading.
- **`OMSI_SCRIPT_OPS=1`:**
  - Opcode tracing and instruction counters for the OMSI VM.
- **`OMSI_INPUT` Replay & Automation Engine:**
  - Supports scripted test sequences (`OMSI_INPUT="t=2.0 move 400,300; t=2.5 click; t=3.0 set throttle=1.0; t=4.0 log pose; t=5.0 shot output.png"`).
- **In-Game Diagnostic Hotkeys:**
  - **`F11` / `Ctrl+F11`:** Logs camera world position $(X, Y, Z)$, tile coordinates `tile_X_Y`, yaw, pitch, and prints a ready-to-use `--cam X,Y,Z,Yaw,Pitch` command-line argument to replicate the exact camera view.
  - **`Ctrl+Y`:** Toggles OMSI's Information Bar (displaying clock time, velocity in km/h, fuel tank percentage, and current schedule terminus/delay).

---

## 7. Scope Creep: Proposed Unified Visual Debug Inspector

### 7.1 Current Architectural Gap
While `openOMSI` features extensive debugging hooks, they are currently split across:
1. Environment variables (`OMSI_*`) requiring process restarts.
2. Console log outputs (`stdout`/`stderr`) requiring external log viewing.
3. Separate picking domains (scenery editor only selects `.sco` scenery objects; cockpit raycaster only selects `.bus` meshes).

There is currently **no unified in-game visual inspector** where clicking an arbitrary entity on screen reveals its live transform, textures, materials, and internal script state.

### 7.2 Architecture & Implementation Plan for an Interactive In-Game Inspector

```
+-----------------------------------------------------------------------------------+
| openOMSI Unified In-Game Inspector                                                |
+-----------------------------------------------------------------------------------+
|  [Select Tool (Ctrl+`)]                                                           |
|                                                                                   |
|  Click on-screen:                                                                 |
|   -> Raycast Scene (Scenery Objects, Vehicles, Road Splines, Humans)             |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  | Selected: "MAN_NL202/model/tacho.o3d"                                       |  |
|  | Parent: Vehicle #0 (MAN Lion's City A21)                                    |  |
|  +-----------------------------------------------------------------------------+  |
|  | [Transform]                                                                 |  |
|  |   World: [1420.50, -320.12, 45.20]  | Local: [0.35, 1.20, 0.85]             |  |
|  |   Rot:   Yaw 182.4 deg, Pitch -1.2 deg | Scale: [1.0, 1.0, 1.0]               |  |
|  +-----------------------------------------------------------------------------+  |
|  | [Rendering & Material]                                                       |  |
|  |   Mesh: 1,428 Vertices, 2,100 Triangles | Bounding Radius: 0.18m            |  |
|  |   Shader: PBR Enhanced (Draw Call #42, Opaque Pass)                         |  |
|  |   Texture 0: "cockpit_gauges.png" (1024x1024, BC7) [View Mips] [Dump PNG]   |  |
|  |   Texture 1: ScriptTex #2 (IBIS LCD 256x64) [Live Preview]                  |  |
|  |   Roughness: 0.45 | Metalness: 0.05 | Emissive Factor: 0.00                 |  |
|  +-----------------------------------------------------------------------------+  |
|  | [Live Simulation State]                                                     |  |
|  |   Animation Var: `cockpit_tacho` = 48.25 km/h                               |  |
|  |   Associated Triggers: `cockpit_tachobeleuchtung` (Value: 1.0)               |  |
|  |   [Add to Watch Table] [Force Value...]                                     |  |
|  +-----------------------------------------------------------------------------+  |
|  | [3D Visualizer Gizmos]                                                      |  |
|  |   [x] Show Bounding Box  [ ] Show Wireframe  [x] Show Local Axes Gizmo      |  |
|  +-----------------------------------------------------------------------------+  |
+-----------------------------------------------------------------------------------+
```

#### Step-by-Step Implementation Blueprint:
1. **Unified Entity Raycasting:**
   - Unify `Editor::pick` and `Player::pick_in` into a universal `Scene::raycast(&self, ray: Ray)` query in `crates/omsi-app`.
   - The query tests in order:
     1. Player vehicle and trailers (using existing `ray_mesh`).
     2. AI vehicles and parked traffic.
     3. Pedestrians / humans (bounding cylinder).
     4. Scenery objects (`SceneryObject` bounding spheres and meshes).
     5. Terrain / road splines.
2. **Visual Debug Overlays (Render Pipelines):**
   - Add a lightweight debug line/wireframe pass to `omsi-render` using `wgpu::PrimitiveTopology::LineList`.
   - Draw an Oriented Bounding Box (OBB) and coordinate axes (Red: $+X$, Green: $+Y$, Blue: $+Z$) around the currently selected entity.
3. **Interactive Debug UI Layer:**
   - Integrate `egui-wgpu` (or expand the custom `omsi-ui` text/canvas layer).
   - Render a collapsible side panel when an object is selected.
   - Panel tabs:
     - **Properties:** File path, mesh index, world/local transforms.
     - **Material & Textures:** Preview textures in UI, display formats, resolutions, mip levels, and provide a single-click "Dump to PNG" button.
     - **Script Variables:** Filterable list of all `.osc` variables tied to the selected vehicle or animated scenery object, supporting live editing.
     - **Render Passes:** Toggle wireframe mode, force-hide mesh, or isolate draw calls in real time without restarting the engine.

---

## 8. Summary Checklist of Available Features

| Category | Feature / Tool | Access Method |
| :--- | :--- | :--- |
| **Object Selection** | Scenery Object Editor | `Ctrl+Shift+E` or Game Menu $\rightarrow$ Object editor |
| **Object Selection** | Cockpit Switch / Part Raycast | Mouse hover in cab (`tooltips=true` in settings) |
| **Object Selection** | Map Teleport / Snap Picker | `Ctrl+Click` on Navigator City Map |
| **Shader Diagnostics** | 17 PBR Shader Channels | `OMSI_DEBUG_ENHANCED=1..17` |
| **Render Diagnostics** | Draw Call & Batch Counters | `OMSI_DEBUG_DRAWS=1` |
| **Render Diagnostics** | Frustum / Distance Culling Logs | `OMSI_DEBUG_CULL=1` |
| **Render Diagnostics** | Oscillation / Flicker Detection | `OMSI_DEBUG_FLICKER=1` |
| **Render Diagnostics** | GPU Timestamp Passes Timing | `OMSI_GPU_TIMERS=1` |
| **Texture Inspection** | Script/Display Texture Dumper | `OMSI_DUMP_SCRIPTTEX=<dir>` or `dumptex <dir>` |
| **Texture Inspection** | Dynamic Scenery Text Dumper | `OMSI_DUMP_SCENERY_TEXT=<dir>` |
| **Texture Inspection** | Texture Memory & PSNR Metrics | `OMSI_DEBUG_TEXTURES=1` |
| **Asset Isolation** | Vehicle Mesh Filter | `OMSI_ONLY_MESH=<str>`, `OMSI_HIDE_MESH=<str>` |
| **Asset Isolation** | Scenery Object Filter | `OMSI_ONLY_OBJECT=<str>`, `OMSI_SKIP_OBJECT=<str>` |
| **Simulation Tracing** | Variable Delta Watcher | `OMSI_WATCH_VARS=a,b` |
| **Simulation Tracing** | Periodic Variable Logger | `OMSI_TRACE_VARS=a,b,$c` |
| **Simulation Tracing** | Camera Position & Pose Reporter | Press `F11` (camera, tile, `--cam` parameter) |
| **HUD / Info Display** | OMSI Information Bar | Press `Ctrl+Y` (speed, fuel, trip, timetable) |
| **Input Automation** | Headless / Scripted Replay | `OMSI_INPUT="t=1.0 ..."` |
