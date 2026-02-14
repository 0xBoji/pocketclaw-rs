use crate::{Tool, ToolError};
use async_trait::async_trait;
use phoneclaw_cron::{CronSchedule, CronService};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct CronTool {
    service: Arc<CronService>,
}

impl CronTool {
    pub fn new(service: Arc<CronService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn description(&self) -> &str {
        "Manage scheduled tasks and reminders (add, list, remove, status). 
         Use 'add' action with 'schedule' (kind: 'at' for one-shot or 'every' for interval) and 'message' to create a reminder.
         Times for 'at' should be in ms since epoch. 'every' interval should be in seconds."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "remove", "status"],
                    "description": "The action to perform."
                },
                "name": {
                    "type": "string",
                    "description": "Short name for the job (required for 'add')."
                },
                "message": {
                    "type": "string",
                    "description": "Message to send when the timer fires (required for 'add')."
                },
                "schedule": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["at", "every"] },
                        "atMs": { "type": "integer", "description": "Absolute timestamp in ms (for kind='at')" },
                        "every": { "type": "integer", "description": "Interval in seconds (for kind='every')" }
                    },
                    "description": "Schedule configuration (required for 'add')."
                },
                "jobId": {
                    "type": "string",
                    "description": "Job ID to remove (required for 'remove')."
                },
                "deliver": {
                    "type": "boolean",
                    "description": "Whether to deliver the response to the original channel. Default: false."
                },
                "channel": {
                    "type": "string",
                    "description": "Target channel for delivery."
                },
                "to": {
                    "type": "string",
                    "description": "Target recipient for delivery."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let action = args["action"].as_str().ok_or(ToolError::InvalidArgs("action required".into()))?;

        match action {
            "status" => {
                Ok(json!({ "status": "ok", "service": "CronService active" }).to_string())
            }
            "list" => {
                let jobs = self.service.list_jobs(true);
                Ok(serde_json::to_string_pretty(&jobs).map_err(|e| ToolError::ExecutionError(e.to_string()))?)
            }
            "remove" => {
                let id = args["jobId"].as_str().ok_or(ToolError::InvalidArgs("jobId required for remove".into()))?;
                if self.service.remove_job(id) {
                    Ok(format!("Job {} removed", id))
                } else {
                    Err(ToolError::ExecutionError(format!("Job {} not found", id)))
                }
            }
            "add" => {
                let name = args["name"].as_str().unwrap_or("unnamed reminder").to_string();
                let message = args["message"].as_str().ok_or(ToolError::InvalidArgs("message required for add".into()))?.to_string();
                let deliver = args["deliver"].as_bool().unwrap_or(false);
                let channel = args["channel"].as_str().map(|s| s.to_string());
                let to = args["to"].as_str().map(|s| s.to_string());

                let schedule_val = &args["schedule"];
                let kind = schedule_val["kind"].as_str().ok_or(ToolError::InvalidArgs("schedule.kind required".into()))?;
                
                let schedule = match kind {
                    "at" => {
                        let at_ms = schedule_val["atMs"].as_i64().ok_or(ToolError::InvalidArgs("schedule.atMs required for kind='at'".into()))?;
                        CronSchedule {
                            kind: "at".to_string(),
                            at_ms: Some(at_ms),
                            every_ms: None,
                            expr: None,
                        }
                    }
                    "every" => {
                        let every_secs = schedule_val["every"].as_i64().ok_or(ToolError::InvalidArgs("schedule.every required for kind='every'".into()))?;
                        CronSchedule {
                            kind: "every".to_string(),
                            at_ms: None,
                            every_ms: Some(every_secs * 1000),
                            expr: None,
                        }
                    }
                    _ => return Err(ToolError::InvalidArgs(format!("Unsupported schedule kind: {}", kind))),
                };

                let job = self.service.add_job(name, schedule, message, deliver, channel, to)
                    .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

                Ok(format!("Reminder scheduled successfully. Job ID: {}", job.id))
            }
            _ => Err(ToolError::InvalidArgs(format!("Unknown action: {}", action))),
        }
    }
}
