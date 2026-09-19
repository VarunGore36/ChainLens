use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    #[serde(rename = "block_committed")]
    BlockCommitted {
        block_number: i64,
        tx_count: i32,
        timestamp: String,
    },
    #[serde(rename = "anomaly_detected")]
    AnomalyDetected {
        anomaly_type: String,
        severity: String,
        entity: String,
        description: String,
    },
    #[serde(rename = "mev_detected")]
    MevDetected {
        mev_type: String,
        block_number: i64,
        description: String,
    },
    #[serde(rename = "reorg_detected")]
    ReorgDetected { common_ancestor: i64, depth: i64 },
}

#[derive(Clone, Debug)]
pub struct WsState {
    pub tx: broadcast::Sender<WsEvent>,
}

impl WsState {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn send(&self, event: WsEvent) {
        let _ = self.tx.send(event);
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<super::routes::AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.ws))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<WsState>) {
    let mut rx = state.tx.subscribe();

    let welcome = serde_json::json!({
        "type": "connected",
        "message": "ChainLens WebSocket connected"
    });

    if let Ok(msg) = serde_json::to_string(&welcome) {
        let _ = socket.send(Message::Text(msg.into())).await;
    }

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                if let Ok(json) = serde_json::to_string(&event) {
                    if socket.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Close(_) => break,
                    Message::Ping(data) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
            else => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_event_serialization() {
        let event = WsEvent::BlockCommitted {
            block_number: 12345,
            tx_count: 100,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("block_committed"));
        assert!(json.contains("12345"));
    }

    #[test]
    fn anomaly_event_serialization() {
        let event = WsEvent::AnomalyDetected {
            anomaly_type: "large_transfer".to_string(),
            severity: "high".to_string(),
            entity: "0x1234".to_string(),
            description: "Large transfer detected".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("anomaly_detected"));
        assert!(json.contains("high"));
    }

    #[test]
    fn mev_event_serialization() {
        let event = WsEvent::MevDetected {
            mev_type: "possible_sandwich".to_string(),
            block_number: 18000000,
            description: "Sandwich detected".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("mev_detected"));
    }

    #[test]
    fn reorg_event_serialization() {
        let event = WsEvent::ReorgDetected {
            common_ancestor: 100,
            depth: 3,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("reorg_detected"));
    }

    #[test]
    fn ws_state_creation() {
        let state = WsState::new(100);
        let _ = state.tx.subscribe();
    }
}
