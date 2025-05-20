use anyhow::{Context, Result};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use theater::client::TheaterConnection;
use theater::messages::ActorResult;
use theater::messages::{ChildError, ChildResult};
use theater::theater_server::{ManagementCommand, ManagementResponse};

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

    println!("✅ Connected to Theater server");

    // Display a visual separator
    println!("\n{}", "-".repeat(50));

    // Run the commit process
    run_commit(&mut connection, current_dir, api_key).await?;

    // Display a visual separator at the end
    println!("{}", "-".repeat(50));

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
    println!("⏳ Starting commit process...");

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

    println!("🔍 Checking repository: {}", repo_path.display());

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

    println!("🤖 Starting Theater commit actor...");

    loop {
        tokio::select! {
            Ok(msg) = connection.receive() => {
                match msg {
                    ManagementResponse::ActorStarted { id } => {
                        println!("✅ Commit actor started! (ID: {})", id);
                        println!("📁 Analyzing changes in repository...");
                        println!("\n⏳ Working: This may take a moment...");
                    },
                    ManagementResponse::ActorResult(result) => {
                        match result {
                            ActorResult::Success(ChildResult { actor_id, result }) => {
                                if let Some(bytes) = result {
                                    if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                                        // First check if we're using the new status_msg format
                                        if let Some(status_msg) = data.get("status_msg").and_then(|s| s.as_str()) {
                                            println!("\n✅ Commit operation completed");

                                            // Parse the pipe-delimited format
                                            let mut success = false;
                                            let mut message = None;
                                            let mut hash = None;
                                            let mut commit_msg = None;
                                            let mut files = 0;
                                            let mut ins = 0;
                                            let mut dels = 0;

                                            // Parse each field
                                            for field in status_msg.split("|") {
                                                if let Some((key, value)) = field.split_once(":") {
                                                    match key {
                                                        "STATUS" => success = value == "true",
                                                        "MESSAGE" => message = Some(value),
                                                        "HASH" => {
                                                            if value != "none" {
                                                                hash = Some(value);
                                                            }
                                                        },
                                                        "COMMIT_MSG" => {
                                                            if value != "none" {
                                                                commit_msg = Some(value);
                                                            }
                                                        },
                                                        "FILES" => files = value.parse().unwrap_or(0),
                                                        "INS" => ins = value.parse().unwrap_or(0),
                                                        "DELS" => dels = value.parse().unwrap_or(0),
                                                        _ => {}
                                                    }
                                                }
                                            }

                                            // Print message
                                            if let Some(msg) = message {
                                                println!("{}", msg);
                                            }

                                            // Print commit hash
                                            if let Some(h) = hash {
                                                println!("Commit hash: {}", h);
                                            }

                                            // Print commit message
                                            if let Some(cm) = commit_msg {
                                                println!("\n💬 Commit message:");
                                                println!("  {}", cm);
                                            }

                                            // Print changes summary
                                            if files > 0 || ins > 0 || dels > 0 {
                                                println!("\n📊 Change summary:");
                                                println!("  {} files changed", files);
                                                if ins > 0 {
                                                    println!("  {} insertions(+)", ins);
                                                }
                                                if dels > 0 {
                                                    println!("  {} deletions(-)", dels);
                                                }
                                            }
                                        } else {
                                            // Fall back to the regular JSON format
                                            // Check if the operation was successful or not
                                            let success = data.get("success").and_then(|s| s.as_bool()).unwrap_or(false);

                                            if success {
                                                println!("\n✅ Commit operation completed successfully");
                                            } else {
                                                println!("\n⚠️ Commit operation completed with issues");
                                            }

                                            // Extract message
                                            if let Some(message) = data.get("message").and_then(|m| m.as_str()) {
                                                println!("{}", message);
                                            }

                                            // Extract commit hash if available
                                            if let Some(hash) = data.get("commit_hash").and_then(|h| h.as_str()) {
                                                println!("Commit hash: {}", hash);
                                            }

                                            // Display the commit message if available
                                            if let Some(commit_msg) = data.get("commit_message").and_then(|m| m.as_str()) {
                                                println!("\n💬 Commit message:");
                                                println!("  {}", commit_msg);
                                            }

                                            // Show summary of changes if available
                                            let files = data.get("files_changed").and_then(|f| f.as_u64()).unwrap_or(0);
                                            let ins = data.get("insertions").and_then(|i| i.as_u64()).unwrap_or(0);
                                            let dels = data.get("deletions").and_then(|d| d.as_u64()).unwrap_or(0);

                                            if files > 0 || ins > 0 || dels > 0 {
                                                println!("\n📊 Change summary:");
                                                println!("  {} files changed", files);
                                                if ins > 0 {
                                                    println!("  {} insertions(+)", ins);
                                                }
                                                if dels > 0 {
                                                    println!("  {} deletions(-)", dels);
                                                }
                                            }
                                        }
                                    } else {
                                        println!("Result from {}: Unable to parse JSON", actor_id);
                                    }
                                } else {
                                    println!("Result from {}: No data returned", actor_id);
                                }
                            }
                            ActorResult::Error(ChildError { actor_id, error }) => {
                                println!("❌ Error from actor {}: {}", actor_id, error);
                            }
                        }
                        break;
                    }
                    ManagementResponse::Error { error } => {
                        println!("❌ Error starting commit actor: {:?}", error);
                    }
                    _ => {
                        println!("❓ Unexpected response: {:?}", msg);
                    }
                }
            }
        }
    }

    Ok(())
}
