# Inspector Scope Creep Implementation Plan (A, B, C, E, G, I)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved scope creep features for the openOMSI Unified Visual Debug Inspector: Materials/Textures (A/A+), Script VM (B/B+), Render Pipeline (C/C+), Human Kinematics (E/E+), Penetration Stacks (G/G+), and Automation/Export (I/I+).

**Baseline Status:**
- **Current Commit:** `000f298` (docs: expand inspector scope)
- **MVP Status:** Completed at `e8f101c` - 10 foundational tasks merged
- **Existing Implementation:** 2,009 lines in `crates/omsi-app/src/inspector.rs`
- **Specification:** `/docs/superpowers/specs/2026-09-30-inspector-extras-scope-creep-spec.md` (2,907 lines)

**Architecture Pillars (Inviolable):**
1. **Zero-Stall GPU Pipeline** - All GPU reads use async staging pools
2. **Snapshot Isolation** - No locks held across frames
3. **Fail-Closed Integrity** - All mutations are sandboxed with 1-click rollback
4. **Frame Budget** - <1.0ms total overhead when active, 0.0ms when inactive

---

## Phase 1: Raycast & Spatial Corridor (G, E Core)

**Duration:** 3.5-5.0 weeks  
**Dependencies:** MVP baseline (completed)  
**Target:** Enhanced spatial picking with multi-hit penetration stacks and human raycasting

### Task 1.1: Dynamic BVH Spatial Acceleration
**Priority:** HIGH  
**Complexity:** HIGH  
**Files:**
- New: `crates/omsi-geometry/src/bvh.rs`
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/app_events.rs`

**Subtasks:**
- [ ] Design BVH node structure with SIMD-friendly AABB layout
- [ ] Implement incremental BVH build using Surface Area Heuristic (SAH)
- [ ] Add adaptive rebuild trigger based on quality metrics (depth > 12, occupancy < 40%)
- [ ] Implement ray-AABB intersection with early exit frustum culling
- [ ] Cap single-frame raycast to 256 candidates maximum
- [ ] Add fallback: disable pedestrian raycasting when entity count > 1000
- [ ] Profile raycast performance: target <0.4ms on 100k triangle scenes
- [ ] Write unit tests for BVH construction, traversal, and quality metrics
- [ ] Add cargo benchmark comparing BVH vs linear scan performance

**Exit Criteria:**
- BVH raycast completes in <0.4ms for representative busy scenes (Spandau with 30 AI buses, 200 pedestrians)
- Quality metrics prevent degeneration to O(N) linear scan
- Zero memory leaks or unbounded growth over 10-minute test runs

---

### Task 1.2: Multi-Hit Penetration Stack & Ray Corridor (G1, G2)
**Priority:** HIGH  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`
- New: Tests in `crates/omsi-app/tests/inspector_penetration_tests.rs`

**Subtasks:**
- [ ] Extend raycast to return all hits along ray, not just closest
- [ ] Sort hits by distance from camera in ascending order
- [ ] Store penetration stack in `InspectorState` as `Vec<PenetrationHit>`
- [ ] Add UI cycling: Tab key cycles forward, Shift+Tab cycles backward
- [ ] Display "Multi-Hit Corridor" panel showing first 10 hits with distances
- [ ] Highlight currently selected hit in stack with visual marker
- [ ] Add "Jump to Hit N" click interaction in penetration panel
- [ ] Test nested geometry: bus interior with seats, passengers, dashboard behind windshield
- [ ] Verify hit ordering matches physical depth along ray
- [ ] Add regression test: raycast through articulated bus joint (3+ overlapping volumes)

**Exit Criteria:**
- Tab cycling works smoothly with up to 20 hits in corridor
- UI panel shows distance-sorted hits with correct mesh names
- Selection marker updates when cycling through stack
- Frame overhead <0.1ms for penetration stack management

---

