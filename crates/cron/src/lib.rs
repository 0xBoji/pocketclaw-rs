use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::types::{Message, Role};
use tokio::time::{interval, Duration};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronSchedule {
    pub kind: String,
    #[serde(rename = "atMs", skip_serializing_if = "Option::is_none")]
    pub at_ms: Option<i64>,
    #[serde(rename = "everyMs", skip_serializing_if = "Option::is_none")]
    pub every_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronPayload {
    pub kind: String,
    pub message: String,
    pub deliver: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobState {
    #[serde(rename = "nextRunAtMs", skip_serializing_if = "Option::is_none")]
    pub next_run_at_ms: Option<i64>,
    #[serde(rename = "lastRunAtMs", skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<i64>,
    #[serde(rename = "lastStatus", skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    #[serde(rename = "lastError", skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub schedule: CronSchedule,
    pub payload: CronPayload,
    pub state: CronJobState,
    #[serde(rename = "createdAtMs")]
    pub created_at_ms: i64,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: i64,
    #[serde(rename = "deleteAfterRun", default)]
    pub delete_after_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronStore {
    pub version: i32,
    pub jobs: Vec<CronJob>,
}

impl Default for CronStore {
    fn default() -> Self {
        Self {
            version: 1,
            jobs: Vec::new(),
        }
    }
}

pub struct CronService {
    store_path: PathBuf,
    store: Arc<RwLock<CronStore>>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

impl CronService {
    pub fn new(store_path: PathBuf) -> Self {
        let store = Self::load_store(&store_path);
        Self {
            store_path,
            store: Arc::new(RwLock::new(store)),
        }
    }

    fn load_store(path: &Path) -> CronStore {
        match fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => CronStore::default(),
        }
    }

    fn save_store(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let store = self.store.read().unwrap();
        let data = serde_json::to_string_pretty(&*store)?;
        fs::write(&self.store_path, data)?;
        Ok(())
    }

    fn compute_next_run(schedule: &CronSchedule, now: i64) -> Option<i64> {
        match schedule.kind.as_str() {
            "at" => schedule.at_ms.filter(|&at| at > now),
            "every" => schedule.every_ms.filter(|&e| e > 0).map(|e| now + e),
            _ => None,
        }
    }

    pub fn add_job(
        &self,
        name: String,
        schedule: CronSchedule,
        message: String,
        deliver: bool,
        channel: Option<String>,
        to: Option<String>,
    ) -> anyhow::Result<CronJob> {
        let now = now_ms();
        let next_run = Self::compute_next_run(&schedule, now);

        let job = CronJob {
            id: format!("{}", now),
            name,
            enabled: true,
            schedule,
            payload: CronPayload {
                kind: "agent_turn".to_string(),
                message,
                deliver,
                channel,
                to,
            },
            state: CronJobState {
                next_run_at_ms: next_run,
                last_run_at_ms: None,
                last_status: None,
                last_error: None,
            },
            created_at_ms: now,
            updated_at_ms: now,
            delete_after_run: false,
        };

        {
            let mut store = self.store.write().unwrap();
            store.jobs.push(job.clone());
        }
        self.save_store()?;
        Ok(job)
    }

    pub fn remove_job(&self, job_id: &str) -> bool {
        let removed = {
            let mut store = self.store.write().unwrap();
            let before = store.jobs.len();
            store.jobs.retain(|j| j.id != job_id);
            store.jobs.len() < before
        };
        if removed {
            let _ = self.save_store();
        }
        removed
    }

    pub fn enable_job(&self, job_id: &str, enabled: bool) -> Option<CronJob> {
        let job = {
            let mut store = self.store.write().unwrap();
            let now = now_ms();
            if let Some(job) = store.jobs.iter_mut().find(|j| j.id == job_id) {
                job.enabled = enabled;
                job.updated_at_ms = now;
                if enabled {
                    job.state.next_run_at_ms = Self::compute_next_run(&job.schedule, now);
                } else {
                    job.state.next_run_at_ms = None;
                }
                Some(job.clone())
            } else {
                None
            }
        };
        if job.is_some() {
            let _ = self.save_store();
        }
        job
    }

    pub fn list_jobs(&self, include_disabled: bool) -> Vec<CronJob> {
        let store = self.store.read().unwrap();
        if include_disabled {
            store.jobs.clone()
        } else {
            store.jobs.iter().filter(|j| j.enabled).cloned().collect()
        }
    }

    /// Start the cron tick loop — checks every 10 seconds for due jobs and fires them.
    pub fn start_loop(self: Arc<Self>, bus: Arc<MessageBus>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("Cron tick loop started (10s interval)");
            let mut ticker = interval(Duration::from_secs(10));

            loop {
                ticker.tick().await;
                self.tick(&bus);
            }
        })
    }

    /// Check all enabled jobs and fire any that are due.
    fn tick(&self, bus: &Arc<MessageBus>) {
        let now = now_ms();
        let mut to_delete = Vec::new();

        {
            let mut store = self.store.write().unwrap();
            for job in store.jobs.iter_mut() {
                if !job.enabled {
                    continue;
                }

                let due = match job.state.next_run_at_ms {
                    Some(next) => next <= now,
                    None => false,
                };

                if !due {
                    continue;
                }

                info!(job_id = %job.id, job_name = %job.name, "Firing cron job");

                // Build and publish message
                let session_key = job
                    .payload
                    .channel
                    .as_deref()
                    .map(|ch| format!("cron:{}", ch))
                    .unwrap_or_else(|| format!("cron:{}", job.id));

                let mut msg = Message::new(
                    "cron",
                    &session_key,
                    Role::User,
                    &job.payload.message,
                ).with_sender("system");
                
                msg.metadata.insert("cron_job_id".to_string(), job.id.clone());

                if let Err(e) = bus.publish(Event::InboundMessage(msg)) {
                    error!(job_id = %job.id, "Failed to publish cron job: {}", e);
                    job.state.last_status = Some("error".to_string());
                    job.state.last_error = Some(e.to_string());
                } else {
                    job.state.last_status = Some("ok".to_string());
                    job.state.last_error = None;
                }

                job.state.last_run_at_ms = Some(now);
                job.updated_at_ms = now;

                // Compute next run
                job.state.next_run_at_ms = Self::compute_next_run(&job.schedule, now);

                if job.delete_after_run {
                    to_delete.push(job.id.clone());
                }
            }

            // Remove one-shot jobs
            if !to_delete.is_empty() {
                store.jobs.retain(|j| !to_delete.contains(&j.id));
            }
        }

        // Persist updated state
        let _ = self.save_store();
    }
}

