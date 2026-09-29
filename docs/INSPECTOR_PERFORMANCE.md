# Inspector Performance Benchmark Recipe

This document describes how to establish a performance baseline for the visual debug inspector feature before optimization work.

## Overview

The inspector's performance impact should be measured in three scenarios:
1. Inspector mode active with no selection (input routing overhead only)
2. Inspector mode active with vehicle selection (snapshot validation + visual feedback)
3. Inspector mode active with scenery selection (snapshot validation + tile pin + visual feedback)

## Test Scene Setup

### Baseline Scene Configuration

Use a consistent test scene for all measurements:

```bash
openomsi --root "/path/to/OMSI 2" \
  --map maps/Grundorf/global.cfg \
  --bus Vehicles/MAN_SD200/MAN_SD80.bus \
  --spawn 4850,0,3170,90 \
  --time 10:00 \
  --traffic 15 \
  --schedule \
  --passengers \
  --radius 2 \
  --no-menu
```

**Scene characteristics:**
- Player bus: MAN SD200 (moderate poly count)
- AI traffic: 15 vehicles (mix of buses and cars)
- Timetable buses: Active with schedule
- Scenery: 2-tile radius around spawn (Grundorf has dense scenery)
- Time: Mid-day for consistent lighting

### Heavy Load Scene (Optional)

For stress testing:

```bash
openomsi --root "/path/to/OMSI 2" \
  --map maps/Spandau/global.cfg \
  --bus Vehicles/MAN_SD200/MAN_SD80.bus \
  --entry 1 \
  --time 08:00 \
  --traffic 30 \
  --schedule \
  --passengers \
  --radius 3 \
  --no-menu
```

**Scene characteristics:**
- Larger map with denser scenery
- 30 AI vehicles (stress test)
- 3-tile radius (more scenery objects)

## Measurement Points

### 1. Frame Time Impact

Measure frame time (milliseconds per frame) in three states:

**A. Inspector Off (Baseline)**
```bash
# Run for 60 seconds, record frame times
# Target: Establish baseline FPS and frame time
```

**B. Inspector On, No Selection**
```bash
# Press Ctrl+I to activate inspector
# Run for 60 seconds without selecting anything
# Target: Measure input routing overhead only
```

**C. Inspector On, Vehicle Selected**
```bash
# Press Ctrl+I, select player bus
# Run for 60 seconds with selection active
# Target: Measure snapshot validation + visual feedback cost
```

**D. Inspector On, Scenery Selected**
```bash
# Press Ctrl+I, select scenery object
# Run for 60 seconds with scenery pinned
# Target: Measure scenery validation + tile pin + visual feedback cost
```

### 2. Selection Query Time

Measure time to perform raycast and select an entity:

**Vehicle Selection:**
- Click on player bus from various distances (5m, 20m, 50m)
- Click on AI vehicle
- Click on remote vehicle (LAN mode)
- Click on trailer

**Scenery Selection:**
- Click on simple scenery object (< 100 triangles)
- Click on complex scenery object (> 1000 triangles)
- Click on scenery at tile boundary

Expected metrics:
- Selection query time: < 5ms per click
- Distance culling working correctly (no hits beyond reasonable range)

### 3. Snapshot Validation Time

Measure time to validate and build snapshot per frame:

**Vehicle validation:**
- Player vehicle: < 0.1ms per frame
- AI vehicle: < 0.1ms per frame
- Remote vehicle: < 0.2ms per frame (includes HashMap lookup)
- Trailer: < 0.15ms per frame (includes bounds check)

**Scenery validation:**
- Non-editable scenery: < 0.1ms per frame
- Tile state check: < 0.05ms per frame

### 4. Visual Feedback Overhead

Measure rendering cost of inspector overlays:

**Main position marker:**
- Single cyan corona: negligible (< 0.01ms, already part of corona system)

**Optional bounds visualization:**
- 8 corner markers: < 0.05ms (8 additional coronas)

**Optional local axes:**
- 3 axis markers: < 0.04ms (3 additional coronas)

Total overlay cost: < 0.1ms per frame with all options enabled

### 5. Tile Pin Overhead

Measure impact of tile pinning on scenery streaming:

**Without tile pin:**
- Tile unload time: < 50ms (baseline)
- Tile load time: varies by tile complexity

**With tile pin (scenery selected):**
- Tile unload check: + 0.1ms (pin check in filter)
- Tile remains loaded: no unload/reload churn

