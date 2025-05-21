use anyhow::{Context, Result};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use theater::client::TheaterConnection;
use theater::messages::ActorResult;
use theater::messages::{ChildError, ChildResult};
use theater::theater_server::{ManagementCommand, ManagementResponse};

mod ui;

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

    // Display the styled header
    ui::print_header();

    // Print repository info only
    ui::print_item(
        "Repository",
        &current_dir.display().to_string(),
        Some("highlight"),
    );

    // Connect to the Theater server
    let mut connection = connect_to_server(server_address)
        .await
        .context("Failed to connect to Theater server")?;

    // Remove the connected message and separator

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

    // Remove repository checking status

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

    // Remove starting actor status

    loop {
        tokio::select! {
            Ok(msg) = connection.receive() => {
                match msg {
                    ManagementResponse::ActorStarted { id } => {
                        ui::print_item("Actor ID", &id.to_string(), Some("info"));
                    },
                    ManagementResponse::ActorResult(result) => {
                        match result {
                            ActorResult::Success(ChildResult { actor_id, result }) => {
                                if let Some(bytes) = result {
                                    if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                                        // First check if we're using the new status_msg format
                                        if let Some(status_msg) = data.get("status_msg").and_then(|s| s.as_str()) {
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

                                            // Only print message if there are issues (not success)
                                            if !success {
                                                if let Some(msg) = message {
                                                    ui::print_item("Message", &msg, Some("warning"));
                                                }
                                            }

                                            // Print commit hash
                                            if let Some(h) = hash {
                                                ui::print_item("Commit hash", h, Some("info"));
                                            }

                                            // Print commit message in a box without additional status
                                            if let Some(cm) = commit_msg {
                                                ui::print_commit_message(cm);
                                            }

                                            // Print simplified change summary on one line
                                            if files > 0 || ins > 0 || dels > 0 {
                                                let mut summary = format!("Files: {}", files);
                                                if ins > 0 {
                                                    summary.push_str(&format!(", +{}", ins));
                                                }
                                                if dels > 0 {
                                                    summary.push_str(&format!(", -{}", dels));
                                                }
                                                ui::print_item("Changes", &summary, Some("info"));
                                            }
                                        } else {
                                            // Fall back to the regular JSON format
                                            // Just track operation success without printing status
                                            let success = data.get("success").and_then(|s| s.as_bool()).unwrap_or(false);

                                            // Only print message if there are issues (not success)
                                            if !success {
                                                if let Some(message) = data.get("message").and_then(|m| m.as_str()) {
                                                    ui::print_item("Message", message, Some("warning"));
                                                }
                                            }

                                            // Extract commit hash if available
                                            if let Some(hash) = data.get("commit_hash").and_then(|h| h.as_str()) {
                                                ui::print_item("Commit hash", hash, Some("info"));
                                            }

                                            // Display the commit message if available without additional status
                                            if let Some(commit_msg) = data.get("commit_message").and_then(|m| m.as_str()) {
                                                ui::print_commit_message(commit_msg);
                                            }

                                            // Print simplified change summary on one line
                                            let files = data.get("files_changed").and_then(|f| f.as_u64()).unwrap_or(0);
                                            let ins = data.get("insertions").and_then(|i| i.as_u64()).unwrap_or(0);
                                            let dels = data.get("deletions").and_then(|d| d.as_u64()).unwrap_or(0);

                                            if files > 0 || ins > 0 || dels > 0 {
                                                let mut summary = format!("Files: {}", files);
                                                if ins > 0 {
                                                    summary.push_str(&format!(", +{}", ins));
                                                }
                                                if dels > 0 {
                                                    summary.push_str(&format!(", -{}", dels));
                                                }
                                                ui::print_item("Changes", &summary, Some("info"));
                                            }
                                        }
                                    } else {
                                        ui::print_status(format!("Result from {}: Unable to parse JSON", actor_id), "error");
                                    }
                                } else {
                                    ui::print_status(format!("Result from {}: No data returned", actor_id), "warning");
                                }
                            }
                            ActorResult::Error(ChildError { actor_id: _, error }) => {
                                ui::print_item("Error", &error.to_string(), Some("error"));
                            }
                        }
                        break;
                    }
                    ManagementResponse::Error { error } => {
                        ui::print_item("Error", &format!("{:?}", error), Some("error"));
                    }
                    _ => {
                        ui::print_status(format!("Unexpected response: {:?}", msg), "warning");
                    }
                }
            }
        }
    }

    Ok(())
}
