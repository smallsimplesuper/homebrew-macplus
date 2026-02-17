use std::process::{Command, Output};

use crate::utils::askpass;

/// Error type for elevated command execution.
#[derive(Debug)]
pub enum ElevatedError {
    UserCancelled,
    IoError(std::io::Error),
    CommandFailed(String),
}

impl std::fmt::Display for ElevatedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElevatedError::UserCancelled => write!(f, "User cancelled the password dialog"),
            ElevatedError::IoError(e) => write!(f, "IO error: {}", e),
            ElevatedError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
        }
    }
}

impl From<std::io::Error> for ElevatedError {
    fn from(e: std::io::Error) -> Self {
        ElevatedError::IoError(e)
    }
}

/// Pre-authenticate with sudo by running `sudo -A -v`.
///
/// Shows the askpass password dialog once and establishes a sudo timestamp
/// so subsequent `sudo -A` calls succeed silently. Returns `true` if
/// authentication succeeded, `false` if the user cancelled or askpass is
/// unavailable.
pub fn pre_authenticate() -> bool {
    let ap = match askpass::askpass_path() {
        Some(p) => p,
        None => return false,
    };

    let output = Command::new("sudo")
        .current_dir("/tmp")
        .env("SUDO_ASKPASS", ap)
        .args(["-A", "-v"])
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Refresh the sudo timestamp non-interactively. Returns `true` if the
/// timestamp was still valid and got extended.
pub fn refresh_timestamp() -> bool {
    Command::new("sudo")
        .current_dir("/tmp")
        .args(["-n", "-v"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a command with elevated privileges.
///
/// 1. Tries `sudo -A <program> <args>` first (benefits from pre-warmed
///    timestamp — no dialog if recently authenticated).
/// 2. Falls back to `osascript ... with administrator privileges` if sudo
///    fails for non-cancellation reasons.
///
/// Returns the command `Output` on success, or `ElevatedError`.
pub fn run_elevated(program: &str, args: &[&str]) -> Result<Output, ElevatedError> {
    // 1. Try sudo -A (benefits from pre-warmed timestamp)
    if let Some(ap) = askpass::askpass_path() {
        let output = Command::new("sudo")
            .current_dir("/tmp")
            .env("SUDO_ASKPASS", ap)
            .arg("-A")
            .arg(program)
            .args(args)
            .output()?;

        if output.status.success() {
            return Ok(output);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        // If user cancelled the askpass dialog, don't fall through
        if stderr.contains("cancelled")
            || stderr.contains("dialog was dismissed")
            || stderr.contains("User canceled")
        {
            return Err(ElevatedError::UserCancelled);
        }
    }

    // 2. Fallback: osascript with administrator privileges
    let shell_cmd = build_shell_command(program, args);
    run_osascript_elevated(&shell_cmd)
}

/// Run a compound shell expression with elevated privileges.
///
/// Like `run_elevated` but wraps the command in `sudo -A sh -c "..."` for
/// cases where the command is a pipeline or uses `&&`.
pub fn run_elevated_shell(shell_cmd: &str) -> Result<Output, ElevatedError> {
    // 1. Try sudo -A sh -c "..."
    if let Some(ap) = askpass::askpass_path() {
        let output = Command::new("sudo")
            .current_dir("/tmp")
            .env("SUDO_ASKPASS", ap)
            .args(["-A", "sh", "-c", shell_cmd])
            .output()?;

        if output.status.success() {
            return Ok(output);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("cancelled")
            || stderr.contains("dialog was dismissed")
            || stderr.contains("User canceled")
        {
            return Err(ElevatedError::UserCancelled);
        }
    }

    // 2. Fallback: osascript with administrator privileges
    run_osascript_elevated(shell_cmd)
}

/// Build a shell-safe command string from a program and its arguments.
fn build_shell_command(program: &str, args: &[&str]) -> String {
    let mut parts = vec![shell_escape(program)];
    for arg in args {
        parts.push(shell_escape(arg));
    }
    parts.join(" ")
}

/// Escape a string for use inside a shell command.
fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '-' || c == '_') {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Run a shell command via osascript with administrator privileges.
fn run_osascript_elevated(shell_cmd: &str) -> Result<Output, ElevatedError> {
    let output = Command::new("osascript")
        .current_dir("/tmp")
        .args([
            "-e",
            &format!(
                "do shell script \"{}\" with administrator privileges",
                shell_cmd.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("-128") {
            return Err(ElevatedError::UserCancelled);
        }
    }

    Ok(output)
}
