//! Updated th-commit implementation using event-driven Theater client
//! This version handles all Theater messages asynchronously without relying on message ordering

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use theater::id::TheaterId;
use theater::ChainEvent;
use theater_client::TheaterConnection;
use theater_server::{ManagementCommand, ManagementResponse};
use tokio::time::timeout;
use tracing::info;

mod ui;

#[derive(Parser, Debug)]
#[command(name = "th-commit", about = "AI-powered git commits using Theater")]
struct Args {
    /// Theater server address
    #[arg(long, env = "THEATER_SERVER_ADDRESS", default_value = "127.0.0.1:9000")]
    server: String,

    /// Automatically push after commit
    #[arg(long)]
    auto_push: bool,

    /// Custom commit message prefix
    #[arg(long)]
    prefix: Option<String>,

    /// Skip staging (commit only already staged files)
    #[arg(long)]
    skip_staging: bool,

    /// Dry run (show what would be committed without actually doing it)
    #[arg(long)]
    dry_run: bool,

    /// Timeout for the commit operation in seconds
    #[arg(long, default_value = "120")]
    timeout_seconds: u64,

    /// Enable verbose logging
    #[arg(long, short)]
    verbose: bool,
}

const COMMIT_ACTOR_MANIFEST: &str =
    "/Users/colinrozzi/work/actor-registry/commit-actor/manifest.toml";

/// Structured response from the commit actor
#[derive(Debug, Serialize, Deserialize)]
struct CommitResult {
    success: bool,
    message: Option<String>,
    commit_hash: Option<String>,
    commit_message: Option<String>,
    files_changed: Option<u64>,
    insertions: Option<u64>,
    deletions: Option<u64>,
    pushed: Option<bool>,
    error: Option<String>,
    status_msg: Option<String>,
}

/// Event-driven Theater client that handles all messages asynchronously
struct EventDrivenClient {
    connection: TheaterConnection,
}

impl EventDrivenClient {
    async fn new(server_addr: &str) -> Result<Self> {
        let addr: SocketAddr = server_addr.parse()
            .context("Invalid server address")?;
        
        let mut connection = TheaterConnection::new(addr);
        connection.connect().await
            .context("Failed to connect to Theater server")?;
        
        Ok(Self { connection })
    }

    /// Send a command to the server
    async fn send_command(&mut self, command: ManagementCommand) -> Result<()> {
        self.connection.send(command).await
    }

    /// Start an actor and wait for it to start
    async fn start_actor(&mut self, manifest_path: &str, initial_state: serde_json::Value) -> Result<TheaterId> {
        // Serialize initial state to bytes
        let initial_state_bytes = serde_json::to_vec(&initial_state)?;
        
        // Send start actor command
        let command = ManagementCommand::StartActor {
            manifest: manifest_path.to_string(),
            initial_state: Some(initial_state_bytes),
            parent: false,
            subscribe: true,
        };
        
        self.connection.send(command).await?;

        // Process incoming messages until we get the actor ID
        loop {
            let response = self.connection.receive().await?;
            match response {
                ManagementResponse::ActorStarted { id } => {
                    return Ok(id);
                },
                ManagementResponse::ActorEvent { event } => {
                    handle_commit_event(&event);
                },
                ManagementResponse::Error { error } => {
                    return Err(anyhow!("Failed to start actor: {:?}", error));
                },
                _ => {
                    // Other responses - continue processing
                }
            }
        }
    }

    /// Send a request to an actor and wait for response
    async fn request_actor_message(&mut self, actor_id: &TheaterId, message: serde_json::Value) -> Result<Vec<u8>> {
        let message_bytes = serde_json::to_vec(&message)?;
        let command = ManagementCommand::RequestActorMessage {
            id: actor_id.clone(),
            data: message_bytes,
        };
        
        self.connection.send(command).await?;

        // Process messages until we get the response
        loop {
            let response = self.connection.receive().await?;
            match response {
                ManagementResponse::RequestedMessage { message, .. } => {
                    return Ok(message);
                },
                ManagementResponse::ActorEvent { event } => {
                    handle_commit_event(&event);
                },
                ManagementResponse::Error { error } => {
                    return Err(anyhow!("Request failed: {:?}", error));
                },
                _ => {
                    // Other responses - continue processing
                }
            }
        }
    }

