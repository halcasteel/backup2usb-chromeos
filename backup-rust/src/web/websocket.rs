use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::Response,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use tracing::{debug, error};

use crate::backup::BackupManager;

pub fn routes() -> Router<BackupManager> {
    Router::new()
        .route("/", get(websocket_handler))
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(manager): State<BackupManager>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, manager))
}

async fn handle_socket(socket: WebSocket, manager: BackupManager) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to events
    let mut event_rx = manager.subscribe_events();
    
    // Send initial status
    let status = manager.get_status().await;
    let msg = json!({
        "type": "status",
        "data": status
    });
    
    if let Err(e) = sender.send(Message::Text(msg.to_string())).await {
        error!("Failed to send initial status: {}", e);
        return;
    }
    
    // Spawn task to handle events and periodic status updates
    let manager_for_events = manager.clone();
    let mut send_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        
        loop {
            tokio::select! {
                // Handle events
                Ok(event) = event_rx.recv() => {
                    let msg = match event {
                        super::super::backup::manager::Event::StateChanged(_state) => {
                            // Send full status update on state change
                            let status = manager_for_events.get_status().await;
                            json!({
                                "type": "status_update",
                                "payload": status
                            })
                        }
                        super::super::backup::manager::Event::ProgressUpdate { index: _, progress: _ } => {
                            // Send full status for progress updates too
                            let status = manager_for_events.get_status().await;
                            json!({
                                "type": "status_update",
                                "payload": status
                            })
                        }
                        super::super::backup::manager::Event::DirectoryCompleted { index: _ } => {
                            let status = manager_for_events.get_status().await;
                            json!({
                                "type": "status_update",
                                "payload": status
                            })
                        }
                        super::super::backup::manager::Event::Error { message } => {
                            json!({
                                "type": "error",
                                "message": message
                            })
                        }
                    };
                    
                    if sender.send(Message::Text(msg.to_string())).await.is_err() {
                        break;
                    }
                }
                
                // Send periodic status updates
                _ = interval.tick() => {
                    let status = manager_for_events.get_status().await;
                    let msg = json!({
                        "type": "status_update",
                        "payload": status
                    });
                    
                    if sender.send(Message::Text(msg.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    
    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received WebSocket message: {}", text);
                    // Handle client messages if needed
                }
                Message::Close(_) => {
                    debug!("WebSocket closed by client");
                    break;
                }
                _ => {}
            }
        }
    });
    
    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }
    
    debug!("WebSocket connection closed");
}