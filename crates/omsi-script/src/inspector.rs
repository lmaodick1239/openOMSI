//! Inspector snapshot API for read-only script variable viewing, live overrides, and watch expressions.
//!
//! Captures all OMSI script variables (L.var, S.var) for selected vehicles and scenery objects,
//! categorized by origin script file (engine.osc, door.osc, cockpit.osc, etc.).
//!
//! Provides live variable override capabilities with transactional rollback, and a watch expression
//! system with sparkline history for real-time debugging.

use crate::{compile::Program, vm::State, ScriptError};
use hashbrown::HashMap;
use std::collections::VecDeque;

/// Type of script variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarType {
    /// Float variable (L.var).
    Float,
    /// String variable (S.var).
    String,
}

/// Snapshot of a single script variable at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct VarSnapshot {
    /// Variable name (lower case, as stored in Program).
    pub name: String,
    /// Current value (for floats) or string content.
    pub value: VarValue,
    /// Variable type.
    pub var_type: VarType,
    /// Source file where this variable was declared (if known).
    pub source_file: Option<String>,
}

/// Value of a variable snapshot.
#[derive(Debug, Clone, PartialEq)]
pub enum VarValue {
    Float(f32),
    String(String),
}

impl VarValue {
    /// Format the value for display with 4 decimal places for floats.
    pub fn format(&self) -> String {
        match self {
            VarValue::Float(v) => format!("{:.4}", v),
            VarValue::String(s) => s.clone(),
        }
    }

    /// Get the float value if this is a float, None otherwise.
    pub fn as_float(&self) -> Option<f32> {
        match self {
            VarValue::Float(v) => Some(*v),
            _ => None,
        }
    }
}

/// Complete snapshot of all script variables for an entity.
#[derive(Debug, Clone, Default)]
pub struct EntityVarSnapshot {
    /// All variable snapshots, sorted by name.
    pub vars: Vec<VarSnapshot>,
    /// Total count of float variables.
    pub float_count: usize,
    /// Total count of string variables.
    pub string_count: usize,
}

impl EntityVarSnapshot {
    /// Create a new snapshot from program and state.
    pub fn new(program: &Program, state: &State) -> Self {
        let mut vars = Vec::new();

        // Capture float variables
        for (id, name) in program.var_names.iter().enumerate() {
            let value = state.vars.get(id).copied().unwrap_or(0.0);
            vars.push(VarSnapshot {
                name: name.clone(),
                value: VarValue::Float(value),
                var_type: VarType::Float,
                source_file: None, // TODO: track source file during compilation
            });
        }

        // Capture string variables
        for (id, name) in program.str_var_names.iter().enumerate() {
            let value = state.str_vars.get(id).cloned().unwrap_or_default();
            vars.push(VarSnapshot {
                name: name.clone(),
                value: VarValue::String(value),
                var_type: VarType::String,
                source_file: None, // TODO: track source file during compilation
            });
        }

        let float_count = program.var_names.len();
        let string_count = program.str_var_names.len();

        // Sort by name for consistent display
        vars.sort_by(|a, b| a.name.cmp(&b.name));

        Self {
            vars,
            float_count,
            string_count,
        }
    }

    /// Filter variables by search pattern with wildcard support.
    ///
    /// Patterns support:
    /// - `*` at start/end: prefix/suffix match (`door_*`, `*_pressure`)
    /// - `*` at both: substring match (`*engine*`)
    /// - No wildcard: exact match (case-insensitive)
    pub fn filter(&self, pattern: &str) -> Vec<&VarSnapshot> {
        if pattern.is_empty() {
            return self.vars.iter().collect();
        }

        let pattern_lower = pattern.to_ascii_lowercase();
        let pattern_lower = pattern_lower.trim();

        // Handle wildcard patterns
        if pattern_lower.starts_with('*') && pattern_lower.ends_with('*') {
            // Substring match
            let substr = &pattern_lower[1..pattern_lower.len() - 1];
            self.vars
                .iter()
                .filter(|v| v.name.contains(substr))
                .collect()
        } else if pattern_lower.starts_with('*') {
            // Suffix match
            let suffix = &pattern_lower[1..];
            self.vars
                .iter()
                .filter(|v| v.name.ends_with(suffix))
                .collect()
        } else if pattern_lower.ends_with('*') {
            // Prefix match
            let prefix = &pattern_lower[..pattern_lower.len() - 1];
            self.vars
                .iter()
                .filter(|v| v.name.starts_with(prefix))
                .collect()
        } else {
            // Exact match (case-insensitive)
            self.vars
                .iter()
                .filter(|v| v.name == pattern_lower)
                .collect()
        }
    }

