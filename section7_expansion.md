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
