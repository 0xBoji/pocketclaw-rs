use clap::Parser;
use std::time::Duration;
use tokio::process::Command;
use tokio::signal;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Command to run (e.g. picoclaw-server)
    #[arg(long, default_value = "./picoclaw-server")]
    command: String,

    /// Arguments to pass to the command
    #[arg(long)]
    args: Vec<String>,

    /// Health check URL
    #[arg(long, default_value = "http://127.0.0.1:3000/health")]
    health_url: String,

    /// Health check interval in seconds
    #[arg(long, default_value_t = 30)]
    health_interval_secs: u64,

    /// Max consecutive health check failures before restart
    #[arg(long, default_value_t = 3)]
    max_fails: u32,

    /// Restart delay in seconds
    #[arg(long, default_value_t = 5)]
    restart_delay_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Setup logging (file + stdout)
    let file_appender = tracing_appender::rolling::daily("logs", "supervisor.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Log to file AND stdout
    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking.and(std::io::stdout))
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("Supervisor started. Monitoring command: {}", args.command);

    let client = reqwest::Client::new();

    loop {
        // Spawn the child process
        let child = Command::new(&args.command)
            .args(&args.args)
            .kill_on_drop(true) // Ensure child dies if supervisor panics/drops handle
            // Redirect output to inherit (supervisor's stdout/stderr)
            // Or log to file? For now inherit so we see it in terminal/logs if supervisor is redirected.
            // On Termux, user usually runs supervisor and wants to see output.
            // But if supervisor runs in background, output goes nowhere?
            // "Log rotation" implies we capture child output.
            // But complex to pipe child output to tracing.
            // Best to let child log to its own file using tracing-appender (Wave B3/C3 done elsewhere?).
            // Agent logs to `~/.phoneclaw/logs`. Supervisor logs to `logs/supervisor.log`.
            .spawn();

        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to spawn command '{}': {}", args.command, e);
                sleep(Duration::from_secs(args.restart_delay_secs)).await;
                continue;
            }
        };

        info!("Child process started (PID: {:?})", child.id());

        let mut fail_count = 0;
        let mut check_interval = interval(Duration::from_secs(args.health_interval_secs));
        
        // Monitor loop
        loop {
            tokio::select! {
                // 1. Child exited
                status = child.wait() => {
                    match status {
                        Ok(s) => error!("Child exited with status: {}", s),
                        Err(e) => error!("Child wait failed: {}", e),
                    }
                    break; // Break monitor loop to restart
                }

                // 2. Health check interval
                _ = check_interval.tick() => {
                    match client.get(&args.health_url).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                fail_count = 0;
                            } else {
                                fail_count += 1;
                                warn!("Health check returned status: {}. Fail count: {}/{}", resp.status(), fail_count, args.max_fails);
                            }
                        }
                        Err(e) => {
                            fail_count += 1;
                            warn!("Health check failed: {}. Fail count: {}/{}", e, fail_count, args.max_fails);
                        }
                    }

                    if fail_count >= args.max_fails {
                        error!("Max health check failures reached. Restarting child...");
                        let _ = child.kill().await;
                        // Child exit will be caught by `child.wait()` branch?
                        // If we kill, `wait()` resolves.
                        // But select! branches are raced. If we kill here, `wait` might resolve in next loop.
                        // Or we explicit break.
                        // If we break here, `child` is dropped. `kill_on_drop(true)` kills it.
                        break; 
                    }
                }

                // 3. Supervisor signal (Ctrl+C)
                _ = signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down...");
                    let _ = child.kill().await;
                    return Ok(());
                }
            }
        }

        info!("Restarting child in {} seconds...", args.restart_delay_secs);
        sleep(Duration::from_secs(args.restart_delay_secs)).await;
    }
}
