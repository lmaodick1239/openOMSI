# ImGui Inspector Migration Design

## Summary

Rescope the openOMSI inspector from a game-overlay/debug panel into a desktop-style, multi-window editor modeled after the attached 03D viewer reference. The inspector will use Dear ImGui through Rust bindings and will present movable, resizable windows with a blue title/header treatment. The normal OMSI HUD and menu remain on the existing UI path.

## Goals

- Provide an inspector that opens as independent ImGui windows rather than one fixed panel.
- Match the reference interaction model: blue window chrome, collapsible sections, scrollable property panels, hierarchy, and log output.
- Allow the selected entity or asset to be inspected and acted upon through context-appropriate controls.
- Preserve the existing inspector selection identity, validation, snapshot, export, telemetry, and override logic.
- Keep the migration desktop-focused and avoid changing Android, VR, or the normal in-game UI in the first release.
- Make window visibility, placement, size, and docking state persistent between runs.

## Non-goals

- Replacing the existing OMSI HUD, launcher, or game menu.
- Adding arbitrary runtime editing to systems that currently expose read-only data.
- Replacing the renderer's overlay pipeline for ordinary game UI.
- Supporting ImGui rendering in Android or XR during the initial migration.
- Introducing a second scene/entity model solely for the UI.
- Maintaining a second inspector frontend after ImGui parity is reached.

## Recommended technology

Use the Rust Dear ImGui ecosystem:

- `imgui` for the immediate-mode UI API.
- `imgui-winit-support` for input, DPI, and platform integration with `winit 0.30`.
- `imgui-wgpu` for rendering through the existing `wgpu 29` device, queue, and surface.

Versions must be selected for compatibility with the repository's current `winit` and `wgpu` versions. Dependencies should be declared in the workspace and enabled only for desktop targets if the selected ImGui backend does not support Android.

## Architecture

### Inspector state remains UI-independent

`inspector_core.rs` and the existing inspector submodules remain the source of truth for selection and inspected data. ImGui code must consume owned view models/snapshots and dispatch explicit inspector commands; it must not borrow mutable simulation objects across the UI frame.

Introduce a desktop-only `InspectorUi` state containing:

- ImGui context/platform/renderer ownership or a narrowly scoped wrapper.
- Window-open flags for Inspector, Hierarchy, Log, Materials, Render, Humans, and Telemetry.
- Per-window size/position/docking persistence identifiers.
- Current pointer position, keyboard capture state, and last rendered surface size.
- A queued command list for actions requested by widgets.

The existing `App` owns this state and invokes it once per desktop frame after simulation updates have produced the current inspector snapshot.

### Rendering flow

For desktop frames:

1. Process the `winit` event through ImGui's platform integration.
2. Let the existing game and inspector input routing update selection state.
3. Build an immutable inspector view model from the current selection and snapshots.
4. Start an ImGui frame.
5. Draw enabled inspector windows.
6. Render the 3D scene using the existing renderer.
7. Render ImGui draw data into the same swapchain texture after the 3D pass and before presentation.
8. Execute queued inspector commands at the existing application boundary.

The ImGui pass must be composited after the world so windows remain readable and are not affected by scene depth, lighting, or post-processing.

### Window layout

The initial desktop layout consists of:

- **Inspector**: selected-object summary, stable identity, transform, object metadata, and action buttons. It displays vehicle, trailer, scenery, human, mesh, and material selections and clearly reports stale selections.
- **Hierarchy**: selectable scene/entity tree with visibility toggles where supported, stable-key selection, and invalidation feedback.
- **Log**: scrollable diagnostic/application log with clear and follow-tail controls, including export and backend errors.
- **Materials**: material, texture, blend, shader, alpha, PBR, mipmap, and sandbox-override data for a selected mesh/material.
- **Render**: render-pass/frame-graph tree, GPU timings, isolation mode, wireframe, normals, UV seams, collision hulls, and frame-graph export controls.
- **Humans**: selected human/driver state, raycast, skeleton, path, and pedestrian debug information.
- **Telemetry**: frame timing, inspector query/validation timing, GPU staging/readback timing, streaming counters, and snapshot statistics.
- **Editor**: promotion to the object editor for editable targets, transform sandbox controls, transaction history, apply, revert, and rollback actions.
- **Export**: selected mesh/entity glTF export controls, destination/status, and recoverable error display.

ImGui is the only inspector frontend after parity is reached. Existing inspector modules provide UI-independent snapshots, view models, and typed commands; they do not render directly through another UI toolkit. Inspector and Hierarchy are visible by default when inspector mode is enabled. Other windows are opened from a small Inspector menu or toolbar. Each window can be moved, resized, collapsed, closed, and optionally docked.

### Inspector module boundary

The existing inspector modules remain responsible for domain state and operations, but their widget-specific rendering methods are removed or converted to UI-independent APIs. For example, material and render modules expose owned view models and typed commands rather than accepting an `egui::Ui` reference:

```rust
pub fn material_view(&self) -> Option<MaterialView>;
pub fn apply_command(&mut self, command: InspectorCommand) -> Result<(), InspectorError>;
```

The ImGui frontend consumes these view models and queues commands. Commands are applied at the application boundary after validation, preserving stable selection keys and preventing stale UI references from mutating simulation state. This applies to core selection, hierarchy, materials, render debugging, humans, telemetry, export, persistence, and the editor bridge.

### Visual style

Use a restrained blue theme based on the reference:

- Dark charcoal application background.
- Blue title bars and section headers.
- Slightly lighter blue hover/active states.
- Light gray text with high contrast.
- Compact spacing and dense property rows.
- No custom texture-based window chrome for the first implementation; use ImGui styling so resizing and docking remain reliable.

