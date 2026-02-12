use std::path::{Path, PathBuf};
use crate::ToolError;

/// Central sandbox configuration for all tools.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// All file operations are confined to this directory.
    pub workspace_path: PathBuf,
    /// Maximum execution time for shell commands (seconds).
    pub exec_timeout_secs: u64,
    /// Maximum combined stdout+stderr size (bytes).
    pub max_output_bytes: usize,
    /// Whether exec_cmd is allowed at all.
    pub exec_enabled: bool,
    /// Allowed domains for web_fetch / web_search. Empty = allow all.
    pub network_allowlist: Vec<String>,
    /// Max number of child processes (RLIMIT_NPROC).
    pub max_child_processes: Option<u64>,
    /// Max number of open file descriptors (RLIMIT_NOFILE).
    pub max_open_files: Option<u64>,
    /// Max CPU time in seconds (RLIMIT_CPU).
    pub cpu_time_limit_secs: Option<u64>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            workspace_path: PathBuf::from("workspace"),
            exec_timeout_secs: 30,
            max_output_bytes: 64 * 1024, // 64 KB
            exec_enabled: true,
            network_allowlist: Vec::new(),
            max_child_processes: Some(50),
            max_open_files: Some(1024),
            cpu_time_limit_secs: Some(30),
        }
    }
}

/// Validate that a requested path is within the workspace boundary.
///
/// Security model:
/// 1. Resolve to absolute path
/// 2. Canonicalize to resolve ALL symlinks and `..` components
/// 3. Verify the canonical path starts with the canonical workspace
/// 4. Reject symlinks that point outside workspace
///
/// This prevents symlink-based escapes and path traversal attacks.
pub fn validate_path(workspace: &Path, requested: &str) -> Result<PathBuf, ToolError> {
    let requested_path = Path::new(requested);

    // Build absolute path
    let absolute = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        workspace.join(requested_path)
    };

    // Canonicalize workspace first (this is our trust anchor)
    let workspace_canonical = workspace.canonicalize().map_err(|e| {
        ToolError::ExecutionError(format!("Failed to resolve workspace: {}", e))
    })?;

    // For existing paths: full canonicalize (resolves ALL symlinks + ..)
    if absolute.exists() {
        // Check if the final path component is a symlink pointing outside
        let meta = std::fs::symlink_metadata(&absolute).map_err(|e| {
            ToolError::ExecutionError(format!("Failed to read metadata: {}", e))
        })?;

        let resolved = absolute.canonicalize().map_err(|e| {
            ToolError::ExecutionError(format!("Failed to resolve path: {}", e))
        })?;

        if !resolved.starts_with(&workspace_canonical) {
            // If it's a symlink, give a specific error
            if meta.file_type().is_symlink() {
                return Err(ToolError::ExecutionError(format!(
                    "Access denied: symlink '{}' points outside workspace",
                    requested
                )));
            }
            return Err(ToolError::ExecutionError(format!(
                "Access denied: path '{}' is outside workspace '{}'",
                requested, workspace.display()
            )));
        }

        return Ok(resolved);
    }

    // For non-existing paths: walk up to first existing ancestor,
    // canonicalize THAT, verify prefix, then append remaining components.
    let mut existing_ancestor = absolute.clone();
    let mut remaining_parts: Vec<std::ffi::OsString> = Vec::new();

    loop {
        if existing_ancestor.exists() {
            break;
        }
        if let Some(file_name) = existing_ancestor.file_name() {
            remaining_parts.push(file_name.to_os_string());
        } else {
            return Err(ToolError::ExecutionError(
                "Invalid path: cannot resolve ancestry".to_string(),
            ));
        }
        if !existing_ancestor.pop() {
            return Err(ToolError::ExecutionError(
                "Invalid path: no existing ancestor found".to_string(),
            ));
        }
    }

    // Canonicalize the existing ancestor (resolves symlinks)
    let ancestor_canonical = existing_ancestor.canonicalize().map_err(|e| {
        ToolError::ExecutionError(format!("Failed to resolve ancestor: {}", e))
    })?;

    // Verify ancestor is within workspace
    if !ancestor_canonical.starts_with(&workspace_canonical) {
        return Err(ToolError::ExecutionError(format!(
            "Access denied: path '{}' is outside workspace '{}'",
            requested, workspace.display()
        )));
    }

    // Rebuild the full path from canonical ancestor + remaining parts
    let mut result = ancestor_canonical;
    for part in remaining_parts.into_iter().rev() {
        // Reject any component that is ".." (redundant safety check)
        if part == ".." {
            return Err(ToolError::ExecutionError(
                "Path traversal ('..') is not allowed".to_string(),
            ));
        }
        result.push(part);
    }

    Ok(result)
}

/// Check if an IP address is in a private/reserved range (SSRF protection).
pub fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()                                      // 127.0.0.0/8
                || v4.is_private()                                // 10/8, 172.16/12, 192.168/16
                || v4.is_link_local()                             // 169.254.0.0/16
                || v4.is_broadcast()                              // 255.255.255.255
                || v4.is_unspecified()                             // 0.0.0.0
                || *v4 == Ipv4Addr::new(169, 254, 169, 254)       // AWS metadata
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64 // 100.64/10 (CGNAT)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()                                      // ::1
                || v6.is_unspecified()                             // ::
                || {
                    let segments = v6.segments();
                    segments[0] & 0xfe00 == 0xfc00                // fc00::/7 (ULA)
                        || segments[0] == 0xfe80                   // fe80::/10 (link-local)
                        || *v6 == Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 1) // ::ffff:127.0.0.1
                }
        }
    }
}

/// Truncate a string to max_bytes, appending a notice if truncated.
pub fn truncate_output(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        output.to_string()
    } else {
        let truncated = &output[..max_bytes];
        format!("{}\n\n--- OUTPUT TRUNCATED ({}B limit) ---", truncated, max_bytes)
    }
}