    /// Subscribe to actor events
    async fn subscribe_to_events(&mut self, actor_id: &TheaterId) -> Result<()> {
        let command = ManagementCommand::SubscribeToActor {
            id: actor_id.clone(),
        };
        self.connection.send(command).await
    }

    /// Stop an actor
    async fn stop_actor(&mut self, actor_id: &TheaterId) -> Result<()> {
        let command = ManagementCommand::StopActor {
            id: actor_id.clone(),
        };
        self.connection.send(command).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        tracing_subscriber::fmt::init();
    }

    // Validate prerequisites
    validate_prerequisites()?;

    // Get repository information
    let repo_path = env::current_dir().context("Failed to get current directory")?;
    let api_key = env::var("GOOGLE_GEMINI_API_KEY")
        .context("GOOGLE_GEMINI_API_KEY environment variable not set")?;

    // Print header
    ui::print_header();
    ui::print_item(
        "Repository",
        &repo_path.display().to_string(),
        Some("highlight"),
    );

    // Execute commit operation
    execute_commit(&args, repo_path, api_key).await?;

    Ok(())
}

fn validate_prerequisites() -> Result<()> {
    // Check if we're in a git repository
    if !std::path::Path::new(".git").exists() {
        return Err(anyhow!("Not in a git repository"));
    }

    // Check if git is available
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return Err(anyhow!("Git is not available in PATH"));
    }

    Ok(())
}

