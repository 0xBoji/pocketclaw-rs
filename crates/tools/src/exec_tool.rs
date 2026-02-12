use std::os::unix::process::CommandExt;
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
        
        // Clone config for closure
        let start_config = self.sandbox.clone();

        let mut cmd = Command::new("sh");
        
        cmd.arg("-c")
            .arg(&args.command)
            .current_dir(&workspace)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .process_group(0);

        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(move || {
                // RLIMIT_NPROC
                if let Some(limit) = start_config.max_child_processes {
                    let rlim = libc::rlimit {
                        rlim_cur: limit as libc::rlim_t,
                        rlim_max: limit as libc::rlim_t,
                    };
                    libc::setrlimit(libc::RLIMIT_NPROC, &rlim);
                }

                // RLIMIT_NOFILE
                if let Some(limit) = start_config.max_open_files {
                     let rlim = libc::rlimit {
                        rlim_cur: limit as libc::rlim_t,
                        rlim_max: limit as libc::rlim_t,
                    };
                    libc::setrlimit(libc::RLIMIT_NOFILE, &rlim);
                }

                // RLIMIT_CPU
                if let Some(limit) = start_config.cpu_time_limit_secs {
                     let rlim = libc::rlimit {
                        rlim_cur: limit as libc::rlim_t,
                        rlim_max: limit as libc::rlim_t,
                    };
                    libc::setrlimit(libc::RLIMIT_CPU, &rlim);
                }

                Ok(())
            });
        }

        let mut child = cmd.spawn()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to spawn: {}", e)))?;

        let pid = child.id().ok_or_else(|| ToolError::ExecutionError("Failed to get child PID".to_string()))?;

        let result = timeout(deadline, child.wait_with_output()).await;

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
                    Ok(truncate_output(&result, self.sandbox.max_output_bytes))
                }
            }
            Ok(Err(e)) => Err(ToolError::ExecutionError(e.to_string())),
            Err(_) => {
                // Timeout: kill the entire process group
                #[cfg(unix)]
                {
                    // Kill process group (negative PID = group)
                    unsafe {
                        // libc::killpg behaves differently on potential failure, but we try anyway
                        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
                    }
                }
                #[cfg(not(unix))]
                {
                    // Fallback for non-Unix (best effort)
                    // We can't use child.kill() here because child is moved.
                    // We would need to use system command taskkill or similar if we really wanted to.
                    // For now, accept that child is dropped and might linger if not perfectly handled by OS.
                    // Actually, since child is dropped when future is cancelled, standard library doesn't kill it.
                    // But we don't have handle.
                }

                Err(ToolError::ExecutionError(format!(
                    "Command timed out after {}s (process group killed)",
                    self.sandbox.exec_timeout_secs
                )))
            }
        }
    }
}
