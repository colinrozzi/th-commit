//! Enhanced th-commit using the Theater Client library
//! 
//! This version demonstrates how much simpler and more robust the code becomes
//! with the enhanced Theater client library.

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tokio::time::Duration;
use tracing::info;

// Use the enhanced theater client
use theater_client::prelude::*;
use theater_client::ChainEvent;

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
    
    // Support for the pipe-delimited status format
    status_msg: Option<String>,
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
    ui::print_item("Repository", &repo_path.display().to_string(), Some("highlight"));
    
    // Execute commit operation
    execute_commit(&args, repo_path, api_key).await?;
    
    Ok(())
}

fn validate_prerequisites() -> Result<()> {
    // Check if we're in a git repository
    if !std::path::Path::new(".git").exists() {
        return Err(anyhow::anyhow!("Not in a git repository"));
    }
    
    // Check if git is available
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return Err(anyhow::anyhow!("Git is not available in PATH"));
    }
    
    Ok(())
}

async fn execute_commit(
    args: &Args,
    repo_path: std::path::PathBuf,
    api_key: String,
) -> Result<()> {
    // Create robust client with retry logic
    let mut client = TheaterClientBuilder::new()
        .server(&args.server)?
        .connection_timeout(Duration::from_secs(10))
        .request_timeout(Duration::from_secs(args.timeout_seconds))
        .build();
    
    // Connect with retry logic
    client.connect_with_retry(RetryConfig::default())
        .await
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
    
    // Start commit actor with robust error handling and timeout
    let timeout_duration = Duration::from_secs(args.timeout_seconds);
    let mut commit_actor = client.start_and_wait(
        COMMIT_ACTOR_MANIFEST,
        StartActorOptionsBuilder::new()
            .initial_state_json(&initial_state)?
            .timeout(timeout_duration)
            .build()
    ).await.context("Failed to start commit actor")?;
    
    ui::print_item("Actor ID", &commit_actor.id.to_string(), Some("info"));
    
    // Subscribe to events for real-time progress updates
    let mut events = commit_actor.subscribe_to_events().await.context("Failed to subscribe to events")?;
    
    // Monitor events in background
    let event_handle = tokio::spawn(async move {
        while let Some(event) = events.receive().await {
            handle_commit_event(&event);
        }
    });
    
    // Send commit request and wait for result
    let commit_request = json!({
        "action": "commit",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    println!("📝 Requesting commit...");
    
    // Use request_json for type-safe communication
    let result: CommitResult = commit_actor
        .request_json(&commit_request)
        .await
        .context("Failed to get commit result")?;
    
    // Wait for event handler to finish
    event_handle.abort();
    
    // Display results
    display_commit_result(&result)?;
    
    // Clean shutdown with timeout
    commit_actor.stop_with_timeout(Duration::from_secs(5)).await
        .unwrap_or_else(|e| eprintln!("Warning: Actor shutdown timeout: {}", e));
    
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
                "MESSAGE" => if value != "none" { message = Some(value); },
                "HASH" => if value != "none" { hash = Some(value); },
                "COMMIT_MSG" => if value != "none" { commit_msg = Some(value); },
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_commit_result_parsing() {
        let json_result = json!({
            "success": true,
            "commit_hash": "abc123",
            "commit_message": "feat: add new feature",
            "files_changed": 3,
            "insertions": 45,
            "deletions": 12
        });
        
        let result: CommitResult = serde_json::from_value(json_result).unwrap();
        assert!(result.success);
        assert_eq!(result.commit_hash, Some("abc123".to_string()));
        assert_eq!(result.files_changed, Some(3));
    }
    
    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(&["th-commit", "--auto-push", "--timeout-seconds", "60"]);
        assert!(args.auto_push);
        assert_eq!(args.timeout_seconds, 60);
    }
    
    #[test]
    fn test_status_msg_parsing() {
        let status_msg = "STATUS:true|HASH:abc123|COMMIT_MSG:test commit|FILES:2|INS:10|DELS:5";
        
        // This would normally call parse_and_display_status_msg, but that prints
        // so we'll just test the parsing logic inline
        let mut success = false;
        let mut hash = None;
        
        for field in status_msg.split('|') {
            if let Some((key, value)) = field.split_once(':') {
                match key {
                    "STATUS" => success = value == "true",
                    "HASH" => if value != "none" { hash = Some(value); },
                    _ => {}
                }
            }
        }
        
        assert!(success);
        assert_eq!(hash, Some("abc123"));
    }
}