### Task 1.3: Pedestrian / Human Raycasting & Key Model (E Core)
**Priority:** HIGH  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-app/src/inspector.rs` (add `HumanKey`)
- Modify: `crates/omsi-app/src/humans.rs` (if exists, else `crates/omsi-sim/src/human.rs`)
- New: Tests in `crates/omsi-app/tests/inspector_human_tests.rs`

**Subtasks:**
- [ ] Define `HumanKey { id: usize, generation: u64, is_driver: bool }`
- [ ] Implement broadphase: bounding cylinder intersection (r≈0.35m, h≈1.85m)
- [ ] Implement narrowphase: triangle-level raycast against animated pose mesh
- [ ] Add human candidate collection in inspector raycast path
- [ ] Store selected human in `InspectorSelection::Human(HumanKey, mesh_id)`
- [ ] Build snapshot data: ID, model path (.hum), world pos, heading, velocity
- [ ] Add behavior state display: Wandering, WalkingToStop, BoardingBus, Seated, etc.
- [ ] Handle generation invalidation when pedestrian despawns or respawns
- [ ] Test: select passenger boarding bus, verify selection persists until alighting
- [ ] Test: select driver in cockpit, verify distinct from player vehicle selection
- [ ] Profile: ensure human raycasting overhead <0.15ms with 200 pedestrians visible

**Exit Criteria:**
- Humans are selectable via click with stable identity across frames
- Snapshot shows behavior state and model path
- Selection invalidates correctly when human despawns
- BVH fallback prevents performance degradation with crowd density >1000

---

## Phase 2: Simulation Observability & Script VM (B, E+)

**Duration:** 4.0-6.0 weeks  
**Dependencies:** Phase 1 (E Core for human selection)  
**Target:** Script variable inspection, live overrides, time-travel debugging, human kinematics

### Task 2.1: Human Kinematics, Poses & Pathfinding Vectors (E+)
**Priority:** MEDIUM  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-sim/src/human.rs` (or equivalent)
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Extract skeletal bone hierarchy from human model (.hum format)
- [ ] Visualize bone links as debug line segments in world space
- [ ] Display animation phase [0..1] and current animation clip name
- [ ] Show destination stop node and A* pathfinding waypoint vector
- [ ] Render pathfinding breadcrumb trail (last 10 waypoints)
- [ ] Display queued behavior: NextStop, SeatTarget, ExitDoor
- [ ] Add "Freeze Animation" toggle to pause individual human for inspection
- [ ] Test: inspect passenger walking aisle, verify bone transforms update smoothly
- [ ] Test: boarding passenger shows door target and seat assignment
- [ ] Profile: kinematics visualization overhead <0.2ms per selected human

**Exit Criteria:**
- Bone hierarchy visualizes correctly for all human model types
- Animation phase updates in real-time, frozen when toggled
- Pathfinding vectors show clear destination intent
- Zero jitter or lock contention during multi-threaded simulation

---

### Task 2.2: Read-Only Script Variable Inspector & Search (B1)
**Priority:** HIGH  
**Complexity:** MEDIUM  
**Files:**
- New: `crates/omsi-script/src/inspector.rs` (snapshot API)
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Add `omsi-script` snapshot API: `fn enumerate_vars(entity_id) -> Vec<VarSnapshot>`
- [ ] Capture all `L.var` and `S.var` from selected vehicle/scenery scripts
- [ ] Categorize by origin script file (engine.osc, door.osc, cockpit.osc, etc.)
- [ ] Build UI panel with scrollable variable list: name, value, type, file
- [ ] Add real-time search/filter input with debounce (200ms)
- [ ] Support wildcard patterns: "door_*", "*_pressure", "engine.*"
- [ ] Display value precision: floats to 4 decimal places, strings truncated at 64 chars
- [ ] Test: select player bus with 500+ variables, verify smooth scrolling
- [ ] Test: filter performance with 1000+ variables, ensure <50ms filter update
- [ ] Profile: snapshot construction overhead <0.3ms per frame

**Exit Criteria:**
- All vehicle/scenery script variables visible in inspector
- Search/filter updates instantly without frame drops
- Variable list scrolls smoothly with 1000+ entries
- Snapshot construction never blocks simulation thread

---

