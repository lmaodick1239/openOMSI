# Launcher Inspector Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in launcher Inspector toggle that prevents inspector initialization when disabled, exposes an Inspector pause-menu button when enabled, and removes the global inspector keybind while retaining inspector-local controls.

**Architecture:** Persist the feature choice with the launcher duty JSON, carry it through `omsi-launcher-core::Duty` into a generated `--inspector` CLI argument, and gate `App` initialization/rendering/menu registration on `Args::inspector`. The pause menu remains the sole entry point into inspector mode; the ImGui inspector continues owning any controls needed after it is open.

**Tech Stack:** Rust, clap, serde/serde_json, winit, existing `omsi-ui`, launcher-core tests, Cargo workspace.

## Global Constraints

- The Inspector toggle defaults to `false`, including when loading older launcher duty files.
- Disabled sessions must not construct or attach `InspectorUi`, process inspector frames, or show an Inspector pause-menu item.
- Enabled sessions expose exactly one pause-menu action labeled `Inspector`; no global Ctrl+I toggle remains.
- Do not remove key handling owned by `imgui_inspector`; only remove the application-global toggle path.
- Preserve existing launcher persistence, argument generation, menu ordering, and inspector behavior when enabled.

---

### Task 1: Persist and expose the launcher toggle

**Files:**
- Modify: `crates/omsi-app/src/launcher/state.rs` (`Choice`, `Default`, `duty`)
- Modify: `crates/omsi-app/src/launcher/pages.rs` (`settings`/settings controls)
- Test: `crates/omsi-app/src/launcher/state.rs` existing choice tests or a focused test module

**Interfaces:**
- Produces `Choice::inspector: bool`, default `false`.
- Produces `core::Duty::inspector: Option<bool>` populated from the choice.

- [ ] **Step 1: Add a failing default/persistence test**

Add a unit test asserting `Choice::default().inspector == false` and that deserializing JSON without an `inspector` field yields `false` through `#[serde(default)]`.

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p omsi --lib launcher::state::choice_tests -- --nocapture`

Expected: compile failure because `Choice` has no `inspector` field.

- [ ] **Step 3: Implement the persisted field and duty mapping**

Add `pub inspector: bool` to `Choice`, initialize it to `false` in `Default`, and add `inspector: Some(c.inspector)` to `State::duty()`. Keep the existing `version` migration unchanged because serde’s missing-field default handles legacy files.

- [ ] **Step 4: Add the settings toggle**

Place a toggle in the launcher’s existing General settings section using the existing `Ui::toggle` pattern. Bind it to `l.state.choice.inspector`; when clicked, update `choice_dirty` using the same dirty/save mechanism used by other launcher choices. Label it `Inspector` and explain that it enables the in-game Inspector pause-menu button.

- [ ] **Step 5: Run focused launcher tests**

Run: `cargo test -p omsi --lib launcher::state::choice_tests -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/omsi-app/src/launcher/state.rs crates/omsi-app/src/launcher/pages.rs
git commit -m "feat: add launcher inspector toggle" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: Propagate the toggle into launcher-core and CLI

**Files:**
- Modify: `crates/omsi-launcher-core/src/lib.rs` (`Duty`, `duty_args`)
- Modify: `crates/omsi-app/src/cli.rs` (`Args`)
- Test: `crates/omsi-launcher-core/src/lib.rs` existing duty argument tests or a focused test module

**Interfaces:**
- Consumes `Duty::inspector: Option<bool>`.
- Produces `--inspector` only when `Duty::inspector.unwrap_or(false)` is true.
- Produces `Args::inspector: bool`, default false.

- [ ] **Step 1: Add failing argument tests**

Add tests constructing a minimal `Duty` with `inspector: Some(true)` and `Some(false)`, asserting the first argument list contains `--inspector` only for true. Add a test parsing `Args` with and without `--inspector` if the CLI test conventions permit direct clap parsing.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `cargo test -p omsi-launcher-core duty_args -- --nocapture`

Expected: compile failure because the new field is absent.

- [ ] **Step 3: Implement the argument propagation**

Add the serde-defaulted `inspector` field to `Duty`, add `#[arg(long)] pub(crate) inspector: bool` to `Args`, and append `--inspector` in each relevant duty argument path. For normal duties, append it alongside the other feature flags. For situation/tutorial paths, preserve current early-return semantics and add the flag before returning when enabled so launcher behavior is consistent.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p omsi-launcher-core duty_args -- --nocapture` and `cargo check -p omsi`

Expected: PASS with no argument-shape regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/omsi-launcher-core/src/lib.rs crates/omsi-app/src/cli.rs
 git commit -m "feat: pass inspector capability to game" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: Gate inspector initialization and frame work

**Files:**
- Modify: `crates/omsi-app/src/app.rs` (`resumed_impl` inspector setup and capability helpers)
- Modify: `crates/omsi-app/src/app_events.rs` (all inspector event/render guards and global toggle path)
- Modify: `crates/omsi-app/src/lib.rs` (`App` construction if needed)
- Test: `crates/omsi-app/src/app_events.rs` existing inspector capture tests, plus a capability-gating test

**Interfaces:**
- Consumes `self.args.inspector`.
- Guarantees `self.inspector_ui.is_none()` for disabled sessions after resume.
- Removes the application-level Ctrl+I recognition and forwarding exception.

- [ ] **Step 1: Add a failing capability test**

Add a test around the existing inspector setup boundary asserting that a disabled `Args` path does not create `InspectorUi`; use a pure helper if the existing app setup is too GPU-dependent.

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p omsi --lib app_events:: -- --nocapture`

