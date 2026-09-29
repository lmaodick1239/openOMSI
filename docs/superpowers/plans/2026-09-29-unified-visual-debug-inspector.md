# Unified Visual Debug Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a separate in-game inspector mode that selects a precise vehicle mesh/part or loaded scenery object, shows its parent and read-only identity/transforms, and highlights it without changing existing driving or object-editor behavior outside inspector mode.

**Architecture:** Keep the feature in `omsi-app`, using a dedicated inspector module to own selection handles, candidate collection, invalidation, and displayed snapshot data. Reuse existing vehicle/scenery geometry and render/UI systems; keep simulation entities authoritative, represent render-only scenery with stable map/tile identity, and add a reference-counted tile pin for the selected scenery target. Inspector input takes precedence only while its mode is active; ordinary cockpit and editor paths remain intact otherwise.

**Tech Stack:** Rust workspace, `glam`, existing `omsi_geometry::ray_mesh`, `omsi_render` scene/overlay APIs, custom `omsi-app::ui`, `cargo test`, and existing scripted/offscreen test facilities.

## Global Constraints

- Keep the feature in the existing Rust workspace and use established dependencies; the MVP adds no UI or rendering dependency.
- Preserve cockpit control and object editor semantics when inspector mode is inactive.
- Inspector selection is read-only: no transforms, script values, material state, or render state may be changed through the MVP.
- Select the specific vehicle mesh/part and display its parent vehicle; selecting a scenery object identifies both the placed object instance and the selected mesh when available.
- Vehicle candidates include the player, AI traffic, remote vehicles, parked vehicles if represented by runtime vehicle meshes, and coupled/trailer parts. Scenery candidates include every loaded scenery object, editable and non-editable. Humans, terrain and splines are deferred.
- Do not pin vehicle simulation objects. Pin only the tile required by a selected scenery object, and release the pin on deselection, replacement selection, inspector exit, or world teardown.
- Avoid per-frame triangle raycasts over every entity. Use broadphase bounds, compute precise candidates on click or when needed for hover, and profile representative busy scenes.
- Treat LOD mesh-index changes, tile reloads, network entity replacement, and asynchronous stream/unload changes as selection invalidation events.
- Use the existing translation and physical-pixel scaling conventions in the custom UI.
- Do not modify unrelated user changes; the pre-existing untracked `DEBUG_INSPECTION_AUDIT.md` is source context only and remains untouched.

---

## Scope and Product Decisions

### MVP in scope