### Task 2.3: Sandboxed Variable Overrides & Clamping (B2)
**Priority:** HIGH  
**Complexity:** HIGH (Safety-Critical)  
**Files:**
- Modify: `crates/omsi-script/src/inspector.rs`
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Add override storage: `HashMap<(EntityKey, VarName), OverrideValue>`
- [ ] Implement clamping validation: throttle [0.0..1.0], RPM [0.0..3000.0], etc.
- [ ] Add "Edit" button next to each variable in inspector panel
- [ ] Show inline text field with real-time validation feedback (red border on invalid)
- [ ] Apply override to simulation on Enter key, cancel on Escape
- [ ] Add prominent "Reset All Overrides" button with confirmation dialog
- [ ] Implement fail-safe: overrides cleared on entity despawn, map change, inspector exit
- [ ] Add override indicator badge (yellow star) next to modified variables
- [ ] Test: override door_state, verify door animation responds immediately
- [ ] Test: invalid input (throttle = 5.0), verify clamps to 1.0 with user warning
- [ ] Test: reset button, verify simulation state reverts to physics-driven values
- [ ] Add transaction log: record all overrides with timestamp for debugging

**Exit Criteria:**
- Variable overrides work smoothly without simulation instability
- Clamping prevents physically impossible values
- Reset button works 100% reliably (critical safety requirement)
- Override state never persists across sessions or entity replacements

---

