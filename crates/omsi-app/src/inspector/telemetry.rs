//! Real-time WebSocket telemetry server for remote diagnostics.
//!
//! Optional diagnostic server (`--telemetry-port 9002`) that broadcasts live JSON packets
//! at 30Hz containing vehicle velocity, engine RPM, gear status, door states, selected
//! entity properties, and watch table values.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::Arc;

/// WebSocket client connection limit.
#[allow(dead_code)]
const MAX_CLIENTS: usize = 4;

/// Telemetry broadcast rate in Hz.
#[allow(dead_code)]
const TELEMETRY_RATE_HZ: u32 = 30;

/// Inspector snapshot for telemetry broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorSnapshot {
    /// Timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
    /// Vehicle telemetry data (if vehicle selected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<VehicleTelemetry>,
    /// Selected entity properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionTelemetry>,
    /// Watch table values.
    #[serde(default)]
    pub watch_values: Vec<WatchValue>,
}

/// Vehicle-specific telemetry data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleTelemetry {
    /// Velocity in m/s.
    pub velocity_ms: f32,
    /// Engine RPM.
    pub engine_rpm: f32,
    /// Current gear (0 = neutral, -1 = reverse).
    pub gear: i32,
    /// Door states (0 = closed, 1 = open).
    pub door_states: Vec<u8>,
    /// Throttle position [0.0, 1.0].
    pub throttle: f32,
    /// Brake pressure [0.0, 1.0].
    pub brake: f32,
}

/// Selected entity telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionTelemetry {
    /// Entity name.
    pub name: String,
    /// World position [x, y, z].
    pub position: [f32; 3],
    /// World rotation (yaw, pitch, roll) in degrees.
    pub rotation: [f32; 3],
}

/// Watch table value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchValue {
    /// Expression (e.g., "L.throttle").
    pub expression: String,
    /// Current value.
    pub value: f32,
    /// Optional label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// WebSocket telemetry server state.
pub struct TelemetryServer {
    #[allow(dead_code)]
    port: u16,
    clients: Arc<RwLock<Vec<WebSocketClient>>>,
    running: Arc<RwLock<bool>>,
}

/// WebSocket client connection (stub).
#[allow(dead_code)]
#[derive(Debug)]
struct WebSocketClient {
    id: u32,
    connected_at: std::time::SystemTime,
}

impl TelemetryServer {
    /// Start the telemetry server on the specified port.
    ///
    /// This is a stub implementation. Full WebSocket server requires `tokio-tungstenite`
    /// or similar async WebSocket library, running on a separate async runtime thread
    /// to avoid blocking the simulation.
    pub fn start(port: u16) -> Result<Self, io::Error> {
        log::info!("Starting telemetry server on port {}", port);

        // TODO: Implement full WebSocket server with tokio-tungstenite
        // - Spawn async runtime thread
        // - Accept WebSocket connections (max MAX_CLIENTS)
        // - Handle handshake and upgrade
        // - Maintain client registry

        let server = Self {
            port,
            clients: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(true)),
        };

        log::warn!("Telemetry server stub started (WebSocket implementation pending)");
        Ok(server)
    }

    /// Broadcast telemetry snapshot to all connected clients.
    ///
    /// This is called at TELEMETRY_RATE_HZ from the simulation thread.
    /// Must be non-blocking and lock-free.
    pub fn broadcast(&mut self, snapshot: &InspectorSnapshot) {
        let clients = self.clients.read();
        if clients.is_empty() {
            return;
        }

        // TODO: Serialize snapshot to JSON and send to all clients
        // - Use try_lock or async channel to avoid blocking
        // - Remove disconnected clients
        // - Rate limit per client

        let json = match serde_json::to_string(snapshot) {
            Ok(j) => j,
            Err(e) => {
                log::error!("Failed to serialize telemetry: {}", e);
                return;
            }
        };

        log::trace!(
            "Broadcasting telemetry to {} clients ({} bytes)",
            clients.len(),
            json.len()
        );
    }

    /// Get current client count.
    pub fn client_count(&self) -> usize {
        self.clients.read().len()
    }

    /// Check if server is running.
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Stop the telemetry server.
    pub fn stop(&mut self) {
        log::info!("Stopping telemetry server");
        *self.running.write() = false;

        // TODO: Close all WebSocket connections gracefully
        // - Send close frame to each client
        // - Wait for acknowledgment (with timeout)
        // - Shutdown async runtime thread
    }
}

impl Drop for TelemetryServer {
    fn drop(&mut self) {
        if self.is_running() {
            self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = InspectorSnapshot {
            timestamp_ms: 1234567890,
            vehicle: Some(VehicleTelemetry {
                velocity_ms: 15.5,
                engine_rpm: 1500.0,
                gear: 3,
                door_states: vec![0, 1, 0, 0],
                throttle: 0.65,
                brake: 0.0,
            }),
            selection: Some(SelectionTelemetry {
                name: "cockpit_speedo.o3d".to_string(),
                position: [100.0, 50.0, 2.5],
                rotation: [180.0, 0.0, 0.0],
            }),
            watch_values: vec![WatchValue {
                expression: "L.throttle".to_string(),
                value: 0.65,
                label: Some("Throttle".to_string()),
            }],
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("velocity_ms"));
        assert!(json.contains("cockpit_speedo"));

        let decoded: InspectorSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.timestamp_ms, 1234567890);
        assert!(decoded.vehicle.is_some());
    }

    #[test]
    fn test_telemetry_rate() {
        let period_ms = 1000 / TELEMETRY_RATE_HZ;
        assert_eq!(period_ms, 33); // ~30 Hz
    }
}