## Performance Budget (MVP Target)

### Frame Time Budget

**Inspector mode overhead:**
- Input routing: < 0.1ms per frame (negligible impact on gameplay)
- Snapshot validation: < 0.2ms per frame (vehicle or scenery)
- Visual feedback: < 0.1ms per frame (with all overlays enabled)
- **Total overhead: < 0.4ms per frame**

**Impact on frame rate:**
- At 60 FPS (16.67ms budget): < 2.4% overhead
- At 30 FPS (33.33ms budget): < 1.2% overhead

**Acceptance criteria:**
- Inspector mode does not drop FPS below 55 at 60 FPS target
- Inspector mode does not drop FPS below 28 at 30 FPS target
- No frame time spikes > 5ms when selecting entities

### Selection Performance Budget

**Click-to-select latency:**
- Raycast query: < 5ms
- Snapshot construction: < 1ms
- Visual feedback update: < 0.1ms
- **Total latency: < 6ms** (imperceptible to user)

**Tile pin overhead:**
- Pin acquisition: < 1ms
- Unload filter check: < 0.1ms per candidate tile
- Pin release: < 0.5ms

## Profiling Instructions

### Using Rust Profiling Tools

**1. Build with profiling:**
```bash
cargo build --release --features profiling
```

**2. Run with profiler:**
```bash
# Linux/macOS
cargo flamegraph --bin openomsi -- --map maps/Grundorf/global.cfg --no-menu

# Windows
cargo build --release
# Use Windows Performance Analyzer or Visual Studio Profiler
```

**3. Focus areas:**
- `inspector::build_inspector_snapshot`
- `inspector::draw_inspector_overlays`
- `input_script::on_left` (inspector input routing)
- `tiles::unload_tiles` (with and without tile pin)

### Manual Frame Time Measurement

Add instrumentation to measure key operations:

```rust
// In app_events.rs render loop
let inspector_start = std::time::Instant::now();

// Inspector snapshot validation
if let Some(sel) = self.inspector_selection.as_ref() {
    if sel.is_active() {
        let snapshot = build_inspector_snapshot(/* ... */);
        // Log or accumulate: inspector_start.elapsed()
    }
}
```

Log results to CSV for analysis:
```
frame,inspector_active,has_selection,validation_us,render_us,total_frame_ms
1,false,false,0,0,16.5
2,true,false,15,0,16.7
3,true,true,120,45,17.1
...
```

## Expected Results (Baseline)

Based on the implementation, expected performance:

**Negligible overhead:**
- Inspector mode toggle: one-time cost, no per-frame impact
- Input routing: early return when inactive, < 0.05ms when active
- Empty selection: no validation or rendering cost

**Low overhead:**
- Vehicle snapshot validation: simple generation check + HashMap lookup
- Scenery snapshot validation: tile state check (currently stubbed)
- Visual feedback: reuses existing corona system

**Acceptable overhead:**
- Bounds visualization: 8 additional coronas (optional, off by default)
- Local axes: 3 additional coronas (optional, off by default)

**No overhead on gameplay:**
- Inspector mode is mutually exclusive with object editor
- Inspector does not modify simulation state
- No locks held across frames
- All snapshot data is owned (no borrowed references)

## Optimization Opportunities (If Needed)

If performance budget is exceeded:

1. **Lazy snapshot validation:** Only validate when selection changes or periodically (every N frames)
2. **Distance culling:** Skip visual feedback beyond certain camera distance
3. **LOD for overlays:** Reduce overlay detail at distance
4. **Batch corona submission:** Submit all inspector overlays in one call
5. **Cache tile state:** Cache scenery validation results for multiple frames

## Regression Testing

After any optimization:

1. **Re-run all benchmark scenarios**
2. **Verify no functional regressions:**
   - Selection still works for all entity types
   - Visual feedback still appears correctly
   - Tile pin still prevents unload
   - Inspector mode toggle still works
3. **Compare frame time distributions:**
   - Mean frame time should decrease or stay same
   - 99th percentile frame time should not increase
   - No new frame time spikes introduced

## Notes

- Measurements should be taken on representative hardware (mid-range desktop/laptop)
- Disable V-sync for accurate frame time measurement
- Run each scenario multiple times and average results
- Document system specs with measurements (CPU, GPU, RAM)
- Performance characteristics may vary by map and scene complexity
- This is a baseline; production optimization should target user-reported issues