### Task 2.4: Sparklines, RPN Stack Tracer & Watch Tables (B3, B+)
**Priority:** MEDIUM  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`
- New: `crates/omsi-app/src/inspector/watch_table.rs`

**Subtasks:**
- [ ] Implement watch table: users pin up to 16 variables
- [ ] Add "+ Watch Variable" button, remove with "X" button
- [ ] Store 120 historical samples per watched variable (2 seconds at 60Hz)
- [ ] Render sparkline graphs inline: 100px wide, 20px tall
- [ ] Color-code value changes: green flash on increment, red on decrement
- [ ] Display statistics: Min, Max, Mean, Δ/second
- [ ] Support computed expressions: `(L.speed * 3.6) + L.temp / 2.0`
- [ ] Add RPN stack viewer: show top 8 float stack elements
- [ ] Display active macro/trigger name during execution
- [ ] Test: watch engine RPM during acceleration, verify smooth sparkline
- [ ] Test: watch expression with division by zero, verify safe error handling
- [ ] Profile: sparkline rendering overhead <0.1ms for 16 watched variables

**Exit Criteria:**
- Watch table updates smoothly with no visual stutter
- Sparklines show clear oscillations and trends
- Computed expressions evaluate correctly
- RPN stack viewer updates in real-time during script execution

---

### Task 2.5: Simulation Time-Travel Scrubbing Buffer (B+)
**Priority:** LOW (Advanced Feature)  
**Complexity:** VERY HIGH  
**Files:**
- New: `crates/omsi-app/src/inspector/time_travel.rs`
- Modify: `crates/omsi-sim/src/lib.rs` (add snapshot hooks)

**Subtasks:**
- [ ] Design ring buffer: 600 frames × variable snapshot size (~2MB total)
- [ ] Capture full vehicle state every frame when inspector active
- [ ] Add pause/scrub toolbar UI: play/pause button, time slider
- [ ] Implement backwards scrubbing: drag slider to historical frame
- [ ] Restore variable states from ring buffer without full simulation rewind
- [ ] Display "Time Travel Mode" banner with current frame offset
- [ ] Add "Resume Live" button to exit time-travel and return to real-time
- [ ] Test: cause engine stall, scrub backwards 3 seconds, inspect throttle/RPM
- [ ] Test: ring buffer wraparound after 10 seconds, verify no memory leak
- [ ] Profile: memory overhead ≤32MB, capture overhead <0.5ms per frame

**Exit Criteria:**
- Time travel works reliably for 10-second history window
- Scrubbing is smooth with no frame stutters
- Memory usage bounded and predictable
- Feature can be disabled to reclaim memory if not needed

---

## Phase 3: Material, Textures & Render Pipeline (A, C)

**Duration:** 5.0-7.0 weeks  
**Dependencies:** None (parallel with Phase 2)  
**Target:** Texture inspection, PBR sandbox, render pass isolation, collision overlays

### Task 3.1: Material Metadata & Async Texture Previews (A1, A2)
**Priority:** HIGH  
**Complexity:** MEDIUM  
**Files:**
- New: `crates/omsi-render/src/inspector.rs` (texture readback API)
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Extract material properties from selected mesh: shader variant, blend mode, double-sided flag
- [ ] Display texture metadata: format (BC7, BC3, RGBA8), resolution, mipmap count, VRAM size
- [ ] Implement async texture readback using wgpu staging buffer pool (4-16 slots, 8MB each)
- [ ] Queue readback requests without blocking render thread
- [ ] Generate thumbnail (256×256) from diffuse/albedo texture
- [ ] Display thumbnail in inspector panel with hover-to-enlarge interaction
- [ ] Add "Export Texture to PNG" button → save to `Screenshots/inspector_dump_<name>.png`
- [ ] Handle staging buffer overflow: downsample or cancel with user error message
- [ ] Test: inspect bus exterior panel, verify diffuse texture thumbnail displays
- [ ] Test: queue 10 texture exports simultaneously, verify no GPU stalls
- [ ] Profile: readback overhead <0.1ms per frame, async completion within 3 frames

**Exit Criteria:**
- Material properties display correctly for all mesh types
- Texture thumbnails load without blocking main thread
- PNG export works for textures up to 4096×4096
- Staging buffer pool prevents GPU pipeline stalls

---

### Task 3.2: Dynamic ScriptTexture / TextTexture Live Zoomer (A+)
**Priority:** MEDIUM  
**Complexity:** HIGH  
**Files:**
- Modify: `crates/omsi-render/src/inspector.rs`
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Identify dynamic textures: scripttexture, texttexture in vehicle model
- [ ] Hook into texture update pipeline to capture live frames
- [ ] Display live preview updating at simulation rate (30-60 Hz)
- [ ] Add pixel-grid zoom: 1×, 2×, 4×, 8× magnification levels
- [ ] Show pixel coordinate and hex color under mouse cursor
- [ ] Display backing font configuration file and character dimensions
- [ ] Show source variable: e.g. "$IBIS_terminus_name"
- [ ] Test: inspect destination blind, verify text updates when route changes
- [ ] Test: inspect digital clock, verify ticking updates in real-time
- [ ] Profile: live preview overhead <0.2ms per frame

**Exit Criteria:**
- Dynamic textures update smoothly in inspector preview
- Pixel zoom works without artifacts
- Font metrics display correctly for text textures
- Source variable bindings are traceable

---

### Task 3.3: Texture Mipmap Heatmap & VRAM Profiler (A3, A+)
**Priority:** LOW  
**Complexity:** MEDIUM  
**Files:**
- New: `crates/omsi-render/src/debug_shaders/mipmap_heatmap.wgsl`
- Modify: `crates/omsi-render/src/lib.rs`
- Modify: `crates/omsi-app/src/inspector.rs`

**Subtasks:**
- [ ] Implement diagnostic shader pass: color-code texel density vs screen pixels
- [ ] Color scheme: Blue (undersampled), Green (optimal), Red (oversampled)
- [ ] Add mipmap slider: force specific mip level on selected mesh
- [ ] Collect VRAM statistics: total allocated, per-texture size, streaming budget
- [ ] Display aggregate VRAM usage in inspector profiler tab
- [ ] Add "Dump VRAM Report" → JSON file with all textures and sizes
- [ ] Test: enable heatmap on Spandau map, verify color-coding makes sense
- [ ] Test: force mip level 3, verify visual aliasing is obvious
- [ ] Profile: heatmap shader overhead <0.3ms per frame when enabled

**Exit Criteria:**
- Heatmap clearly identifies undersampled/oversampled textures
- Mipmap slider works without crashing or corrupting textures
- VRAM profiler shows accurate memory usage
- Feature can be toggled off with zero overhead when disabled

---

### Task 3.4: Render Pass Frame Graph & Mesh Isolation (C1, C2)
**Priority:** MEDIUM  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-render/src/lib.rs`
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Add render pass toggles: Shadow Cascades, SSAO, Opaque, Transparent, Tonemapping
- [ ] Implement global wireframe overlay: `wgpu::PolygonMode::Line`
- [ ] Add "Isolate Selected Mesh" action: hide all other meshes, neutral background
- [ ] Add "Hide Selected Mesh" action: temporarily suppress rendering
- [ ] Ensure overlay state is transient, cleared on selection change or inspector exit
- [ ] Test: isolate speedometer needle, verify clean isolation against clear color
- [ ] Test: toggle shadow pass off, verify shadows disappear immediately
- [ ] Profile: isolation/wireframe overhead <0.2ms per frame

