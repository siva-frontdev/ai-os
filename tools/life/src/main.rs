//! The `life` command-line interface.
//!
//! Usage:
//!
//! ```text
//! life setup <provider> [--env-file <path>] [--port <n>] [--timeout <secs>]
//!                       [--to <email>] [--no-verify] [--force]
//! life setup --list
//! ```
//!
//! On failure the command prints a structured error (matching the runtime's
//! `ProviderError` JSON shape) and exits non-zero.

use std::error::Error;

use life::setup::{
    find_provider, providers, SetupContext, DEFAULT_CALLBACK_PORT, DEFAULT_ENV_FILE,
    DEFAULT_TIMEOUT_SECS,
};
use life::SetupError;

#[derive(Debug)]
struct Cli {
    provider: String,
    ctx: SetupContext,
}

fn print_usage() {
    println!(
        "life setup <provider> [options]\n\n\
         Providers: gmail, whatsapp\n\n\
         Options:\n\
           --env-file <path>   credentials file (default {DEFAULT_ENV_FILE})\n\
           --port <n>          localhost callback port (default {DEFAULT_CALLBACK_PORT})\n\
           --timeout <secs>    callback timeout seconds (default {DEFAULT_TIMEOUT_SECS})\n\
           --to <email|phone>  send a real test message after validation\n\
           --no-verify         skip live validation after setup\n\
           --force             re-run setup even if credentials are stored\n\
           --list              list registered providers\n\
           --help              this help"
    );
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut provider = String::new();
    let mut ctx = SetupContext::default();

    // Consume the `setup` subcommand.
    let args = match args.first().map(String::as_str) {
        Some("setup") => &args[1..],
        Some("--list") | Some("--help") | Some("-h") => args,
        Some(other) => {
            return Err(format!("unknown command: {other} (try `life setup gmail`)"));
        }
        None => args,
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--list" => {
                let names: Vec<&str> = providers().iter().map(|p| p.name()).collect();
                println!("providers: {}", names.join(", "));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--env-file" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --env-file".to_string())?;
                ctx.env_file = value.into();
            }
            "--port" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --port".to_string())?;
                ctx.callback_port = value.parse().map_err(|_| "invalid --port".to_string())?;
            }
            "--timeout" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --timeout".to_string())?;
                ctx.timeout_secs = value.parse().map_err(|_| "invalid --timeout".to_string())?;
            }
            "--to" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "missing value for --to".to_string())?;
                ctx.test_recipient = Some(value.clone());
            }
            "--no-verify" => ctx.verify = false,
            "--force" => ctx.force = true,
            other if other.starts_with('-') => return Err(format!("unknown argument: {other}")),
            other if provider.is_empty() => provider = other.to_string(),
            other => return Err(format!("unexpected argument: {other}")),
        }
        i += 1;
    }

    if provider.is_empty() {
        return Err("no provider given (try: life setup gmail)".to_string());
    }
    Ok(Cli { provider, ctx })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        std::process::exit(2);
    }

    let cli = match parse_args(&args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("error: {e}");
            print_usage();
            std::process::exit(2);
        }
    };

    let Some(provider) = find_provider(&cli.provider) else {
        let err = SetupError {
            code: "InvalidInput".into(),
            message: format!(
                "unknown provider '{}'; run `life setup --list` for registered providers",
                cli.provider
            ),
            retryable: false,
        };
        fail(&err);
    };

    if let Err(err) = provider.setup(&cli.ctx).await {
        fail(&err);
    }

    println!("✓ {0} configured successfully", cli.provider);
    Ok(())
}

/// Print a structured provider error and exit non-zero.
fn fail(err: &SetupError) -> ! {
    eprintln!("error: {err}");
    eprintln!(
        "{}",
        serde_json::to_string_pretty(&err.as_json()).unwrap_or_default()
    );
    std::process::exit(1);
}
