//! Tests for script inspector functionality: variable snapshots, live overrides, and watch expressions.

use omsi_script::inspector::*;
use omsi_script::{Program, State};

fn create_test_program(var_names: &[&str]) -> (Program, State) {
    let mut program = Program::default();
    
    // Declare variables
    for &name in var_names {
        program.declare_var(name);
    }
    
    let state = State::new(&program);
    (program, state)
}

#[test]
fn test_variable_snapshot() {
    let (program, mut state) = create_test_program(&["throttle", "rpm", "speed"]);
    
    // Set initial values
    let throttle_id = program.var("throttle").unwrap();
    let rpm_id = program.var("rpm").unwrap();
    state.set(throttle_id, 0.5);
    state.set(rpm_id, 850.0);
    
    let snapshot = EntityVarSnapshot::new(&program, &state);
    
    assert_eq!(snapshot.float_count, 3);
    assert_eq!(snapshot.string_count, 0);
    assert_eq!(snapshot.vars.len(), 3);
    
    // Check values
    let throttle = snapshot.vars.iter().find(|v| v.name == "throttle").unwrap();
    assert_eq!(throttle.value.as_float(), Some(0.5));
    
    let rpm = snapshot.vars.iter().find(|v| v.name == "rpm").unwrap();
    assert_eq!(rpm.value.as_float(), Some(850.0));
}

#[test]
fn test_script_context_snapshot() {
    let (program, mut state) = create_test_program(&["door_open", "engine_rpm"]);
    
    let door_id = program.var("door_open").unwrap();
    let rpm_id = program.var("engine_rpm").unwrap();
    state.set(door_id, 1.0);
    state.set(rpm_id, 1500.0);
    
    let context = ScriptContext::new(program);
    let snapshot = context.snapshot_variables(&state);
    
    assert_eq!(snapshot.variables.len(), 2);
    assert_eq!(snapshot.variables.get("door_open"), Some(&1.0));
    assert_eq!(snapshot.variables.get("engine_rpm"), Some(&1500.0));
}

#[test]
fn test_live_override_basic() {
    let (program, mut state) = create_test_program(&["throttle"]);
    
    let throttle_id = program.var("throttle").unwrap();
    state.set(throttle_id, 0.5);
    
    let mut context = ScriptContext::new(program);
    
    // Apply override
    context.apply_override(&mut state, "throttle", 0.85).expect("Override failed");
    
    let throttle_id = context.program.var("throttle").unwrap();
    assert_eq!(state.get(throttle_id), 0.85);
    
    // Check that original value was stored
    assert!(context.has_override("throttle"));
    assert_eq!(context.get_original_value("throttle"), Some(0.5));
}

#[test]
fn test_override_rollback() {
    let (program, mut state) = create_test_program(&["throttle", "rpm"]);
    
    let throttle_id = program.var("throttle").unwrap();
    let rpm_id = program.var("rpm").unwrap();
    state.set(throttle_id, 0.5);
    state.set(rpm_id, 850.0);
    
    let mut context = ScriptContext::new(program);
    
    // Apply multiple overrides
    context.apply_override(&mut state, "throttle", 0.85).unwrap();
    context.apply_override(&mut state, "rpm", 2000.0).unwrap();
    
    assert_eq!(context.override_count(), 2);
    
    // Verify overridden values
    let throttle_id = context.program.var("throttle").unwrap();
    let rpm_id = context.program.var("rpm").unwrap();
    assert_eq!(state.get(throttle_id), 0.85);
    assert_eq!(state.get(rpm_id), 2000.0);
    
    // Clear all overrides
    context.clear_overrides(&mut state).unwrap();
    
    // Verify original values restored
    assert_eq!(state.get(throttle_id), 0.5);
    assert_eq!(state.get(rpm_id), 850.0);
    assert_eq!(context.override_count(), 0);
}

#[test]
fn test_override_nonexistent_variable() {
    let (program, mut state) = create_test_program(&["throttle"]);
    
    let mut context = ScriptContext::new(program);
    
    // Try to override non-existent variable
    let result = context.apply_override(&mut state, "nonexistent", 1.0);
    assert!(result.is_err());
}

#[test]
fn test_watch_expression_variable() {
    let mut watch = WatchExpression::new_variable(0, "speed".to_string());
    
    let mut snapshot = ScriptSnapshot::default();
    snapshot.variables.insert("speed".to_string(), 10.0);
    
    watch.update(&snapshot);
    assert_eq!(watch.current_value(), Some(10.0));
    assert_eq!(watch.previous_value(), None);
    
    snapshot.variables.insert("speed".to_string(), 15.0);
    watch.update(&snapshot);
    assert_eq!(watch.current_value(), Some(15.0));
    assert_eq!(watch.previous_value(), Some(10.0));
}

#[test]
fn test_watch_expression_computed() {
    let mut watch = WatchExpression::new_computed(0, "speed".to_string(), "* 3.6")
        .expect("Failed to create computed watch");
    
    let mut snapshot = ScriptSnapshot::default();
    snapshot.variables.insert("speed".to_string(), 10.0);
    
    watch.update(&snapshot);
    assert_eq!(watch.current_value(), Some(36.0)); // 10.0 m/s * 3.6 = 36.0 km/h
}

