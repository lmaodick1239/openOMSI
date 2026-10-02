# Inspector Guide

The Inspector is openOMSI’s desktop visual-debugging interface. It shows an owned snapshot of the selected vehicle or scenery object and provides tools for understanding what is visible in the world without changing the simulation.

> **Current scope:** The Inspector frontend is desktop-only and primarily read-only. Some windows and view models are present as diagnostics surfaces, but mutation controls, interactive telemetry watches, and interactive export are not currently exposed through the Inspector UI.

## 1. Enable the Inspector

The Inspector is opt-in and disabled by default.

1. Open the openOMSI launcher.
2. Open **Settings → General**.
3. Enable **Inspector**.
4. Start or restart the game session.

When the option is disabled, the game does not initialize the Inspector frontend, load its layout, or expose its pause-menu action.

The setting is saved with the launcher’s remembered duty configuration. Existing launcher configurations that predate the option remain disabled until you enable it.

## 2. Open and close Inspector mode

With the Inspector enabled:

1. Open the in-game pause menu.
2. Select **Inspector**.
3. Select **Inspector** again from the pause menu to leave Inspector mode.

The Inspector is opened from the pause menu only. The former global `Ctrl+I` shortcut is not used.

Inspector mode and the object editor are mutually exclusive. Entering one mode leaves the other mode before it starts.

### Platform availability

- **Desktop:** The Dear ImGui Inspector windows are available.
- **Android:** The Inspector frontend is not available in this rollout; the normal HUD and input path remain active.
- **LAN:** When enabled, the Inspector action is available in the game menu for both ordinary sessions and LAN server/client menus. It inspects the entities available to the current game instance.

## 3. Select an entity

When Inspector mode is active, point the camera at an entity and left-click it.

The Inspector currently supports:

- The player vehicle.
- AI traffic vehicles.
- Remote LAN vehicles.
- Trailers and other vehicle parts represented by the vehicle hierarchy.
- Loaded scenery objects.

A cyan selection marker identifies the selected entity. This distinguishes Inspector selection from the object editor’s magenta marker.

Click empty space to clear the selection. If the selected entity is unloaded, despawned, or replaced, its snapshot becomes unavailable and the selection is cleared or invalidated rather than keeping a stale simulation reference.

### Selection identity

The Inspector uses stable selection identities rather than retaining live mutable objects across UI frames. Depending on the selected entity, the main panel can show:

- Entity type and identity.
- Model or asset path.
- World position.
- Rotation.
- Selected mesh name, when available.
- Entity-specific metadata.
- Tile coordinates for scenery.
- Parent/local information for attached vehicle parts such as trailers.
- Bounding information, when the runtime provides it.

## 4. Inspector window

The main **Inspector** window is the starting point for selection details and selection controls.

### Selection controls

- **Clear selection** removes the current selection.
- **Previous hit** and **Next hit** move through entities along the selection ray.
- Hit entries show their order, distance, and display name. Select a hit entry to jump directly to it.
- **Bounds** toggles the selected entity’s bounds overlay.
- **Local axes** toggles local-axis visualization.
- **Mesh name** toggles the selected mesh name overlay.

The exact controls shown depend on whether a current selection and a penetration/hit stack are available.

### Input capture

When an Inspector window has keyboard or pointer focus, it captures that input from the game. Camera movement, vehicle controls, and game shortcuts do not receive events that ImGui has claimed. Moving focus away from Inspector windows restores normal game input handling.

## 5. Additional windows

Open or hide windows from the Inspector’s **Windows** menu. Window positions, sizes, and visibility are saved and restored between desktop sessions.

The default layout opens the main Inspector and Hierarchy windows. Other windows can be enabled as needed.

### Hierarchy

**Hierarchy** lists the entities in the current Inspector snapshot. It is useful for checking what the runtime exposed and for locating related vehicle entries.

### Log

**Log** displays diagnostic lines supplied to the Inspector snapshot. It is a read-only view of current diagnostic information, not a separate log configuration interface.

### Materials

**Materials** displays material information when the selected entity has a material snapshot, including values such as:

- Material name.
- Shader variant.
- Metallic and roughness values.
- Other material data exposed by the snapshot.

Material mutation controls are not currently exposed through this window.

Material and texture readbacks are designed to be asynchronous. A missing or pending snapshot is shown as unavailable rather than blocking the game while the GPU is queried.

