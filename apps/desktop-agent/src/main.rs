mod agent;
mod operator;
mod strategy;
mod verify;

use ai_os_core::bootstrap::PlatformBuilder;
use osal_linux::LinuxKernelFacade;
use std::io::{self, BufRead, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("AI-OS Desktop Operator v1");
    println!("=========================");
    println!("Embodied execution engine — the AI's hands and eyes.");
    println!("Describe what you'd like done. The Brain plans; I execute.");
    println!("Type 'exit' or 'quit' to quit.");
    println!();

    let app = PlatformBuilder::new()
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("Platform build failed: {e}"))?;

    app.run()
        .await
        .map_err(|e| anyhow::anyhow!("Platform run failed: {e}"))?;

    let kernel = LinuxKernelFacade::new();
    let agent = agent::DesktopAgent::new(app, kernel).await?;

    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("> ");
        io::stdout().flush().ok();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        if matches!(input, "exit" | "quit" | "q") {
            println!("Goodbye!");
            break;
        }

        match agent.process_input(input).await {
            Ok(output) => {
                print!("{}", output);
                if !output.ends_with('\n') {
                    println!();
                }
            }
            Err(e) => {
                println!("Error: {e}");
            }
        }
    }

    agent.shutdown().await;

    Ok(())
}
