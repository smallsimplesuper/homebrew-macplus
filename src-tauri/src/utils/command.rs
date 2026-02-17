use std::process::{Command, Output};
use std::time::Duration;
use tokio::time::timeout;

use crate::utils::{AppError, AppResult};

/// Run a system command asynchronously with a timeout.
///
/// Spawns the command on a blocking thread via `tokio::task::spawn_blocking`
/// and wraps it with a timeout so a hung subprocess (e.g. `mas list`) can
/// never freeze the entire scan.
pub async fn run_command_with_timeout(
    program: &str,
    args: &[&str],
    timeout_secs: u64,
) -> AppResult<Output> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    let program_for_err = program.clone();
    let result = timeout(
        Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || Command::new(&program).args(&args).output()),
    )
    .await;

    match result {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(e))) => Err(AppError::CommandFailed(format!("{}: {}", program_for_err, e))),
        Ok(Err(e)) => Err(AppError::CommandFailed(format!("task join: {}", e))),
        Err(_) => Err(AppError::CommandFailed(format!(
            "{} timed out after {}s",
            program_for_err, timeout_secs
        ))),
    }
}