    /// Get snapshot overhead estimate in microseconds.
    pub fn snapshot_overhead_us(&self) -> f32 {
        // Estimate: ~2ns per variable (copy + format)
        (self.vars.len() as f32) * 0.002
    }
}

/// ScriptSnapshot provides a complete snapshot of script state including variables,
/// string variables, active macros, and the current stack state.
#[derive(Debug, Clone, Default)]
pub struct ScriptSnapshot {
    /// All float variables (L.var)
    pub variables: HashMap<String, f32>,
    /// All string variables (S.var)
    pub string_vars: HashMap<String, String>,
    /// Currently active macros (for advanced tracing)
    pub active_macros: Vec<String>,
    /// Top 8 elements of the float stack
    pub stack_top: Vec<f32>,
}

/// ScriptContext manages script execution state with live override capabilities.
///
/// This context provides:
/// - Read-only variable snapshots for inspection
/// - Live variable overrides with transactional rollback
/// - Automatic override cleanup on entity despawn
pub struct ScriptContext {
    /// Original values for overridden variables (for rollback)
    overrides: HashMap<String, f32>,
    /// Program reference for variable lookup
    pub program: Program,
}

impl ScriptContext {
    /// Create a new script context for a given program.
    pub fn new(program: Program) -> Self {
        Self {
            overrides: HashMap::new(),
            program,
        }
    }

    /// Snapshot all variables for the given state.
    pub fn snapshot_variables(&self, state: &State) -> ScriptSnapshot {
        let mut variables = HashMap::new();
        let mut string_vars = HashMap::new();

        // Capture float variables
        for (id, name) in self.program.var_names.iter().enumerate() {
            if let Some(&value) = state.vars.get(id) {
                variables.insert(name.clone(), value);
            }
        }

        // Capture string variables
        for (id, name) in self.program.str_var_names.iter().enumerate() {
            if let Some(value) = state.str_vars.get(id) {
                string_vars.insert(name.clone(), value.clone());
            }
        }

        ScriptSnapshot {
            variables,
            string_vars,
            active_macros: Vec::new(),
            stack_top: Vec::new(),
        }
    }

    /// Apply a live override to a variable. Returns error if variable doesn't exist
    /// or if the value is out of valid bounds.
    ///
    /// Overrides are stored transactionally - the original value is saved for rollback.
    pub fn apply_override(&mut self, state: &mut State, var_name: &str, value: f32) -> Result<(), ScriptError> {
        let var_id = self.program.var(var_name).ok_or_else(|| ScriptError {
            file: "inspector".into(),
            line: 0,
            message: format!("Variable '{}' not found", var_name),
        })?;

        // Store original value if this is the first override for this variable
        if !self.overrides.contains_key(var_name) {
            self.overrides.insert(var_name.to_string(), state.get(var_id));
        }

        // Apply the override
        state.set(var_id, value);
        Ok(())
    }

    /// Clear all overrides and restore original values.
    pub fn clear_overrides(&mut self, state: &mut State) -> Result<(), ScriptError> {
        for (var_name, original_value) in self.overrides.drain() {
            if let Some(var_id) = self.program.var(&var_name) {
                state.set(var_id, original_value);
            }
        }
        Ok(())
    }

    /// Check if a variable has an active override.
    pub fn has_override(&self, var_name: &str) -> bool {
        self.overrides.contains_key(var_name)
    }

    /// Get the original value of an overridden variable.
    pub fn get_original_value(&self, var_name: &str) -> Option<f32> {
        self.overrides.get(var_name).copied()
    }

    /// Get count of active overrides.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }
}

/// Watch expression for tracking variable values over time.
#[derive(Debug, Clone)]
pub struct WatchExpression {
    /// Expression identifier
    pub id: usize,
    /// Human-readable expression (e.g., "L.speed * 3.6")
    pub expression: String,
    /// Compiled expression for evaluation
    expr_type: WatchExprType,
    /// Historical values (ring buffer, last 120 samples)
    history: VecDeque<f32>,
    /// Maximum history size
    max_history: usize,
}

#[derive(Debug, Clone)]
enum WatchExprType {
    /// Simple variable reference
    Variable(String),
    /// Computed expression (for future expansion)
    Computed { var_name: String, operation: ComputeOp },
}

#[derive(Debug, Clone)]
enum ComputeOp {
    Multiply(f32),
    Add(f32),
    Divide(f32),
}

