# Inspector Selection and Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the ImGui Inspector windows and selection controls functional, repair viewport selection, and add white translucent hover-fill plus outline overlays with optional all-hit highlighting.

**Architecture:** Preserve the owned-snapshot/typed-command boundary. Fix ImGui layout state propagation in the desktop frontend, repair the Inspector viewport input-to-raycast-to-command path, and add highlight state to the existing render/debug-draw integration rather than implementing screen-space UI geometry. Hover defaults to the topmost hit; a persisted Inspector setting can show all penetration hits. Selected targets retain cyan treatment.

**Tech Stack:** Rust, winit, Dear ImGui, imgui-wgpu, wgpu, existing OMSI raycast/penetration and debug-draw systems, Cargo tests.

## Global Constraints

- Inspector overlays are active only while Inspector mode is active.
- Hover uses white translucent fill plus white outline; selection uses cyan and takes precedence when both refer to the same target.
- Hover defaults to the topmost ray hit; an Inspector setting enables highlighting all intersected hits without changing selection semantics.
- UI must emit typed `InspectorCommand` values and must not retain mutable simulation borrows across ImGui frames.
- Preserve existing non-Inspector input behavior and Android conditional compilation.
- Do not add external dependencies.
- Update directly related Inspector documentation.

---

### Task 1: Repair ImGui window visibility and Inspector selection input

**Files:**
- Modify: `crates/omsi-app/src/inspector/imgui_inspector.rs`
- Modify: `crates/omsi-app/src/app_events.rs`
- Modify: `crates/omsi-app/src/app.rs` and/or `crates/omsi-app/src/inspector_core.rs` only where required to complete the existing command/raycast path
- Test: existing Inspector frontend/application tests plus focused regression tests in the nearest existing test modules

**Interfaces:**
- Consumes: existing `ImGuiLayout`, `InspectorWindow`, `InspectorCommand`, `SelectionTarget`, penetration stack, cursor/raycast state.
- Produces: window menu and close-button state that persists and controls drawing; viewport clicks that select the intended `SelectionTarget`; hover state updated from the current topmost hit; an Inspector setting for all-hit highlighting exposed through the existing command/state boundary.

- [ ] **Step 1: Add failing regression coverage for the window menu state.** Assert that toggling a menu item changes the layout entry used by the same frame’s window draw, and that closing a window prevents it from being submitted on the next frame.
- [ ] **Step 2: Add failing regression coverage for selection dispatch.** Exercise the existing Inspector input path with a representative vehicle/scenery hit and assert that the typed select command targets that hit rather than being dropped or blocked by ImGui capture.
- [ ] **Step 3: Run the focused tests and verify they fail for the reported behavior.**
- [ ] **Step 4: Fix `InspectorUi::draw` so it renders windows from the updated menu/close state, not the stale cloned layout.** Preserve positions/sizes and safely handle the main Inspector window being closed.
- [ ] **Step 5: Trace and repair the existing viewport pointer path so Inspector-mode clicks update the raycast/penetration stack and queue `InspectorCommand::Select` for the topmost valid hit, while clicks on empty space clear selection.** Keep ImGui pointer capture authoritative when the cursor is over an Inspector window.
- [ ] **Step 6: Add the all-hit highlight preference through the smallest existing Inspector state/command mechanism, defaulting to false and not changing `CycleNextHit`, `CyclePrevHit`, or `JumpToHit` semantics.
- [ ] **Step 7: Run the focused tests and verify they pass.**
- [ ] **Step 8: Run `cargo fmt --all -- --check` and the relevant `cargo test -p omsi-app` targets.
- [ ] **Step 9: Commit the task as `fix: wire inspector windows and selection`.

---

### Task 2: Add 3D hover and selection highlight overlays

**Files:**
- Modify: `crates/omsi-app/src/app_events.rs` and the existing render/debug-draw integration files identified during Task 1
- Modify: `crates/omsi-render/src/*` only where the existing overlay renderer requires a focused API extension
- Test: focused overlay/state tests in the nearest existing render or app test modules

**Interfaces:**
- Consumes: owned hover target, selected target, penetration hit stack, all-hit preference, Inspector active state.
- Produces: renderer/debug-draw overlay commands or passes that render complete target geometry with white translucent fill and outline for hover, cyan for selection, and deterministic precedence for overlap.

- [ ] **Step 1: Add failing tests for overlay target selection.** Cover topmost-only default, all-hit mode, selected-over-hover precedence, and inactive Inspector mode producing no overlays.
- [ ] **Step 2: Run those tests and verify they fail before implementation.
- [ ] **Step 3: Implement a small owned highlight description/state type at the existing render boundary.** It must contain target identity and style/layer information, not live simulation references.
- [ ] **Step 4: Connect the Inspector hover/selection state to the renderer’s existing mesh/object traversal or debug-draw path.** Render a depth-aware translucent white fill and white outline for hover; render cyan for selection; do not duplicate geometry in ImGui.
- [ ] **Step 5: Apply topmost/all-hit filtering and selected-target precedence before submitting overlays.
- [ ] **Step 6: Run focused tests, then `cargo fmt --all -- --check` and relevant `cargo test` targets.
- [ ] **Step 7: Commit the task as `feat: highlight inspector hover targets`.

---

### Task 3: Runtime verification and documentation

**Files:**
- Modify: `docs/INSPECTOR_GUIDE.md`
- Test/build: project Cargo targets and the provided runtime at `/mnt/More_Games/openOMSI-0.1.78-linux-x64/`

**Interfaces:**
- Consumes: completed window, selection, and overlay behavior.
- Produces: documentation matching shipped behavior and verified executable/runtime behavior using an installed map and vehicle where available.

- [ ] **Step 1: Update the Inspector guide to document working window toggles, viewport selection, hover/selected colors, and the all-hit highlight setting.
- [ ] **Step 2: Run the smallest relevant Inspector tests and a release/build command that produces the executable.
- [ ] **Step 3: Copy/overwrite the provided runtime executable only if the build output is compatible, preserving a backup if the runtime workflow requires one.
- [ ] **Step 4: Launch a representative map/vehicle from the provided install and verify window toggles, click selection, hover fill/outline, selected cyan precedence, and all-hit mode. Capture any runtime failure for follow-up fixes.
- [ ] **Step 5: Run `git diff --check` and the final focused test/build commands.
- [ ] **Step 6: Commit documentation and verification-related changes as `docs: document inspector highlighting`.
