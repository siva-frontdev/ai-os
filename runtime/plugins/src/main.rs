//! The `ai-os-mcp-server` binary: hosts a selected set of plugins over the
//! MCP stdio transport.
//!
//! AI-OS spawns this binary via `ai-os-openclaw-runtime`'s stdio transport.
//! All logging goes to **stderr** — anything written to stdout would
//! corrupt the newline-delimited JSON-RPC protocol on stdout.
//!
//! Usage:
//!
//! ```text
//! ai-os-mcp-server [--plugins email,filesystem,github,calendar,telegram,whatsapp]
//!                  [--root <dir>]
//!                  [--env-file <path>]
//! ```
//!
//! `--plugins` selects which plugins to host (comma-separated, default all).
//! `--root` sets the filesystem plugin root (default the current directory).
//! `--env-file` loads provider credentials from a `.env` file (default
//! `runtime/config/.env`, as written by `life setup`).

use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use ai_os_mcp_server::{McpServer, McpServerError, StdioServerTransport};
use ai_os_plugins::provider::{load_env_file, Providers};
use ai_os_plugins::{build_plugins, PLUGIN_NAMES};

/// Command-line configuration.
#[derive(Debug)]
struct Cli {
    plugins: Vec<String>,
    root: PathBuf,
    env_file: PathBuf,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut plugins: Vec<String> = PLUGIN_NAMES.iter().map(|s| s.to_string()).collect();
    let mut root = PathBuf::from(".");
    let mut env_file = PathBuf::from("runtime/config/.env");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--plugins" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --plugins".to_string())?;
                plugins = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--root" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --root".to_string())?;
                root = PathBuf::from(value);
            }
            "--env-file" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --env-file".to_string())?;
                env_file = PathBuf::from(value);
            }
            "--help" | "-h" => {
                println!("ai-os-mcp-server [--plugins a,b,c] [--root <dir>] [--env-file <path>]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(Cli {
        plugins,
        root,
        env_file,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(&args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    if let Err(e) = load_env_file(&cli.env_file) {
        tracing::warn!(env_file = %cli.env_file.display(), "failed to load env file: {e}");
    }

    let enabled: HashSet<String> = cli.plugins.into_iter().collect();
    let providers = Providers::from_env();
    let plugins = match build_plugins(&cli.root, &enabled, &providers) {
        Ok(plugins) => plugins,
        Err(McpServerError::Plugin(e)) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
        Err(e) => return Err(e.into()),
    };

    let server = McpServer::new(plugins);
    let transport = StdioServerTransport::new();
    tracing::info!(plugins = ?enabled, root = %cli.root.display(), "ai-os-mcp-server ready");
    server.serve(Arc::new(transport)).await;
    Ok(())
}