Expected: failure or compile error before the guard exists.

- [ ] **Step 3: Gate initialization**

Wrap the non-Android `InspectorLayout` load, `InspectorUi::new`, renderer attachment, and assignment in `if self.args.inspector { ... }`. Ensure disabled sessions never call inspector layout loading or renderer attachment.

- [ ] **Step 4: Remove only the global keybind path**

Delete the `inspector_toggle` detection in `window_event`, remove the `imgui_keyboard_blocks_event` exception that made Ctrl+I bypass ImGui capture, and remove the global Ctrl+I mode switch from the application input path. Keep calls that deliver events to an already-created `InspectorUi` and preserve all mode-local handling inside `imgui_inspector`.

- [ ] **Step 5: Guard per-frame inspector work**

Require `self.args.inspector` together with `self.inspector_active` around selection overlays, snapshot generation, inspector frame begin/draw/render, command processing, capture handling, and layout saving. This ensures disabled sessions cannot accidentally do inspector work if stale state is present.

- [ ] **Step 6: Run focused tests and compile**

Run: `cargo test -p omsi --lib app_events:: -- --nocapture` and `cargo check -p omsi`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/omsi-app/src/app.rs crates/omsi-app/src/app_events.rs crates/omsi-app/src/lib.rs
git commit -m "feat: disable inspector runtime unless enabled" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: Add the pause-menu Inspector action

**Files:**
- Modify: `crates/omsi-app/src/input_script.rs` (`game_menu_items`, menu constants, menu selection)
- Modify: `crates/omsi-app/src/app_events.rs` or `input_script.rs` existing menu action dispatch location
- Modify: `crates/omsi-app/locales/app.yml` only if the project localizes this exact menu label
- Test: focused menu tests in `input_script.rs` or an existing app test module

**Interfaces:**
- Enabled `Args::inspector` yields a menu entry `("inspector", "Inspector")`.
- Disabled sessions never yield that entry.
- Selecting the entry sets `inspector_active = true` and closes/updates the pause menu using existing mode-transition behavior.

- [ ] **Step 1: Add failing menu tests**

Add tests asserting `game_menu_items()` contains `Inspector` only when `args.inspector` is true, and that selecting the `inspector` action activates inspector mode. Assert Ctrl+I does not activate it.

- [ ] **Step 2: Run focused tests and verify they fail**

Run: `cargo test -p omsi --lib input_script:: -- --nocapture`

Expected: failure because the menu entry and action are absent.

- [ ] **Step 3: Add the conditional menu entry**

Add `("inspector", "Inspector")` to the full game menu list (not the server menu unless the existing Inspector is intentionally supported there), and insert/retain it only when `self.args.inspector` is true. Place it in the extended menu so the pause menu’s button is available without changing the everyday basic list unless current UI conventions require it.

- [ ] **Step 4: Wire the action**

In the existing menu action match, handle `Some("inspector")` by setting `self.inspector_active = true`, clearing stale selection/capture as appropriate through existing inspector transition helpers, and closing the game menu. Do not add any keyboard shortcut.

- [ ] **Step 5: Run tests**

Run: `cargo test -p omsi --lib input_script:: -- --nocapture` and `cargo check -p omsi`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/omsi-app/src/input_script.rs crates/omsi-app/locales/app.yml
git commit -m "feat: open inspector from pause menu" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 5: Verify end-to-end behavior and update documentation

**Files:**
- Modify: `docs/USER_GUIDE.md` or the existing Inspector documentation file selected by repository conventions
- Test: workspace tests and CLI/launcher argument tests

**Interfaces:**
- Documents launcher opt-in, pause-menu entry, default-off behavior, and removal of global Ctrl+I.

- [ ] **Step 1: Update user documentation**

Document: enable Inspector in launcher settings, start a session, open pause menu, select Inspector; state that the Inspector is not loaded when disabled and that the old global keybind is no longer available. Mention that controls inside Inspector remain available once its mode is open.

- [ ] **Step 2: Run targeted tests**

Run: `cargo test -p omsi-launcher-core duty_args -- --nocapture` and `cargo test -p omsi --lib launcher::state::choice_tests input_script:: app_events:: -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Run final validation**

Run: `cargo check --workspace` and `cargo test --workspace`

Expected: successful check and tests; if unrelated pre-existing failures occur, report their exact package/test names without altering them.

- [ ] **Step 4: Review the diff**

Run: `git diff HEAD~5..HEAD --check` and `git status --short`.

Expected: no whitespace errors and only the intended implementation, tests, documentation, and plan/spec commits.

- [ ] **Step 5: Commit documentation if separate**

```bash
git add docs/USER_GUIDE.md
git commit -m "docs: explain inspector launcher toggle" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```
