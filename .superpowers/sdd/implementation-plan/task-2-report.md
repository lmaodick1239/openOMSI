# Task 2 Implementation Report

## Scope

Implemented the desktop-only ImGui inspector state/frontend boundary. Android remains excluded at compile time through `cfg(not(target_os = "android"))`; no HUD, launcher, menu, VR/XR, or simulation code was changed.

## Changes

- Added workspace dependencies compatible with repository `winit 0.30` and `wgpu 29`:
  - `imgui 0.12`
  - `imgui-winit-support 0.13`
  - `imgui-wgpu 0.28`
- Added `crates/omsi-app/src/inspector/imgui_inspector.rs` with:
  - Owned `InspectorUiSnapshot` inputs for all Task 1 view models.
  - `InspectorUi` context/platform/optional renderer wrapper.
  - Recoverable platform and renderer errors via `last_backend_error`.
  - Typed FIFO `InspectorCommand` queue with `queue_command`/`drain_commands`.
  - Input-capture state for pointer and keyboard suppression.
  - Nine floating windows: Inspector, Hierarchy, Log, Materials, Render, Humans, Telemetry, Editor, Export.
  - Centralized blue theme.
  - Versioned `ImGuiLayout` persistence with default layout and invalid-data/version fallback.
- Registered the module only for non-Android targets.
- Cargo.lock updated for the new dependencies.

## Tests and validation

Commands were run from `/mnt/Random/users/user/Documents/GitHub/openOMSI/.worktrees/imgui-inspector-migration`.

```text
cargo test -p omsi-app inspector::imgui_inspector::tests --lib --quiet
running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 183 filtered out; finished in 0.00s
```

```text
cargo check -p omsi-app --quiet
exit code: 0
```

```text
git diff --check
exit code: 0
```

The focused tests cover layout round-trip and fallback, default window visibility/window identifiers/theme constant, and FIFO owned command queue behavior without requiring a live GPU.

## Task 3 integration notes

Task 3 should call `handle_event`, `begin_frame`, `draw`, and `render` around the existing desktop frame, pass immutable owned snapshots, and drain commands only at the existing application mutation boundary. The renderer is optional until `attach_renderer` succeeds; backend failures are retained as diagnostics and do not stop the game loop. Layout save/load can use `ImGuiLayout::save` and `ImGuiLayout::load` at controlled lifecycle points.

## Review Fixes (commit 4450934 follow-up)

- Updated `InspectorUi::draw` to accept the desktop `winit::window::Window` and capture each visible window's live ImGui position, size, and close-button visibility into `self.layout` before serialization. `WindowFlags::NO_SAVED_SETTINGS` remains enabled, so persistence stays explicitly controlled by `ImGuiLayout` rather than ImGui's ini state.
- Added `WinitPlatform::prepare_render(ui, window)` after all windows are drawn and before `Context::render()`, ensuring native cursor/platform state is applied for the frame.
- Added a focused GPU-free layout capture test covering geometry and visibility updates.

Validation after the review fixes:

```text
cargo test -p omsi-app inspector::imgui_inspector::tests --lib --quiet
running 4 tests
....
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 183 filtered out; finished in 0.00s
```

```text
cargo check -p omsi-app --quiet
exit code: 0
```

```text
git diff --check
exit code: 0
```
