use anyhow::{Context, Result};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use theater::client::TheaterConnection;
use theater::events::EventData;
use theater::id::TheaterId;
use theater::messages::ActorResult;
use theater::theater_server::{ManagementCommand, ManagementResponse};
use theater::ChainEvent;

// Default Theater server address
const DEFAULT_SERVER_ADDRESS: &str = "127.0.0.1:9000";

// The actor manifest location
const COMMIT_ACTOR_MANIFEST: &str =
    "/Users/colinrozzi/work/actor-registry/commit-actor/manifest.toml";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get the current directory
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    // Get the API key from environment variables
    let api_key = env::var("GOOGLE_GEMINI_API_KEY")
        .context("GOOGLE_GEMINI_API_KEY environment variable not set")?;

    // Get the server address from environment variables or use default
    let server_address: SocketAddr = env::var("THEATER_SERVER_ADDRESS")
        .unwrap_or_else(|_| DEFAULT_SERVER_ADDRESS.to_string())
        .parse()
        .context("Invalid server address format")?;

    println!("🎭 Theater Commit");
    println!("Repository: {}", current_dir.display());
    println!("Connecting to Theater server at {}...", server_address);

    // Connect to the Theater server
    let mut connection = connect_to_server(server_address)
        .await
        .context("Failed to connect to Theater server")?;

    println!("Connected to Theater server");

    // Run the commit process
    run_commit(&mut connection, current_dir, api_key).await?;

    Ok(())
}

/// Connect to the Theater server
async fn connect_to_server(address: SocketAddr) -> Result<TheaterConnection> {
    let mut connection = TheaterConnection::new(address);
    connection.connect().await?;
    Ok(connection)
}

/// Run the commit process
async fn run_commit(
    connection: &mut TheaterConnection,
    repo_path: PathBuf,
    api_key: String,
) -> Result<()> {
    println!("Starting commit process...");

    // Prepare the initial state for the commit-actor
    let initial_state = serde_json::json!({
        "repository_path": repo_path.to_string_lossy(),
        "api_key": api_key
    });

    // Read the commit-actor manifest
    let manifest = std::fs::read_to_string(COMMIT_ACTOR_MANIFEST)
        .context("Failed to read commit-actor manifest")?;

    // Convert initial state to bytes
    let initial_state_bytes =
        serde_json::to_vec(&initial_state).context("Failed to serialize initial state")?;

    println!("Starting commit-actor...");

    // Start the commit-actor
    connection
        .send(ManagementCommand::StartActor {
            manifest,
            initial_state: Some(initial_state_bytes),
            parent: true,
            subscribe: false, // Subscribe to get updates
        })
        .await
        .context("Failed to send StartActor command")?;

    // Receive actor started response
    let response = connection
        .receive()
        .await
        .context("Failed to receive response")?;

    match response {
        ManagementResponse::ActorStarted { id } => {
            println!("✅ Commit actor started!");
            println!("Actor ID: {}", id);

            // Wait for actor to complete
            monitor_actor_progress(connection, id).await?;
        }
        ManagementResponse::Error { error } => {
            println!("❌ Error starting commit actor: {:?}", error);
        }
        _ => {
            println!("❓ Unexpected response: {:?}", response);
        }
    }

    Ok(())
}

/// Monitor the progress of the commit actor
async fn monitor_actor_progress(
    connection: &mut TheaterConnection,
    actor_id: TheaterId,
) -> Result<()> {
    println!("📝 Committing changes...");

    // We're already subscribed from the StartActor command,
    // so now we just listen for events
    let mut completed = false;

    while !completed {
        match connection.receive().await {
            Ok(ManagementResponse::ActorResult(res)) => {
                completed = true;
                match res {
                    ActorResult::Success(data) => {
                        println!("success!")
                    }
                    ActorResult::Error(err) => {
                        println!("error: {}", err)
                    }
                }
            }
            Ok(other) => {
                // Uncomment for debugging
                // println!("Other response: {:?}", other);
            }
            Err(e) => {
                println!("❌ Error receiving events: {}", e);
                completed = true;
            }
        }
    }

    Ok(())
}

// Helper functions for chain events

/// Extract the shutdown reason from a chain event, if present
fn event_shutdown_data(event: &ChainEvent) -> Option<Vec<u8>> {
    match serde_json::from_slice(&event.data) {
        Ok(EventData::Runtime(theater::events::runtime::RuntimeEventData::ShutdownCall {
            data,
        })) => data,
        _ => None,
    }
}

/// Extract the log message from a chain event, if present
fn event_log(event: &ChainEvent) -> Option<String> {
    match serde_json::from_slice(&event.data) {
        Ok(EventData::Runtime(theater::events::runtime::RuntimeEventData::Log {
            message, ..
        })) => Some(message),
        _ => None,
    }
}