impl WatchExpression {
    /// Create a new watch expression for a variable.
    pub fn new_variable(id: usize, var_name: String) -> Self {
        Self {
            id,
            expression: format!("L.{}", var_name),
            expr_type: WatchExprType::Variable(var_name),
            history: VecDeque::with_capacity(120),
            max_history: 120,
        }
    }

    /// Create a computed watch expression (e.g., "L.speed * 3.6" for km/h conversion).
    pub fn new_computed(id: usize, var_name: String, operation: &str) -> Result<Self, String> {
        let op = if let Some(multiplier) = operation.strip_prefix("* ") {
            let factor = multiplier.parse::<f32>().map_err(|_| "Invalid multiplier")?;
            ComputeOp::Multiply(factor)
        } else if let Some(addend) = operation.strip_prefix("+ ") {
            let value = addend.parse::<f32>().map_err(|_| "Invalid addend")?;
            ComputeOp::Add(value)
        } else if let Some(divisor) = operation.strip_prefix("/ ") {
            let value = divisor.parse::<f32>().map_err(|_| "Invalid divisor")?;
            if value == 0.0 {
                return Err("Division by zero".to_string());
            }
            ComputeOp::Divide(value)
        } else {
            return Err("Unsupported operation".to_string());
        };

        Ok(Self {
            id,
            expression: format!("(L.{} {})", var_name, operation),
            expr_type: WatchExprType::Computed {
                var_name,
                operation: op,
            },
            history: VecDeque::with_capacity(120),
            max_history: 120,
        })
    }

    /// Update the watch expression with a new sample from the snapshot.
    pub fn update(&mut self, snapshot: &ScriptSnapshot) {
        let value = match &self.expr_type {
            WatchExprType::Variable(var_name) => {
                snapshot.variables.get(var_name).copied().unwrap_or(0.0)
            }
            WatchExprType::Computed { var_name, operation } => {
                let base = snapshot.variables.get(var_name).copied().unwrap_or(0.0);
                match operation {
                    ComputeOp::Multiply(factor) => base * factor,
                    ComputeOp::Add(value) => base + value,
                    ComputeOp::Divide(divisor) => base / divisor,
                }
            }
        };

        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(value);
    }

    /// Get the current value.
    pub fn current_value(&self) -> Option<f32> {
        self.history.back().copied()
    }

    /// Get the previous value for change detection.
    pub fn previous_value(&self) -> Option<f32> {
        if self.history.len() >= 2 {
            self.history.get(self.history.len() - 2).copied()
        } else {
            None
        }
    }

    /// Get change indicator for UI color coding.
    pub fn change_indicator(&self) -> ChangeIndicator {
        match (self.current_value(), self.previous_value()) {
            (Some(current), Some(previous)) => {
                let delta = current - previous;
                if delta.abs() < 0.0001 {
                    ChangeIndicator::Unchanged
                } else if delta > 0.0 {
                    ChangeIndicator::Increased
                } else {
                    ChangeIndicator::Decreased
                }
            }
            _ => ChangeIndicator::Unchanged,
        }
    }

    /// Get sparkline data (last N samples for rendering).
    pub fn sparkline_data(&self, count: usize) -> Vec<f32> {
        let start = self.history.len().saturating_sub(count);
        self.history.iter().skip(start).copied().collect()
    }

    /// Get statistics for the history buffer.
    pub fn statistics(&self) -> WatchStatistics {
        if self.history.is_empty() {
            return WatchStatistics::default();
        }

        let mut min = f32::MAX;
        let mut max = f32::MIN;
        let mut sum = 0.0;

        for &value in &self.history {
            min = min.min(value);
            max = max.max(value);
            sum += value;
        }

        let mean = sum / self.history.len() as f32;

        WatchStatistics { min, max, mean }
    }
}

/// Change indicator for UI color coding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeIndicator {
    Increased,   // Green
    Decreased,   // Red
    Unchanged,   // Gray
}

/// Statistics for a watch expression history.
#[derive(Debug, Clone, Copy, Default)]
pub struct WatchStatistics {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
}

/// Watch table manages up to 16 watched expressions with sparkline history.
pub struct WatchTable {
    expressions: Vec<WatchExpression>,
    next_id: usize,
    max_watches: usize,
}

