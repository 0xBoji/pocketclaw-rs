use crate::sandbox::{SandboxConfig, truncate_output};
use crate::{Tool, ToolError};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::warn;

/// Patterns that are blocked from execution for safety.
const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "sudo ",
    "mkfs",
    "dd if=",
    ":(){",           // fork bomb
    "> /dev/",        // overwrite devices
    "chmod 777 /",
    "chown root",
    "pkill -9",
    "killall",
    "shutdown",
    "reboot",
    "init 0",
    "init 6",
    "format ",
    "fdisk",
];

pub struct ExecTool {
    sandbox: SandboxConfig,
}

impl ExecTool {
    pub fn new(sandbox: SandboxConfig) -> Self {
        Self { sandbox }
    }

    /// Check if a command matches any blocked pattern.
    fn is_command_blocked(command: &str) -> Option<&'static str> {
        let lower = command.to_lowercase();
        for pattern in BLOCKED_PATTERNS {
            if lower.contains(pattern) {
                return Some(pattern);
            }
        }
        None
    }
}

#[derive(Deserialize)]
struct ExecArgs {
    command: String,
}

#[async_trait]
impl Tool for ExecTool {
    fn name(&self) -> &str {
        "exec_cmd"
    }

    fn description(&self) -> &str {
        "Execute a shell command in the workspace."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        // Check if exec is enabled
        if !self.sandbox.exec_enabled {
            return Err(ToolError::ExecutionError(
                "Command execution is disabled by sandbox policy".to_string(),
            ));
        }

        let args: ExecArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // Block path traversal
        if args.command.contains("..") {
            return Err(ToolError::ExecutionError(
                "Command contains disallowed '..' sequence".to_string(),
            ));
        }

        // Block dangerous commands
        if let Some(pattern) = Self::is_command_blocked(&args.command) {
            warn!(
                command = %args.command,
                pattern = %pattern,
                "Blocked dangerous command"
            );
            return Err(ToolError::ExecutionError(format!(
                "Command blocked by security policy (matched: '{}')",
                pattern
            )));
        }

        let deadline = Duration::from_secs(self.sandbox.exec_timeout_secs);
        let workspace = self.sandbox.workspace_path.to_string_lossy().to_string();

        // Execute with timeout
        let result = timeout(deadline, async {
            Command::new("sh")
                .arg("-c")
                .arg(&args.command)
                .current_dir(&workspace)
                .output()
                .await
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&format!("STDOUT:\n{}\n", stdout));
                }
                if !stderr.is_empty() {
                    result.push_str(&format!("STDERR:\n{}\n", stderr));
                }

                if result.is_empty() {
                    Ok("(no output)".to_string())
                } else {
                    // Truncate output to max_output_bytes
                    Ok(truncate_output(&result, self.sandbox.max_output_bytes))
                }
            }
            Ok(Err(e)) => Err(ToolError::ExecutionError(e.to_string())),
            Err(_) => Err(ToolError::ExecutionError(format!(
                "Command timed out after {}s",
                self.sandbox.exec_timeout_secs
            ))),
        }
    }
}