The style must be centralized in one function so it can be tuned without changing individual panels.

## Interaction and input rules

- ImGui receives desktop pointer, wheel, keyboard, and text events through `imgui-winit-support`.
- When ImGui reports that a window wants pointer or keyboard input, the event must not also drive camera look, vehicle controls, chat, or game-menu shortcuts.
- Escape closes the active ImGui popup/window interaction before changing game state; a second Escape retains existing game behavior.
- Inspector selection remains available through the existing world-picking path when the pointer is not over an ImGui window.
- The inspector must not capture input when it is disabled or when no ImGui window is hovered/focused.
- Window state is saved on controlled changes and at shutdown, not every frame.

## Commands and mutation boundary

Widgets produce typed commands such as:

- `Select(InspectorSelection)`
- `SetVisibility(EntityKey, bool)`
- `SetTransform(EntityKey, TransformEdit)` where the existing edit capability permits it
- `SetMaterialOverride(MaterialOverride)`
- `SetRenderFlag(RenderFlag, bool)`
- `ExportSelection(ExportRequest)`
- `ClearLog`
- `ToggleWindow(InspectorWindow, bool)`

Commands are validated against the current stable key/snapshot before mutation. Stale selections become a visible “Selection no longer available” state and cannot mutate a replacement entity.

## Persistence

Persist a versioned desktop inspector layout under the existing openOMSI settings/config location. The record contains:

- Window open/closed flags.
- Position and size for each window.
- Docking layout, if enabled by the chosen ImGui backend.
- Theme version and layout schema version.

Invalid or older layouts fall back to the default blue layout. Persistence failures are logged but must not prevent the game from starting.

## Error handling

- If ImGui initialization or the backend renderer cannot be created, log the error and disable only the ImGui inspector; the game remains playable with existing inspector behavior or no inspector UI.
- Handle swapchain resize, scale-factor changes, surface loss, and minimized windows without panics.
- Avoid rendering an ImGui frame when the surface is unavailable.
- Backend errors must include enough context to identify initialization, event handling, texture upload, or draw submission failures.

## Compatibility and rollout

Phase 1 is desktop-only and feature-gated at compile time where necessary. Existing `inspector_active`, selection, snapshot, and debug-draw behavior remain functional while the ImGui view is introduced. The old inspector rendering path should be removed only after the new path has parity for the currently exposed panels.

The first implementation should not add docking-specific APIs until the basic floating-window path works. Docking can be enabled after input, persistence, and rendering are stable.

## Testing and acceptance criteria

### Unit tests

- Inspector view-model construction is independent of ImGui and preserves stable selection identity for vehicle, trailer, scenery, human, mesh, and material targets.
- Command validation rejects stale or incompatible selections and prevents unsupported edits.
- Each migrated panel produces an owned view model without an `egui::Ui` dependency.
- Material, render, human, telemetry, editor, export, and persistence commands serialize or validate deterministically where applicable.
- Layout serialization round-trips positions, sizes, open flags, and schema version.
- Invalid layout data falls back to defaults.

### Integration tests/manual checks

- Build and run the desktop application with the inspector disabled and confirm no behavior or rendering regression.
- Enable the inspector and verify Inspector, Hierarchy, and Log windows appear with blue chrome.
- Move and resize every window, restart, and verify layout restoration.
- Hover/focus an ImGui control and verify camera, vehicle, chat, and game shortcuts do not receive the same event.
- Select player, AI, trailer, scenery, mesh/material, and human targets and verify the correct panel content appears.
- Resize the window, change display scale, minimize/restore, and verify the UI remains correctly positioned and rendered.
- Exercise the existing export, render override, and debug visualization actions through queued commands.
- Run the existing targeted inspector tests and the workspace check/build for desktop.

Acceptance is met when the inspector visibly behaves as a collection of movable blue ImGui windows like the reference, every currently exposed inspector function is reachable through an ImGui window or command, no inspector module requires `egui`, and selection identity, existing inspector actions, and normal game controls remain correct outside those windows.

## Implementation boundary

Expected primary files:

- `crates/omsi-app/Cargo.toml`: desktop ImGui dependencies.
- `Cargo.toml`: compatible workspace dependency versions if needed.
- `crates/omsi-app/src/imgui_inspector.rs`: backend lifecycle, theme, windows, view-model rendering, and command queue.
- `crates/omsi-app/src/app.rs`: application ownership and initialization/state persistence.
- `crates/omsi-app/src/app_events.rs`: event forwarding, frame lifecycle, input capture, and final ImGui rendering.
- `crates/omsi-app/src/inspector/mod.rs`: shared view-model/command interfaces.
- `crates/omsi-app/src/inspector_core.rs`: only narrowly scoped adapters required to expose existing data.
- `crates/omsi-app/src/inspector/material_panel.rs`: UI-independent material view model and material commands.
- `crates/omsi-app/src/inspector/render_panel.rs`: UI-independent render view model and render commands.
- `crates/omsi-app/src/inspector/human_panel.rs`: UI-independent human view model and human commands.
- `crates/omsi-app/src/inspector/editor_bridge.rs`: editor view model and validated editor commands.
- `crates/omsi-app/src/inspector/export.rs`: export request/status view model and command adapter.
- `crates/omsi-app/src/inspector/telemetry.rs`: telemetry view model and display-ready counters.
- `crates/omsi-app/tests/`: view-model, command, and persistence tests.
- `docs/`: user-facing inspector controls and desktop-only limitation notes.

No unrelated simulation, renderer, or Android refactor is part of this migration.