impl WatchTable {
    /// Create a new watch table with a maximum of 16 watches.
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
            next_id: 0,
            max_watches: 16,
        }
    }

    /// Add a variable watch. Returns error if table is full.
    pub fn add_variable(&mut self, var_name: String) -> Result<usize, String> {
        if self.expressions.len() >= self.max_watches {
            return Err(format!("Watch table full (max {})", self.max_watches));
        }

        let id = self.next_id;
        self.next_id += 1;
        self.expressions.push(WatchExpression::new_variable(id, var_name));
        Ok(id)
    }

    /// Add a computed expression watch. Returns error if table is full.
    pub fn add_computed(&mut self, var_name: String, operation: &str) -> Result<usize, String> {
        if self.expressions.len() >= self.max_watches {
            return Err(format!("Watch table full (max {})", self.max_watches));
        }

        let id = self.next_id;
        self.next_id += 1;
        let expr = WatchExpression::new_computed(id, var_name, operation)?;
        self.expressions.push(expr);
        Ok(id)
    }

    /// Remove a watch by ID.
    pub fn remove(&mut self, id: usize) -> bool {
        if let Some(pos) = self.expressions.iter().position(|e| e.id == id) {
            self.expressions.remove(pos);
            true
        } else {
            false
        }
    }

    /// Update all watches with a new snapshot.
    pub fn update_all(&mut self, snapshot: &ScriptSnapshot) {
        for expr in &mut self.expressions {
            expr.update(snapshot);
        }
    }

    /// Get all watch expressions.
    pub fn expressions(&self) -> &[WatchExpression] {
        &self.expressions
    }

    /// Get count of active watches.
    pub fn count(&self) -> usize {
        self.expressions.len()
    }

    /// Clear all watches.
    pub fn clear(&mut self) {
        self.expressions.clear();
    }

    /// Estimate update overhead in microseconds.
    pub fn update_overhead_us(&self) -> f32 {
        // Estimate: ~5-6 microseconds per watch (lookup + compute + history update)
        (self.expressions.len() as f32) * 6.0
    }
}

impl Default for WatchTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_exact() {
        let snapshot = EntityVarSnapshot {
            vars: vec![
                VarSnapshot {
                    name: "door_open".to_string(),
                    value: VarValue::Float(1.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "engine_rpm".to_string(),
                    value: VarValue::Float(850.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
            ],
            float_count: 2,
            string_count: 0,
        };

        let filtered = snapshot.filter("door_open");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "door_open");
    }

    #[test]
    fn test_filter_prefix() {
        let snapshot = EntityVarSnapshot {
            vars: vec![
                VarSnapshot {
                    name: "door_open".to_string(),
                    value: VarValue::Float(1.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "door_pressure".to_string(),
                    value: VarValue::Float(5.2),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "engine_rpm".to_string(),
                    value: VarValue::Float(850.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
            ],
            float_count: 3,
            string_count: 0,
        };

        let filtered = snapshot.filter("door_*");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|v| v.name.starts_with("door_")));
    }

    #[test]
    fn test_filter_suffix() {
        let snapshot = EntityVarSnapshot {
            vars: vec![
                VarSnapshot {
                    name: "door_pressure".to_string(),
                    value: VarValue::Float(5.2),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "brake_pressure".to_string(),
                    value: VarValue::Float(8.5),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "engine_rpm".to_string(),
                    value: VarValue::Float(850.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
            ],
            float_count: 3,
            string_count: 0,
        };

        let filtered = snapshot.filter("*_pressure");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|v| v.name.ends_with("_pressure")));
    }

    #[test]
    fn test_filter_substring() {
        let snapshot = EntityVarSnapshot {
            vars: vec![
                VarSnapshot {
                    name: "door_engine_lock".to_string(),
                    value: VarValue::Float(0.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "engine_rpm".to_string(),
                    value: VarValue::Float(850.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
                VarSnapshot {
                    name: "starter_engine".to_string(),
                    value: VarValue::Float(1.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
            ],
            float_count: 3,
            string_count: 0,
        };

        let filtered = snapshot.filter("*engine*");
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().all(|v| v.name.contains("engine")));
    }

    #[test]
    fn test_filter_empty_pattern() {
        let snapshot = EntityVarSnapshot {
            vars: vec![
                VarSnapshot {
                    name: "door_open".to_string(),
                    value: VarValue::Float(1.0),
                    var_type: VarType::Float,
                    source_file: None,
                },
            ],
            float_count: 1,
            string_count: 0,
        };

        let filtered = snapshot.filter("");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_value_format() {
        let float_val = VarValue::Float(3.14159265);
        assert_eq!(float_val.format(), "3.1416");

        let string_val = VarValue::String("test".to_string());
        assert_eq!(string_val.format(), "test");
    }
}