#[test]
fn test_watch_change_indicator() {
    let mut watch = WatchExpression::new_variable(0, "rpm".to_string());
    
    let mut snapshot = ScriptSnapshot::default();
    snapshot.variables.insert("rpm".to_string(), 1000.0);
    watch.update(&snapshot);
    
    assert_eq!(watch.change_indicator(), ChangeIndicator::Unchanged);
    
    snapshot.variables.insert("rpm".to_string(), 1500.0);
    watch.update(&snapshot);
    assert_eq!(watch.change_indicator(), ChangeIndicator::Increased);
    
    snapshot.variables.insert("rpm".to_string(), 1200.0);
    watch.update(&snapshot);
    assert_eq!(watch.change_indicator(), ChangeIndicator::Decreased);
}

#[test]
fn test_watch_sparkline_data() {
    let mut watch = WatchExpression::new_variable(0, "value".to_string());
    
    let mut snapshot = ScriptSnapshot::default();
    for i in 0..10 {
        snapshot.variables.insert("value".to_string(), i as f32);
        watch.update(&snapshot);
    }
    
    let sparkline = watch.sparkline_data(5);
    assert_eq!(sparkline.len(), 5);
    assert_eq!(sparkline, vec![5.0, 6.0, 7.0, 8.0, 9.0]);
}

#[test]
fn test_watch_statistics() {
    let mut watch = WatchExpression::new_variable(0, "value".to_string());
    
    let mut snapshot = ScriptSnapshot::default();
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    for &val in &values {
        snapshot.variables.insert("value".to_string(), val);
        watch.update(&snapshot);
    }
    
    let stats = watch.statistics();
    assert_eq!(stats.min, 1.0);
    assert_eq!(stats.max, 5.0);
    assert_eq!(stats.mean, 3.0);
}

#[test]
fn test_watch_table_add_remove() {
    let mut table = WatchTable::new();
    
    let id1 = table.add_variable("speed".to_string()).unwrap();
    let id2 = table.add_variable("rpm".to_string()).unwrap();
    
    assert_eq!(table.count(), 2);
    
    table.remove(id1);
    assert_eq!(table.count(), 1);
    
    table.remove(id2);
    assert_eq!(table.count(), 0);
}

#[test]
fn test_watch_table_max_capacity() {
    let mut table = WatchTable::new();
    
    // Add 16 watches (maximum)
    for i in 0..16 {
        table.add_variable(format!("var{}", i)).unwrap();
    }
    
    assert_eq!(table.count(), 16);
    
    // Try to add 17th watch
    let result = table.add_variable("overflow".to_string());
    assert!(result.is_err());
}

#[test]
fn test_watch_table_update_all() {
    let mut table = WatchTable::new();
    table.add_variable("speed".to_string()).unwrap();
    table.add_variable("rpm".to_string()).unwrap();
    
    let mut snapshot = ScriptSnapshot::default();
    snapshot.variables.insert("speed".to_string(), 25.0);
    snapshot.variables.insert("rpm".to_string(), 1800.0);
    
    table.update_all(&snapshot);
    
    assert_eq!(table.expressions()[0].current_value(), Some(25.0));
    assert_eq!(table.expressions()[1].current_value(), Some(1800.0));
}

#[test]
fn test_watch_table_performance() {
    let mut table = WatchTable::new();
    
    // Add 16 watches
    for i in 0..16 {
        table.add_variable(format!("var{}", i)).unwrap();
    }
    
    // Estimate should be under 0.3ms for 16 watches
    let overhead = table.update_overhead_us();
    assert!(overhead < 300.0, "Update overhead {} exceeds 300µs", overhead);
}

#[test]
fn test_sparkline_history_ring_buffer() {
    let mut watch = WatchExpression::new_variable(0, "value".to_string());
    
    let mut snapshot = ScriptSnapshot::default();
    
    // Fill beyond max history (120 samples)
    for i in 0..150 {
        snapshot.variables.insert("value".to_string(), i as f32);
        watch.update(&snapshot);
    }
    
    // Should only keep last 120
    let sparkline = watch.sparkline_data(120);
    assert_eq!(sparkline.len(), 120);
    assert_eq!(sparkline[0], 30.0); // First value should be 150-120 = 30
    assert_eq!(sparkline[119], 149.0);
}

#[test]
fn test_computed_expression_operations() {
    // Test multiply
    let mut watch_mul = WatchExpression::new_computed(0, "speed".to_string(), "* 3.6").unwrap();
    let mut snapshot = ScriptSnapshot::default();
    snapshot.variables.insert("speed".to_string(), 10.0);
    watch_mul.update(&snapshot);
    assert_eq!(watch_mul.current_value(), Some(36.0));
    
    // Test add
    let mut watch_add = WatchExpression::new_computed(1, "temp".to_string(), "+ 273.15").unwrap();
    snapshot.variables.insert("temp".to_string(), 20.0);
    watch_add.update(&snapshot);
    assert_eq!(watch_add.current_value(), Some(293.15));
    
    // Test divide
    let mut watch_div = WatchExpression::new_computed(2, "pressure".to_string(), "/ 2.0").unwrap();
    snapshot.variables.insert("pressure".to_string(), 10.0);
    watch_div.update(&snapshot);
    assert_eq!(watch_div.current_value(), Some(5.0));
}

#[test]
fn test_computed_expression_invalid() {
    // Division by zero should be rejected
    let result = WatchExpression::new_computed(0, "value".to_string(), "/ 0.0");
    assert!(result.is_err());
    
    // Invalid operation
    let result = WatchExpression::new_computed(1, "value".to_string(), "% 2.0");
    assert!(result.is_err());
}