async fn execute_commit(args: &Args, repo_path: std::path::PathBuf, api_key: String) -> Result<()> {
    // Create event-driven client
    let mut client = EventDrivenClient::new(&args.server).await
        .context("Failed to connect to Theater server")?;

    info!("Connected to Theater server at {}", args.server);

    // Prepare initial state for commit actor
    let initial_state = json!({
        "repository_path": repo_path.to_string_lossy(),
        "api_key": api_key,
        "auto_push": args.auto_push,
        "message_prefix": args.prefix,
        "skip_staging": args.skip_staging,
        "dry_run": args.dry_run
    });

    println!("🚀 Starting commit actor...");

    // Start commit actor (this handles all the async message processing)
    let actor_id = client
        .start_actor(COMMIT_ACTOR_MANIFEST, initial_state)
        .await
        .context("Failed to start commit actor")?;

    ui::print_item("Actor ID", &actor_id.to_string(), Some("info"));

    // Subscribe to events (events will be handled automatically during request processing)
    client.subscribe_to_events(&actor_id).await
        .context("Failed to subscribe to events")?;

    // Send commit request
    let commit_request = json!({
        "action": "commit",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    println!("📝 Requesting commit...");

    // Use timeout for the entire operation
    let operation = async {
        let response_bytes = client
            .request_actor_message(&actor_id, commit_request)
            .await?;
        
        // Parse response
        let result: CommitResult = serde_json::from_slice(&response_bytes)
            .context("Failed to parse commit result")?;

        Ok::<CommitResult, anyhow::Error>(result)
    };

    let result = timeout(Duration::from_secs(args.timeout_seconds), operation)
        .await
        .context("Commit operation timed out")?
        .context("Commit operation failed")?;

    // Display results
    display_commit_result(&result)?;

    // Clean shutdown
    if let Err(e) = client.stop_actor(&actor_id).await {
        eprintln!("Warning: Failed to stop actor: {}", e);
    }

    Ok(())
}

fn handle_commit_event(event: &ChainEvent) {
    match event.event_type.as_str() {
        "git_status_check" => {
            println!("🔍 Checking git status...");
        }
        "staging_files" => {
            println!("📁 Staging files...");
        }
        "generating_message" => {
            println!("🤖 Generating commit message with AI...");
        }
        "creating_commit" => {
            println!("💾 Creating commit...");
        }
        "pushing_changes" => {
            println!("🌐 Pushing to remote...");
        }
        "error" => {
            if let Ok(error_msg) = String::from_utf8(event.data.clone()) {
                ui::print_item("Error", &error_msg, Some("error"));
            }
        }
        _ => {
            // Handle other event types or show description
            if let Some(desc) = &event.description {
                println!("ℹ️  {}", desc);
            }
        }
    }
}

fn display_commit_result(result: &CommitResult) -> Result<()> {
    println!("\n{}", "=".repeat(50));

    // Handle pipe-delimited status format if present
    if let Some(status_msg) = &result.status_msg {
        parse_and_display_status_msg(status_msg)?;
    } else {
        // Handle regular JSON format
        display_json_result(result)?;
    }

    println!("{}", "=".repeat(50));
    Ok(())
}

fn parse_and_display_status_msg(status_msg: &str) -> Result<()> {
    let mut success = false;
    let mut message = None;
    let mut hash = None;
    let mut commit_msg = None;
    let mut files = 0;
    let mut ins = 0;
    let mut dels = 0;

    // Parse pipe-delimited format
    for field in status_msg.split('|') {
        if let Some((key, value)) = field.split_once(':') {
            match key {
                "STATUS" => success = value == "true",
                "MESSAGE" => {
                    if value != "none" {
                        message = Some(value);
                    }
                }
                "HASH" => {
                    if value != "none" {
                        hash = Some(value);
                    }
                }
                "COMMIT_MSG" => {
                    if value != "none" {
                        commit_msg = Some(value);
                    }
                }
                "FILES" => files = value.parse().unwrap_or(0),
                "INS" => ins = value.parse().unwrap_or(0),
                "DELS" => dels = value.parse().unwrap_or(0),
                _ => {}
            }
        }
    }

    if success {
        println!("✅ Commit completed successfully!");
    } else {
        println!("❌ Commit failed");
        if let Some(msg) = message {
            ui::print_item("Error", msg, Some("error"));
        }
        return Ok(());
    }

    if let Some(h) = hash {
        ui::print_item("Commit hash", h, Some("info"));
    }

    if let Some(cm) = commit_msg {
        println!("\n💬 Commit message:");
        ui::print_commit_message(cm);
    }

    // Display change statistics
    if files > 0 || ins > 0 || dels > 0 {
        let mut stats = format!("📊 Changes: {} files", files);
        if ins > 0 {
            stats.push_str(&format!(", +{} insertions", ins));
        }
        if dels > 0 {
            stats.push_str(&format!(", -{} deletions", dels));
        }
        println!("{}", stats);
    }

    Ok(())
}

fn display_json_result(result: &CommitResult) -> Result<()> {
    if result.success {
        println!("✅ Commit completed successfully!");

        if let Some(hash) = &result.commit_hash {
            ui::print_item("Commit hash", hash, Some("info"));
        }

        if let Some(message) = &result.commit_message {
            println!("\n💬 Commit message:");
            ui::print_commit_message(message);
        }

        // Display change statistics
        let files = result.files_changed.unwrap_or(0);
        let insertions = result.insertions.unwrap_or(0);
        let deletions = result.deletions.unwrap_or(0);

        if files > 0 || insertions > 0 || deletions > 0 {
            let mut stats = format!("📊 Changes: {} files", files);
            if insertions > 0 {
                stats.push_str(&format!(", +{} insertions", insertions));
            }
            if deletions > 0 {
                stats.push_str(&format!(", -{} deletions", deletions));
            }
            println!("{}", stats);
        }

        if result.pushed.unwrap_or(false) {
            println!("🌐 Changes pushed to remote repository");
        }
    } else {
        println!("❌ Commit failed");
        if let Some(error) = &result.error {
            ui::print_item("Error", error, Some("error"));
        }
        if let Some(message) = &result.message {
            ui::print_item("Details", message, Some("warning"));
        }
    }

    Ok(())
}