- A dedicated toggleable inspector mode. The candidate key from the audit (`Ctrl+``) is not final; pick an available, non-conflicting binding by inspecting current input mappings and expose it in the UI/help text.
- Click-to-select the closest valid mesh hit on the cursor ray.
- Vehicle selections identify a mesh and the owning vehicle, with trailer/coupled section identity where relevant.
- Scenery selections identify the placed instance and mesh. Include loaded editable and non-editable objects; do not limit this to `World::edit_objects` because that collection is specifically editor-oriented and incomplete for generic visual inspection.
- A compact, read-only side panel with kind, parent, mesh, source path, world/local transform, and bounds where real data exists.
- Selection marker and optional bounds/local-axis debug visualization.
- Explicit clear-selection and inspector-exit behavior.
- Selection continuity for loaded scenery through tile pinning, subject to the safety constraints below.
- Focused tests for hit ordering, identity stability, stale selections, input routing, pin lifecycle, and UI geometry.

### Deliberately deferred

- Human/pedestrian hit tests; terrain and spline picking.
- Texture/material preview, mip inspection, PNG export, dynamic texture capture.
- Script-variable watches, editing, trigger execution, or watch-table UX.
- Wireframe pipeline, render-pass controls, mesh hiding/isolation, forced material changes.
- Scenery transform manipulation or replacement of the existing object editor.
- Persistent inspector selection across game sessions.
- Multiplayer synchronization of inspector selection or server-authoritative inspection.
- Asset-browser integration, arbitrary file opening, and export/report generation.

### Selection and lifetime policy

Scenery tile pinning is the requested default, but must be implemented as a narrow, explicit lease owned by the inspector selection. The streamer remains the authority over load/unload, and every unload path (ordinary distance unload, explicit reload, world replacement, shutdown) must consult or invalidate that lease. A selected tile is not allowed to remain pinned indefinitely if it is being reloaded or its object record disappears: invalidate the selection, release its lease, show a brief unavailable state, then permit normal unload. Pin only one selected scenery tile at a time; selection replacement releases the previous tile before acquiring the new one. This avoids keeping an unbounded set resident and avoids stale GPU/runtime references.

For vehicles, retain a stable logical key, not a borrowed reference or collection index: player identity; AI/placed vehicle identity with a generation component; or remote player ID plus vehicle generation/definition identity. If the selected instance is replaced, invalidate it even if another instance now occupies the same vector slot. Mesh identity should use source model mesh definition identity/path plus a duplicate disambiguator, not only a transient LOD/render index.

---

## Repository File Map

The implementation should follow existing module boundaries. Exact APIs should be confirmed during execution before edits; this plan names likely touch points and their responsibilities.

- `crates/omsi-app/src/inspector.rs` (new): inspector mode state, stable selection handles, candidate/hit records, selected snapshot construction, invalidation, and pure selection-ordering logic. Keep this separate from editor and player behavior.
- `crates/omsi-app/src/lib.rs`: declare the inspector module following the app's current module organization.
- `crates/omsi-app/src/app.rs`: store inspector state, selection pin lease, and any UI-facing snapshot/status state on `App`.
- `crates/omsi-app/src/input_script.rs`: route toggle, mouse clicks, cursor-to-ray conversion, and automation/scripted input hooks; inspector mode consumes clicks before cockpit/editor interactions.
- `crates/omsi-app/src/app_events.rs`: construct per-frame inspector snapshot, reconcile entity/tile lifetimes, submit selection visualizations, and pass inspector view data into UI rendering.
- `crates/omsi-app/src/player.rs`: expose or factor vehicle mesh-hit collection so inspector can reuse transforms/broadphase/triangle intersection without firing mouse events. Preserve `pick`, `pick_trailer`, `hovered_part`, and `click` contracts.
- `crates/omsi-app/src/scene.rs`: expose loaded scenery render metadata needed to map a hit to a placed instance, including object identity, mesh data/transform, visibility/LOD state, and optional bounds. Do not make generic inspection depend on editor-only `EditObject`.
- `crates/omsi-app/src/tiles.rs`: add a selected-tile pin lease/reference-count mechanism and integrate it with normal unload, reload, requested/in-flight state, and world teardown.
- `crates/omsi-app/src/traffic.rs`: provide stable identities/mapping for AI/parked runtime vehicles to their render instances when required; avoid constructing identity from collection position alone.
- `crates/omsi-app/src/lan.rs`: expose remote vehicle generation/identity as needed, or define a stable key from existing remote id and vehicle replacement events.
- `crates/omsi-app/src/ui.rs`: add inspector view model input, panel/hitbox layout, selection status, clear/toggle commands, optional bounds/axes toggles, and UI tests.
- `crates/omsi-app/locales/app.yml`: add labels/status strings in all supported language sections following existing translation conventions.
- `crates/omsi-render/src/lib.rs`: only if existing APIs cannot support a selected-object marker/lines; prefer existing coronas/overlays and add a minimal debug-line primitive path only when demonstrated necessary.
- Existing test modules in the touched Rust files, or a new focused `crates/omsi-app/src/inspector.rs` test module: deterministic synthetic geometry and lifecycle tests.
- `docs/USER_GUIDE.md`: document inspector activation, selection behavior, supported targets, panel fields, and current limitations after implementation.

---

## Core Interfaces and Data Flow

### Inspector ownership

`App` owns an `Inspector` state. It must not own or mutably borrow `VehicleInstance`, `World`, renderer instances, or tile GPU allocations directly. Its stored selection is a stable handle and a read-only snapshot is rebuilt from authoritative data at defined boundaries.

### Selection records

Create a public-to-crate `InspectorSelection` enum with variants conceptually equivalent to:

- Vehicle mesh: `VehicleKey`, optional coupled-section identity, stable mesh definition key.
- Scenery mesh/object: tile coordinate, placed-object identity, stable mesh key.

A `VehicleKey` distinguishes at minimum player, AI/placed, and remote vehicle classes; it includes a generation token whenever the runtime can replace an instance under the same domain ID. Scenery object identity must not be only `ObjectType` or source path because many placed instances share the same asset.

A `InspectorHit` carries the handle, ray distance, part-local transform, world transform, and metadata required to build the panel. A `InspectorSnapshot` is an owned, UI-safe value with display strings and numeric transform/bounds values; it must not hold references across frames or lock scopes.

### Raycast flow

1. Input creates the same world-space ray convention used by existing cursor picking.
2. `Inspector::pick` asks domain adapters for visible candidate hits.
3. Each adapter broadphase-tests bounds and then calls `omsi_geometry::ray_mesh` only for viable candidates.
4. The app selects the nearest positive hit using a shared distance coordinate convention. Avoid the existing forgiving multi-ray ring behavior for inspector precision; aim at the cursor ray itself.
5. Selection lease changes are applied atomically from the old handle to the new one; invalid hits clear or retain current selection according to the explicit click policy (MVP default: empty-space click clears selection).
6. The renderer receives visual debug primitives derived from a validated snapshot, never raw indices left over from a previous frame.
7. UI receives the snapshot and inspector mode/status, renders controls, and reports panel hit regions to input routing so clicks in the panel do not select world geometry.

### Coordinate conventions

- World positions use simulation/world coordinates (double precision where the source does); do not expose render-origin-shifted coordinates as world positions.
- Local transforms use the same object/vehicle model convention used to transform the hit mesh.
- Articulated trailer transforms are relative to that part's own current world origin and pose.
- UI values are rounded for readability without mutating stored precision.
- Unknown or unavailable values are omitted or labeled unavailable, not invented from a parent transform.

---

## Implementation Tasks

### Task 1: Establish baseline contracts and test seams

**Files:**
- Inspect: `crates/omsi-app/src/player.rs`, `crates/omsi-app/src/scene.rs`, `crates/omsi-app/src/tiles.rs`, `crates/omsi-app/src/input_script.rs`, `crates/omsi-app/src/ui.rs`, `crates/omsi-app/src/app_events.rs`.
- Test: existing `omsi-app` and `omsi-geometry` unit test modules.

- [ ] Record current behavior and call paths for player part raycasting, articulated trailer raycasting, editor click/drag, mouse switch activation, tile unload/reload, and UI mouse hit regions.
- [ ] Identify available stable IDs/generations for AI, parked, player, and remote entities; write down any missing identity boundary before implementing selection.
- [ ] Run baseline `cargo test -p omsi-geometry` and focused existing `omsi-app` tests that build without requiring stock content; record environmental/content-dependent exclusions.
- [ ] Add no implementation changes in this task. Treat discovered incompatibilities as design constraints to fold into later tasks.

**Exit criteria:** Current interaction behavior and stable-identity gaps are known; focused baseline tests pass or documented failures are attributable to unavailable environment/content.

### Task 2: Define inspector selection model and stable identity tests

**Files:**
- Create: `crates/omsi-app/src/inspector.rs`.
- Modify: `crates/omsi-app/src/lib.rs`.

- [ ] Define stable keys for vehicle domains and scenery object instances, including generation/replacement semantics.
- [ ] Define `InspectorSelection`, `InspectorHit`, `InspectorSnapshot`, selection status, and view toggles as owned types. Keep renderer IDs and borrowed references out of persisted selection state.
- [ ] Define mesh logical identity using model source/path and definition index plus a disambiguator for duplicate names. Include a resolution policy when a precise mesh is absent at the current LOD.
- [ ] Implement pure candidate ordering: reject non-finite/non-positive distances, sort nearest-first, and use stable identity ordering as a deterministic tie-breaker.
- [ ] Test equal-distance tie-breaking, invalid distances, duplicate mesh names, entity-generation replacement, and LOD fallback identity behavior.
- [ ] Run `cargo test -p omsi-app inspector::`.

**Exit criteria:** Selection identity and ordering can be tested without loading map content or initializing a GPU.

### Task 3: Expose precise vehicle part hits without interaction side effects

**Files:**
- Modify: `crates/omsi-app/src/player.rs`.
- Modify: `crates/omsi-app/src/inspector.rs`.

- [ ] Factor a read-only vehicle mesh raycast helper that returns every useful nearest candidate or a nearest candidate with a complete stable handle, rather than triggering `[mouseevent]` bindings.
- [ ] Reuse the vehicle mesh visibility state, current `mesh_local_transform`, broadphase check, and `ray_mesh` triangle intersection.
- [ ] Apply the helper to the player's lead vehicle and every coupled/trailer section, with world-space ray distance comparable across sections.
- [ ] Add adapters for AI/placed and remote vehicle instances using the same pure helper; obtain identities from the owning collection rather than enumeration order.
- [ ] Ensure mirror-only or view-restricted meshes are not treated as visible world hit candidates unless they are actually visible in the current camera pass; confirm existing viewpoint and visibility conventions before finalizing this filter.
- [ ] Test lead-vs-trailer nearest selection, hidden mesh exclusion, non-interactive mesh inclusion, and proof that raycast does not fire triggers or mutate vehicle state.
- [ ] Run focused `cargo test -p omsi-app inspector::` and `cargo test -p omsi-app player::` if tests exist under those filters.

**Exit criteria:** All vehicle domains share one inspector-only, side-effect-free mesh intersection path, while cockpit picking remains unchanged.

### Task 4: Expose loaded scenery instance and mesh hits

**Files:**
- Modify: `crates/omsi-app/src/scene.rs`.
- Modify: `crates/omsi-app/src/inspector.rs`.

- [ ] Trace loaded tile render data back to each placed scenery object instance and its source mesh data. Do not infer instance identity from shared `ObjectType` or `.sco` path.
- [ ] Add a read-only iteration/query boundary that yields loaded scenery identities, world transforms, mesh logical identities, visible LOD mesh data, and bounds without leaking internal locks.
- [ ] Include editable and non-editable objects and account for object edits already applied to the render transform.
- [ ] Include currently rendered LOD geometry for intersection and retain stable logical mesh identity where lower LODs change the concrete mesh list.
- [ ] Avoid selecting duplicate/hidden render instances that are not visible to the active camera; verify LOD/visibility fields at the actual draw path.
- [ ] Test two placements of the same asset remain distinguishable, edited object transforms are reflected, LOD fallback retains object identity, and missing mesh data produces a valid object-level hit when a supported bound exists.
- [ ] Run `cargo test -p omsi-app inspector::`.

**Exit criteria:** Every loaded scenery placement, including those outside the editor's editable subset, can be attributed to a stable placed-object identity and accurate transform.

### Task 5: Add a bounded tile pin lease for selected scenery

**Files:**
- Modify: `crates/omsi-app/src/tiles.rs`.
- Modify: `crates/omsi-app/src/app.rs`.
- Modify: `crates/omsi-app/src/inspector.rs`.

- [ ] Add pin ownership keyed by tile coordinate with a lease/token so duplicate or stale releases cannot decrement another owner's pin.
- [ ] Integrate pin checks into ordinary stream distance-unload selection. Pinned tiles remain loaded but are not added to the streamer's camera centers, so selecting an entity does not cause surrounding map expansion.
- [ ] Define reload behavior: before explicit reload unloads a selected tile, notify/validate the inspector selection and release its pin; never leave an old handle referring to pre-reload objects.
- [ ] Handle tile disappearance, object removal, world replacement, inspector shutdown, and application shutdown by invalidating selection before freeing render resources.
- [ ] Permit only one scenery selection lease from this inspector; ensure vehicle selection releases a previous scenery tile lease.
- [ ] Test pin/unpin idempotence, normal unload of unpinned tiles, pinned-tile survival, replacement selection release, reload invalidation, and teardown release.
- [ ] Run `cargo test -p omsi-app tiles::` plus `cargo test -p omsi-app inspector::`.

**Exit criteria:** A selected scenery instance cannot be unloaded behind the inspector, and no tile remains pinned after its selection becomes invalid or the inspector closes.

### Task 6: Wire inspector mode and input precedence

**Files:**
- Modify: `crates/omsi-app/src/app.rs`.
- Modify: `crates/omsi-app/src/input_script.rs`.
- Modify: `crates/omsi-app/src/app_events.rs`.
- Modify: `crates/omsi-app/src/ui.rs`.

- [ ] Choose an available key binding after checking `app_events.rs`, `input_script.rs`, and OMSI `keyboard.cfg` handling. Avoid assuming the audit's `Ctrl+`` binding is free; document the chosen binding in UI/help text.
- [ ] Add inspector mode and click actions to `App`. Mode entry clears stale hover/drag state as needed; mode exit releases tile leases and clears debug overlays/selection.
- [ ] Give inspector mode first refusal over world left-clicks, before cockpit `Player::click`, object editor picking/drag, and mouse-driven interaction. Keep escape/menu, chat typing, panel interaction, and pointer capture behavior coherent.
- [ ] Ensure a click on the inspector panel or another existing UI region is consumed by UI and does not select behind it. Empty-space world clicks clear selection. A new hit replaces selection and pin lease in one operation.
- [ ] Preserve existing editor behavior when editor mode is active outside inspector mode. If both modes can be entered, define and enforce mutual exclusion or explicit activation transition; never let both consume the same click.
- [ ] Add deterministic input-routing tests for inspector-on/off, cockpit switch under cursor, editor active, panel hit, chat/menu open, and mode exit.
- [ ] If `OMSI_INPUT` can automate clicks/keys for this app, add or adapt commands so this mode and world selection can be covered in scripted runs without introducing test-only production behavior.
- [ ] Run `cargo test -p omsi-app input_script::` and the new inspector routing tests.

**Exit criteria:** Inspector mode consumes only its intended interactions, and all existing interactions retain their previous paths when the mode is inactive.

### Task 7: Build validated snapshots for display

**Files:**
- Modify: `crates/omsi-app/src/inspector.rs`.
- Modify: `crates/omsi-app/src/app_events.rs`.
- Modify: `crates/omsi-app/src/traffic.rs` and/or `crates/omsi-app/src/lan.rs` only where entity identity lookup is needed.

- [ ] Resolve each stored selection against current authoritative runtime state once per frame or on state-generation changes; create owned `InspectorSnapshot` values.
- [ ] Include kind, source file/config path, mesh name and stable/logical mesh identity, parent vehicle/object display name, tile coordinates for scenery, and world/local transform. Include bounds only when actual mesh/object bounds are available.
- [ ] Detect stale selection after AI despawn, remote timeout, bus replacement, trailer change, scenery reload, object deletion, or LOD mesh disappearance.
- [ ] For stale handles, release leases and expose a short-lived unavailable status, then clear the stored selection. Do not silently bind a replacement object that reused an index.
- [ ] Avoid holding locks while rendering UI or issuing draw commands; copy snapshot values and drop guards before those stages.
- [ ] Test snapshot coordinate consistency, remote replacement invalidation, AI generation invalidation, and scenery reload invalidation.
- [ ] Run focused inspector tests and `cargo check -p omsi-app`.

**Exit criteria:** The UI and renderer consume an owned validated snapshot, never a stale runtime reference.

### Task 8: Add selected-entity visual feedback

**Files:**
- Modify: `crates/omsi-app/src/app_events.rs`.
- Modify: `crates/omsi-render/src/lib.rs` only if existing `Scene` facilities are insufficient.
- Test: render/app test path available in repository.

- [ ] Start with existing corona/highlight and overlay support. Use a stable screen-readable color that contrasts against the audit's object-editor magenta marker.
- [ ] Add optional bounds and local X/Y/Z axes based on the validated current world transform, with axes expressed in the inspected entity's local basis.
- [ ] Do not add a new render pipeline unless a targeted render test or visual prototype shows existing primitives cannot represent the overlays. If a line path is required, keep it debug-only, transient per-frame, depth-tested by default, and excluded from shadow/reflection passes unless explicitly justified.
- [ ] Ensure overlays are removed immediately on clear, deselection, invalidation, inspector exit, and world change.
- [ ] Verify lead vehicle, trailer, scenery rotation, scenery elevation, and object-editor-adjusted transform placement with an offscreen screenshot or reproducible in-game test.
- [ ] Check overlay cost with many normal scene entities; selected-entity overlay work must scale with one selection, not the number of meshes.

**Exit criteria:** Selection is unambiguous in the scene and overlay geometry follows the live selected transform without lingering after invalidation.

### Task 9: Add compact custom inspector panel and localization

**Files:**
- Modify: `crates/omsi-app/src/ui.rs`.
- Modify: `crates/omsi-app/src/app_events.rs`.
- Modify: `crates/omsi-app/locales/app.yml`.

- [ ] Add an owned inspector view input containing mode state, optional snapshot/status, and supported display toggles. Keep `ui.rs` independent of mutable simulation entities.
- [ ] Lay out a compact side panel with title/mode state, selected object/part, parent, asset, world position/rotation, local position/rotation, bounds, tile if applicable, and clear/close actions.
- [ ] Define a pure panel layout/hit-test method so input routing can consume panel clicks before world picking. Use physical-pixel coordinates and the same scale conventions as existing widgets.
- [ ] Make the panel responsive to small and large surfaces: clamp dimensions, avoid chat/menu/cursor tooltip overlap where practical, and ensure long asset paths wrap, truncate with an accessible full-path alternative, or clip within the panel bounds without escaping it.
- [ ] Add localized labels and statuses in every supported locale section, following existing fallback conventions. Avoid adding implementation prose into visible UI.
- [ ] Add tests for panel hit-test boundaries, small/large screen layout, no-selection state, unavailable status, and long paths.
- [ ] Capture screenshots at representative 16:9 desktop and narrow/mobile-sized resolutions and inspect for overlap/clipping.
- [ ] Run focused `cargo test -p omsi-app ui::` and locale validation available in the repository.

**Exit criteria:** The panel works through the existing overlay renderer, reports accurate current data, consumes its own clicks, and fits target display sizes.

### Task 10: Add user documentation and regression coverage

**Files:**
- Modify: `docs/USER_GUIDE.md`.
- Modify: `crates/omsi-app/src/inspector.rs` and other touched test modules as necessary.

- [ ] Document the selected key, toggle lifecycle, click-to-select/empty-click-to-clear, supported target types, mesh/parent distinction, panel fields, and limitations.
- [ ] Add or extend scripted end-to-end coverage for entering inspector mode, selecting a vehicle part, selecting scenery, replacing a selection, clearing, and exiting.
- [ ] Add a busy-scene performance check or profiling recipe covering the player bus, traffic, remotes, and loaded scenery; establish a measurable budget before optimization work.
- [ ] Run `cargo test -p omsi-geometry`, `cargo test -p omsi-app`, and `cargo check --workspace` after focused tests pass.
- [ ] Manually verify normal gameplay, cockpit hover/click, object editor select/drag, chat, menu, free camera, and stream boundaries with inspector both on and off.
- [ ] Inspect the final diff for unrelated edits and ensure no texture, script-editing, or render-control capabilities slipped into the MVP.

**Exit criteria:** The feature is documented, regression-tested, and workspace checks pass; any content-dependent/manual-only tests are called out explicitly.

---

## Scope-Creep and Follow-On Register

The following are attractive extensions surfaced by the audit and implementation design. They are not MVP acceptance criteria. Each needs an independent mini-spec and acceptance criteria before work begins.

### A. Material and texture inspection

Potential value: diagnose wrong texture slots, formats, mip chains, alpha modes, script textures, and material parameters in context.

Risks: GPU texture lifetime and cache eviction, compressed-format readback, render-thread synchronization, dynamic script texture churn, accidental large memory spikes, display of private/local content paths, and unclear distinction between source image and runtime override.

Promotion criteria: MVP selection handles and snapshot identities are stable; texture metadata can be read without GPU stalls; a memory/readback budget and export destination policy are agreed. Begin with metadata only, then safe preview, then explicit dump action as separate increments.

### B. Script variables and live editing

Potential value: inspect animation inputs and vehicle/scenery state; add variables to a watch table.

Risks: variable ownership differs for player/AI/remote/scenery; variable reads can race parallel AI updates; writes can violate simulation invariants or cause dangerous vehicle behavior; strings and numeric values have different state APIs; remote edits require authority/security policy; variable names can be huge.

Promotion criteria: read-only watch capability first, a thread-safe snapshot boundary, per-entity variable ownership mapping, explicit multiplayer permission rules, and an audit log. Do not expose a generic force-value control without value/type validation, permission checks, range constraints, and reset semantics.

### C. Render pass controls and mesh isolation

Potential value: inspect draw passes, hide/isolate mesh, wireframe selected geometry.

Risks: the rendered scene may share instances/materials among consumers; hiding by mesh path can affect all instances; shadow/mirror/reflection passes diverge from the main camera; state may leak beyond the selected target or persist into gameplay; draw call IDs are frame-ephemeral.

Promotion criteria: define whether each control is per-instance or per-asset, pass scope, reset-on-exit behavior, and interaction with shadows/mirrors before adding controls. Start with non-mutating pass labels/statistics, defer force-hide/wireframe until instance-level targeting is proven.

### D. Terrain and road spline selection

Potential value: inspect blended ground, spline junctions, terrain holes, collision/deformation geometry, and navigation structures.

Risks: geometry is generated across tile boundaries and may be duplicated or merged; the hit mesh may not correspond to one authorable source record; tile streaming and ground regeneration invalidate handles; selection hierarchy differs from object/vehicle mesh hierarchy.

Promotion criteria: source provenance from generated geometry back to map/spline/terrain records, stable IDs across tessellation/regeneration, and dedicated selection semantics (surface patch vs source definition).

### E. Human/pedestrian selection

Potential value: inspect model, pose, queue/avoidance state, and route decisions.

Risks: skinning and pose updates run in parallel and may be distance/throttle dependent; identities may be pooled/recycled; simplified collision shapes differ from rendered geometry; large crowds make naïve raycasts expensive.

Promotion criteria: stable human generation IDs, a safe pose snapshot barrier, broadphase spatial indexing, and explicit distinction between rendered-pose hit and collision-body hit.

### F. Object editor integration

Potential value: inspector could offer edit selection, object record operations, save/undo, or variant changes from the panel.

Risks: existing editor has LAN synchronization, undo/save, object copies, terrain brush, tile ownership, and map-file persistence. Combining state machines could make inspection clicks unexpectedly destructive or undermine the editor's current editing rules.

Promotion criteria: keep selection sharing read-only first; separately specify editor mode transitions, authority, undo granularity, dirty-state presentation, and save conflict behavior. MVP does not replace editor picking or editing.

### G. Click-through selection, cycling, and occlusion policy

Potential value: cycle through stacked candidates or intentionally select an occluded object.

Risks: current scenery editor uses forgiving angular scoring rather than strict triangle occlusion; cockpit picker has pixel forgiveness; a unified tool needs one predictable policy. Exact triangle raycasts may miss thin geometry or transparent panes; ignoring depth may select hidden entities.

Promotion criteria: user testing identifies need. MVP uses strict cursor ray and nearest positive visible mesh; consider a candidate cycling affordance only after exact-hit reliability is measured. Any transparent/cutout policy must match visible rendering or be exposed explicitly.

### H. Additional editor targets and map authoring

Potential value: select tree instances, traffic lights, route helpers, attached scenery, and parked cars with their source records.

Risks: many are generated/attached/helper render data without a simple editable map record; multiple source records can contribute to a single rendered type; parking objects can transition into AI vehicles.

Promotion criteria: classify each runtime target as authorable instance, generated helper, or shared asset; provide distinct stable identities and avoid accidentally advertising editability.

### I. Persistent inspector layouts and automation

Potential value: saved panel placement, hotkeys, JSON/CSV diagnostic exports, scripted inspect commands, screenshot-linked reports.

Risks: settings compatibility, locale-sensitive parsing, filesystem permissions/path traversal, unstable identifiers in saved sessions, and support burden for an undocumented diagnostic format.

Promotion criteria: demonstrate repeated workflows and define versioned output schemas and privacy/path handling. Keep this distinct from the initial panel.

### J. Multiplayer and administrator tooling

Potential value: inspect remote entities or share a selected entity with another participant.

Risks: authority changes, player privacy, unstable network state, exposing internal paths/variables, host/client mismatch, and risk of treating inspection as a command.

Promotion criteria: read-only local view remains the default; any shared inspector state needs a network protocol, permission policy, privacy review, and compatibility/versioning design.

### K. Selection performance and broadphase data structures

Potential value: pick consistently in large maps with dense traffic and large tile radii.

Risks: a straightforward scan of every loaded scene mesh can negate streaming/render optimizations; broadphase structures can become stale as vehicles move, LOD changes, tiles load, and object edits update transforms.

Promotion criteria: collect measured candidate counts and raycast timings first. Add a spatial index only if the MVP budget is exceeded, with invalidation tied to existing tile/traffic generations. Never optimize by reducing correctness silently.

---

## Risk Register and Mitigations

| Risk | Why it matters | Planned mitigation / acceptance condition |
|---|---|---|
| Unstable runtime indices | Vectors reorder and entities are pooled or replaced | Store domain ID plus generation; re-resolve every snapshot and invalidate on replacement |
| Shared scenery types | One asset can have many loaded placements | Key by placed-object instance identity and tile, not source path/type |
| Render and simulation coordinate mismatch | Incorrect panel values and displaced gizmos | Use source simulation transforms and explicitly test render-origin conversion |
| LOD replacement | Concrete mesh indices differ over distance | Retain logical mesh identity and degrade to parent object/part if unavailable |
| Vehicle visibility differs by view/pass | Hidden mesh could win a raycast | Match current camera visibility and viewport rules; test cockpit/exterior/mirror cases |
| Scenery reload while pinned | Existing resources become stale | Explicit reload invalidates and releases before unload/rebuild; re-selection requires a new hit |
| Memory growth from pins | Users can keep remote map tiles alive | Single lease only, explicit mode/selection release, no expanding camera center, teardown tests |
| Panel and world click conflict | Panel click can select an object behind UI | Expose tested panel hitboxes to input precedence |
| Existing input behavior regression | Cockpit switches/editor operations are interactive | Inspector consumes clicks only while active; routing tests and manual regression matrix |
| Thin/transparent geometry | Exact ray may not match perception | MVP policy documented and measured; do not reintroduce fuzzy rays without tests |
| Draw overlays in mirrors/shadows | Debug geometry can leak into unrelated passes | Prefer existing main-view overlays; new pipeline excluded from mirrors/shadows unless intentional |
| Stale GPU IDs | Renderer frees/recycles resources during streaming | Build overlays from validated snapshot each frame; selection state stores no GPU IDs |
| Internationalization gaps | New panel appears inconsistently across languages | Translate all labels/statuses using existing locale pattern and fallback tests |

---

## Verification Matrix

- **Geometry/unit:** nearest positive hit, exact cursor ray, deterministic equal-distance tie, behind-camera rejection, bounds broadphase, hidden-mesh exclusion, visible triangle hit.
- **Identity/lifetime:** same asset at two locations, AI despawn/reuse, remote bus replacement, trailer removal, tile pin acquire/release, reload and world teardown.
- **Input:** inspector active/inactive, panel click, chat/menu open, cockpit switch, mouse steering, object editor, empty-world click, mode exit.
- **Visual:** player part, trailer part, scenery instance, rotated scenery, modified scenery, bounds/axes, deselection, stream boundary, exterior/cab camera.
- **UI:** no selection, selected vehicle, selected scenery, unavailable status, long source path, narrow screen, high DPI, chat/menu coexistence.
- **Performance:** pick a busy scene and report candidate count plus query duration; ensure no all-triangle-per-frame scan and no cost proportional to all entities for overlay rendering.
- **Regression:** existing cockpit interaction, hover tooltip, object editor click/drag/wheel/save, chat, menu, free camera, and ordinary streaming work unchanged with inspector off.
- **Build/test:** focused crate tests first; then `cargo test -p omsi-geometry`, `cargo test -p omsi-app`, and `cargo check --workspace`.

## Completion Checklist

- [ ] MVP boundaries above are met; all follow-on register items remain out of scope unless explicitly approved.
- [ ] A selected vehicle mesh reports a stable parent vehicle and coupled section where applicable.
- [ ] Any loaded scenery instance is selectable and remains valid through its one-tile lease, or displays unavailable after reload/removal.
- [ ] Clicking inspector UI never picks world geometry; inspector off preserves existing click behavior.
- [ ] No persistent renderer IDs or borrowed simulation references are stored in inspector selection state.
- [ ] Tile leases are released on every transition and teardown path.
- [ ] Selection visualization is current-frame, bounded in cost, and absent when no valid selection exists.
- [ ] Documentation, localization, tests, and verification matrix are complete.
- [ ] No unrelated files or changes were included.
