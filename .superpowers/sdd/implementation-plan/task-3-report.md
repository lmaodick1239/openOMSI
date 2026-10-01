# Task 3 Implementation Report

## Status

Implemented desktop ImGui inspector lifecycle integration at the application event/render boundary.

## Changes

- Added desktop-only `InspectorUi` ownership to `App`.
- Loaded persisted ImGui window layout during desktop window creation and saved it during application exit.
- Attached the ImGui wgpu renderer to the application renderer without changing Android/XR ownership.
- Forwarded cloned winit window events through `WinitPlatform` before normal application dispatch.
- Suppressed keyboard and pointer game handlers while ImGui reports capture, while preserving the Ctrl+I inspector toggle.
- Kept resize, scale-factor, focus, minimized, and surface-loss paths in the existing application lifecycle; ImGui receives the same events and unavailable surfaces skip presentation/reconfigure instead of shutting down.
- Began and drew an owned inspector snapshot once per active frame, then rendered ImGui after the world/HUD/touch pass and before presentation.
- Drained typed inspector commands after rendering at the application mutation boundary, validating against the current owned selection before applying supported selection/cycle commands.
- Added focused tests for independent keyboard/pointer capture routing and non-fatal backend-unavailable state.

## Validation

All commands were run from `/mnt/Random/users/user/Documents/GitHub/openOMSI/.worktrees/imgui-inspector-migration`.

```text
cargo test -p omsi-app inspector::imgui_inspector::tests --lib --quiet
running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 183 filtered out; finished in 0.00s
```

```text
cargo check -p omsi-app --quiet
exit code: 0
```

```text
git diff --check
exit code: 0
```

`cargo fmt --all -- --check` remains unavailable as a repository-wide gate because pre-existing formatting differences exist in unrelated files, including `crates/omsi-app/build.rs`; no formatter-wide rewrite was applied.

## Concerns

- No live GPU/window integration test was added because the existing test environment is headless. The renderer path is guarded by optional backend state and compile-checked; runtime backend failures are retained as recoverable diagnostics and do not call application exit.
- Existing XR rendering remains unchanged; the desktop ImGui overlay is only submitted to the normal desktop surface path.

## Task 3 Review Fix Report

- Inactive inspector capture is now explicitly disabled: stale `InputCaptureState` is treated as empty and cleared whenever the desktop inspector is inactive, so gameplay input cannot be suppressed by a prior ImGui frame.
- Window-event routing now separates keyboard and pointer capture. Keyboard events consult only `want_capture_keyboard`; mouse buttons, wheel, and cursor movement consult only `want_capture_mouse`. Ctrl+I remains exempt so the inspector can be toggled off.
- Desktop `DeviceEvent::MouseMotion` now uses the same active pointer-capture gate, preventing camera look and mouse steering while an ImGui window owns pointer input.
- Added focused tests for inactive stale-capture suppression and pointer-only device-motion gating.

### Validation

```text
cargo test -p omsi-app capture --lib --quiet
running 4 tests
....
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 187 filtered out
```

```text
cargo test -p omsi-app inspector::imgui_inspector::tests --lib --quiet
running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 185 filtered out
```

```text
cargo check -p omsi-app --quiet
exit code: 0
```

`cargo fmt --package omsi-app -- --check` remains blocked by the repository's pre-existing formatting difference in `crates/omsi-app/build.rs`; the changed source files were kept scoped and `git diff --check` passes.