**Exit Criteria:**
- Render pass toggles work without breaking pipeline
- Mesh isolation cleanly hides/shows geometry
- Wireframe mode is useful for debugging topology
- All changes revert cleanly when inspector exits

---

### Task 3.5: Ghost/X-Ray Mode & Collision Hull Overlays (C+)
**Priority:** LOW  
**Complexity:** HIGH  
**Files:**
- New: `crates/omsi-render/src/debug_shaders/xray.wgsl`
- Modify: `crates/omsi-render/src/lib.rs`
- Modify: `crates/omsi-app/src/inspector.rs`

**Subtasks:**
- [ ] Implement X-Ray shader: selected mesh full opacity, surroundings 15% dimmed
- [ ] Add depth override: selected mesh renders last, always on top
- [ ] Extract collision hull geometry from physics system
- [ ] Visualize collision boxes, cylinders, capsules as wireframe overlays
- [ ] Color-code collision types: static (green), dynamic (yellow), trigger (cyan)
- [ ] Test: inspect bus bumper near curb, verify collision box alignment with visual mesh
- [ ] Test: X-ray mode on dashboard speedo, verify interior clearly visible
- [ ] Profile: X-ray overhead <0.4ms per frame

**Exit Criteria:**
- X-ray mode makes interior components clearly visible
- Collision hulls overlay accurately on visual geometry
- Color-coding helps distinguish collision types
- Performance remains within frame budget

---

## Phase 4: World Infrastructure, Audio & Environment (D, H)

**Duration:** 3.0-4.5 weeks  
**Dependencies:** Phase 1 (BVH for spatial queries)  
**Target:** Spline inspection, traffic paths, audio cones, weather probes

*Note: D and H are NOT in approved scope (only A, B, C, E, G, I approved), but included in spec for completeness. Mark as DEFERRED until explicit approval.*

### Task 4.1-4.4: DEFERRED
**Status:** Awaiting explicit approval for D (Splines/Traffic) and H (Audio/Weather) subsystems.

---

## Phase 5: Automation, Persistence, Export & External Telemetry (I, F)

**Duration:** 4.0-6.0 weeks  
**Dependencies:** Phases 1-3 (all subsystems for export)  
**Target:** Persistent layouts, glTF export, CI validation, editor bridge

### Task 5.1: Persistent Layouts & User Preferences (I1)
**Priority:** LOW  
**Complexity:** LOW  
**Files:**
- New: `crates/omsi-app/src/inspector/persistence.rs`
- New: `~/.config/openomsi/inspector_prefs.toml` (or platform-appropriate path)

**Subtasks:**
- [ ] Define preference schema: panel position, watched variables, tab selection
- [ ] Serialize preferences to TOML format on inspector exit
- [ ] Deserialize preferences on inspector open, apply layout
- [ ] Add "Reset Layout" button to restore defaults
- [ ] Test: close and reopen inspector, verify panel position persists
- [ ] Test: watched variables persist across sessions
- [ ] Handle migration: gracefully handle old preference file formats

**Exit Criteria:**
- Layout persists reliably across sessions
- Preference file is human-readable and editable
- Migration handles version upgrades without data loss

---

### Task 5.2: Automated Script Testing via OMSI_INPUT (I2)
**Priority:** MEDIUM  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-app/src/inspector.rs`
- New: Test scripts in `tests/inspector_automation/`

**Subtasks:**
- [ ] Expose inspector actions via `OMSI_INPUT` automation framework
- [ ] Add commands: `inspector.open`, `inspector.select(entity_id)`, `inspector.get_var(name)`
- [ ] Record inspector interactions as scriptable commands
- [ ] Test: automated script selects bus, checks door_state variable, asserts expected value
- [ ] Test: CI pipeline runs headless validation with `--inspect-check` flag
- [ ] Profile: automation overhead ≤scripted input framework baseline

**Exit Criteria:**
- Inspector actions are fully scriptable
- CI validation catches broken assets automatically
- Automation tests run reliably in headless mode

---

### Task 5.3: Single-Click glTF 2.0 Export (.glb) (I3, I+)
**Priority:** MEDIUM  
**Complexity:** HIGH  
**Files:**
- New: `crates/omsi-export/src/gltf_exporter.rs`
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/ui.rs`

