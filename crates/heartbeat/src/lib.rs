use chrono::Local;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::types::{Message, Role};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use uuid::Uuid;

pub struct HeartbeatService {
    workspace: PathBuf,
    interval_secs: u64,
    enabled: bool,
    running: Arc<AtomicBool>,
}

impl HeartbeatService {
    pub fn new(workspace: PathBuf, interval_secs: u64, enabled: bool) -> Self {
        Self {
            workspace,
            interval_secs,
            enabled,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the heartbeat loop. Publishes heartbeat prompts to the bus
    /// so the agent can act on scheduled checks.
    pub fn start(&self, bus: Arc<MessageBus>) -> tokio::task::JoinHandle<()> {
        let running = self.running.clone();
        let workspace = self.workspace.clone();
        let interval_secs = self.interval_secs;
        let enabled = self.enabled;

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            if !enabled {
                info!("Heartbeat service disabled");
                return;
            }

            info!(interval_secs, "Heartbeat service started");
            let mut ticker = interval(Duration::from_secs(interval_secs));

            loop {
                ticker.tick().await;

                if !running.load(Ordering::SeqCst) {
                    info!("Heartbeat service stopped");
                    return;
                }

                let prompt = build_prompt(&workspace);

                // Publish heartbeat prompt to bus so the agent processes it
                let msg = Message::new(
                    "heartbeat",
                    "heartbeat:system",
                    Role::User,
                    &prompt,
                ).with_sender("system");

                if let Err(e) = bus.publish(Event::InboundMessage(msg)) {
                    error!("Failed to publish heartbeat to bus: {}", e);
                }

                // Also log to file as audit trail
                log_heartbeat(&workspace, &prompt);
            }
        })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("Heartbeat service stop requested");
    }
}

fn build_prompt(workspace: &Path) -> String {
    let notes_file = workspace.join("memory/HEARTBEAT.md");
    let notes = fs::read_to_string(&notes_file).unwrap_or_default();
    let now = Local::now().format("%Y-%m-%d %H:%M").to_string();

    format!(
        "# Heartbeat Check\n\nCurrent time: {}\n\n\
        Check if there are any tasks I should be aware of or actions I should take.\n\
        Review the memory file for any important updates or changes.\n\n{}\n",
        now, notes
    )
}

fn log_heartbeat(workspace: &Path, message: &str) {
    let log_file = workspace.join("memory/heartbeat.log");
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let entry = format!("[{}] Heartbeat check completed ({}B prompt)\n", timestamp, message.len());

    if let Err(e) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(entry.as_bytes())
        })
    {
        error!("Failed to write heartbeat log: {}", e);
    }
}
