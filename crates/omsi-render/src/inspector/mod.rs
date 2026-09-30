//! Render pipeline introspection and debugging tools.
//!
//! Provides frame graph visualization, GPU timing queries, selective pass toggles,
//! mesh isolation, and ghost/x-ray modes for visual debugging.

pub mod frame_graph;
pub mod gpu_timing;

pub use frame_graph::{RenderSnapshot, PassInfo, IsolationMode};
pub use gpu_timing::TimestampQueryPool;