**Subtasks:**
- [ ] Extract mesh geometry: vertices, normals, UVs, indices
- [ ] Extract material properties: PBR roughness, metallic, textures
- [ ] Extract transform hierarchy for articulated vehicles (lead + trailers)
- [ ] Encode as glTF 2.0 binary (.glb) format
- [ ] Embed textures or reference external files (user choice)
- [ ] Add "Export to GLB" button in inspector panel
- [ ] Save to `Exports/inspector_<entity>_<timestamp>.glb`
- [ ] Test: export bus, open in Blender, verify transforms and textures correct
- [ ] Test: export articulated bus, verify joint hierarchy preserved
- [ ] Profile: export time <5 seconds for complex vehicle

**Exit Criteria:**
- Exported glTF files open correctly in Blender, Unity, Godot
- Transform hierarchy is preserved for articulated vehicles
- Textures are embedded or referenced correctly
- Export process does not block main thread

---

### Task 5.4: Remote Telemetry WebSocket Server (I+)
**Priority:** LOW  
**Complexity:** HIGH  
**Files:**
- New: `crates/omsi-telemetry/src/websocket_server.rs`
- Modify: `crates/omsi-app/src/inspector.rs`

**Subtasks:**
- [ ] Implement WebSocket server on `ws://localhost:9090/inspector`
- [ ] Stream inspector snapshots as JSON: selection, variables, telemetry
- [ ] Add authentication token for security
- [ ] Implement rate limiting: max 60 updates/second
- [ ] Add web client example: HTML/JS dashboard displaying live telemetry
- [ ] Test: connect from browser, verify live variable updates stream
- [ ] Test: disconnect handling, reconnect robustness
- [ ] Profile: telemetry overhead <0.2ms per frame

**Exit Criteria:**
- WebSocket server is stable and secure
- External tools can consume inspector data in real-time
- Performance overhead is negligible
- Feature can be disabled for security-sensitive environments

---

### Task 5.5: Non-Destructive Editor Handover Bridge (F, F+)
**Priority:** LOW  
**Complexity:** MEDIUM  
**Files:**
- Modify: `crates/omsi-app/src/inspector.rs`
- Modify: `crates/omsi-app/src/editor.rs` (if exists)

**Subtasks:**
- [ ] Add "Promote to Editor" button for selected scenery objects
- [ ] Transfer selection to object editor mode (`Ctrl+Shift+E`)
- [ ] Preserve selection identity and transform state during handover
- [ ] Test: select building, promote to editor, verify object is selected in editor
- [ ] Test: round-trip: editor → inspector → editor, verify no data loss
- [ ] Handle edge cases: objects not editable, editor mode unavailable

**Exit Criteria:**
- Seamless transition between inspector and editor modes
- Selection state preserved across mode switches
- Proper error handling for non-editable objects

---

## Cross-Cutting Concerns

### Testing Strategy
- **Unit Tests:** Each subsystem has dedicated test file in `tests/inspector_*_tests.rs`
- **Integration Tests:** End-to-end scenarios in `tests/integration/inspector_scenarios.rs`
- **Performance Benchmarks:** `benches/inspector_bench.rs` using criterion.rs
- **Regression Suite:** Automated via CI on every PR touching inspector code

### Documentation Requirements
- **User Guide:** Update `docs/USER_GUIDE.md` with new features
- **API Documentation:** Rustdoc comments for all public APIs
- **Architecture Document:** Update `docs/superpowers/specs/section_9_inspector.md`
- **Change Log:** Maintain `docs/INSPECTOR_CHANGELOG.md`

### Review Checkpoints
- **Pre-Implementation:** Risk matrix review, API design approval
- **Mid-Phase:** Code review after 50% completion
- **Pre-Merge:** Full integration test pass, performance benchmarks, documentation complete

---

## Implementation Tracking

**Status Legend:**
- `[ ]` Not started
- `[~]` In progress
- `[x]` Complete
- `[!]` Blocked

**Current Phase:** Not started (awaiting subagent delegation)

---

**Next Steps:**
1. Use subagent-driven-development skill to delegate Phase 1 tasks
2. Review and approve each task plan before implementation
3. Execute tasks sequentially with review checkpoints
4. Update this plan as implementation progresses
