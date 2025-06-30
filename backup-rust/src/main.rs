use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

mod api;
mod backup;
mod storage;
mod utils;
mod web;

use crate::storage::Storage;
use crate::backup::BackupManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with minimal overhead
    utils::logging::init_tracing();

    // Load configuration
    let config = utils::config::load_config()?;
    
    info!("Starting Backup System v3.0.0");
    info!("CPU cores available: {}", num_cpus::get());
    info!("Memory efficient mode: enabled");

    // Initialize storage layer
    let storage = Storage::new(&config.database_url).await?;
    storage.run_migrations().await?;

    // Check for existing session to restore
    let existing_session = storage.get_latest_session().await?;
    
    // Initialize backup manager with resource limits
    let backup_manager = BackupManager::new(config.clone(), storage.clone());
    
    // Restore previous session if it was Running or Paused
    if let Some(session) = existing_session {
        match session.state {
            backup::BackupState::Running | backup::BackupState::Paused => {
                info!("Restoring previous session: {} (state: {:?})", session.id, session.state);
                let was_running = matches!(session.state, backup::BackupState::Running);
                backup_manager.restore_session(session).await?;
                
                // If it was running, pause it so user can explicitly restart
                if was_running {
                    info!("Previous session was running, setting to paused state");
                    backup_manager.pause().await?;
                }
            }
            backup::BackupState::Stopped => {
                info!("Previous session was stopped, starting fresh");
                // Trigger directory scan without blocking
                let manager_clone = backup_manager.clone();
                tokio::task::spawn_blocking(move || {
                    tokio::runtime::Handle::current().block_on(async {
                        if let Err(e) = manager_clone.scan_directories().await {
                            tracing::warn!("Failed to scan directories: {}", e);
                        }
                    });
                });
            }
        }
    } else {
        info!("No previous session found, starting fresh");
        // Trigger directory scan without blocking
        let manager_clone = backup_manager.clone();
        tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current().block_on(async {
                if let Err(e) = manager_clone.scan_directories().await {
                    tracing::warn!("Failed to scan directories: {}", e);
                }
            });
        });
    }
    
    // Build the API router
    let app = Router::new()
        .nest("/api", api::routes())
        .nest("/ws", web::websocket::routes())
        .merge(web::static_files::routes())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(backup_manager);

    // Start the server with resource-conscious settings
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Server listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server stopped gracefully");
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received");
}