### Render

**Render** displays render diagnostics when frame-graph data is available:

- Total frame time.
- Draw-call count.
- Triangle count.
- Individual render passes.
- Per-pass GPU timing when available.

If render diagnostics are unavailable, the window shows the reason supplied by the runtime. Render mutation controls are not currently exposed through the Inspector.

### Humans

**Humans** displays a human/pedestrian snapshot when one is available, including:

- Human identifier and generation.
- Current animation.
- Animation phase.
- Playback state.
- Number and names of available skeleton bones.
- Skeleton details in a collapsible section.

The current selection/raycast workflow is centered on vehicles and scenery. A human panel can display snapshot data when supplied, but human mutation controls are not available.

### Telemetry

**Telemetry** displays timing and watch values already present in the snapshot:

- Frame time.
- Inspector query time.
- GPU staging time.
- Existing watch-expression values.

Creating, editing, or changing watch expressions from the Inspector is not currently exposed. For external telemetry, use the telemetry facilities provided by the build/runtime configuration rather than assuming the Inspector window can start or edit a watch session.

### Editor

**Editor** reports the state of the Inspector editor bridge and transform sandbox when that state is supplied. It can show whether a sandbox is active, the entity key, transform snapshots, and transaction count.

The Inspector UI currently exposes no mutation controls in this window. Use the separate object editor for the supported in-game scenery editing workflow.

### Export

**Export** reports the status of the Inspector export view and any error or destination information supplied by the runtime.

Interactive export controls are not currently available in the Inspector. Use the production command-line exporter, `--export-glb`, for supported glTF export workflows.

## 6. Layout persistence

The desktop Inspector stores its window layout separately from the simulation:

- Window positions are persisted.
- Window sizes are persisted.
- Window visibility is persisted.
- Invalid or unsupported layout data falls back to the default layout.

If a layout becomes unusable, close the game and remove the Inspector layout file from the openOMSI data directory so the next session can recreate the defaults. The exact path is platform-specific and is determined by the openOMSI configuration directory.

## 7. Read-only and safety behavior

The Inspector is designed around owned snapshots:

- UI frames do not retain mutable simulation borrows.
- A selected scenery tile is pinned while needed so streaming does not unload the selected object underneath the Inspector.
- Selection is invalidated when the underlying entity is no longer valid.
- GPU readbacks are asynchronous; unavailable data is reported instead of forcing a synchronous wait.
- Inspector commands are queued and processed at the application boundary.

The Inspector does not replace the object editor and should not be treated as a general-purpose runtime modification console.

## 8. Troubleshooting

### The Inspector option is missing from the launcher

Confirm that you are using the openOMSI launcher settings page and that the game binary was updated to a build containing the Inspector integration. The option is under **Settings → General**.

### The Inspector entry is missing from the pause menu

The Inspector must be enabled in the launcher **before starting the session**. Restart the session after changing the launcher setting. Android builds do not expose the desktop Inspector frontend.

### The Inspector windows do not appear

Open the Inspector window’s **Windows** menu and enable the hidden windows. If the layout is corrupted, remove the persisted Inspector layout file from the openOMSI data directory and restart the game.

### Clicking the world does not select anything

Check that Inspector mode is active, that the target is loaded, and that the camera ray intersects the target. Vehicles and loaded scenery are supported; unloaded objects, terrain, splines, and unsupported entity types cannot be selected by the current workflow.

### Game controls stop responding

This is expected while an Inspector window has keyboard or pointer focus. Click outside the Inspector windows or move focus back to the game view to return control to the normal game input path.

### A panel says that controls are unavailable

This indicates a read-only or partially implemented panel, not a failed selection. The current UI displays several diagnostic snapshots while deliberately withholding mutation, watch-editing, and interactive-export operations.

## 9. Current limitations

- Desktop Dear ImGui frontend only; Android is not supported in this rollout.
- Inspector selection is read-only.
- Humans, terrain, and splines are not part of the normal world-selection workflow.
- A scenery selection can pin only one tile at a time.
- Selection is lost when an entity unloads or is replaced.
- Material, render, human, telemetry-watch, editor-mutation, and interactive-export controls are not exposed as live mutation operations.
- The production exporter is separate from the Inspector UI and is invoked with `--export-glb`.
