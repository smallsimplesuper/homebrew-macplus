use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use futures::StreamExt;

use crate::models::UpdateResult;
use crate::utils::{AppError, AppResult};
use super::UpdateExecutor;

pub struct SparkleExecutor {
    download_url: String,
    app_name: String,
    source_type: String,
}

impl SparkleExecutor {
    pub fn new(download_url: String, app_name: String) -> Self {
        Self { download_url, app_name, source_type: "sparkle".to_string() }
    }

    pub fn with_source_type(mut self, source_type: &str) -> Self {
        self.source_type = source_type.to_string();
        self
    }
}

impl UpdateExecutor for SparkleExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        let tmp_dir = tempfile::tempdir()
            .map_err(|e| AppError::CommandFailed(format!("Failed to create temp dir: {}", e)))?;

        // 1. Download the file
        on_progress(2, "Requesting download...", None);

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| AppError::CommandFailed(format!("Failed to create HTTP client: {}", e)))?;

        let response = client.get(&self.download_url).send().await
            .map_err(|e| AppError::CommandFailed(format!("Download failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(format!("Download returned HTTP {}", response.status())),
                source_type: self.source_type.clone(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            });
        }

        // Capture Content-Type before consuming the response
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        // Reject HTML/text responses — these aren't installer files
        if content_type.contains("text/html") || content_type.contains("text/plain") {
            return Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some("Download URL returned HTML instead of an installer file".to_string()),
                source_type: self.source_type.clone(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            });
        }

        // Determine filename from URL or Content-Disposition
        let filename = response
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.split("filename=").nth(1).map(|f| f.trim_matches('"').to_string())
            })
            .unwrap_or_else(|| {
                self.download_url
                    .split('/')
                    .last()
                    .unwrap_or("update")
                    .split('?')
                    .next()
                    .unwrap_or("update")
                    .to_string()
            });

        let total_bytes = response.content_length();
        let download_path = tmp_dir.path().join(&filename);
        let mut file = std::fs::File::create(&download_path)
            .map_err(|e| AppError::CommandFailed(format!("Failed to create download file: {}", e)))?;
        let mut downloaded: u64 = 0;
        let mut last_emit = Instant::now();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| AppError::CommandFailed(format!("Download stream error: {}", e)))?;
            file.write_all(&chunk)
                .map_err(|e| AppError::CommandFailed(format!("Failed to write chunk: {}", e)))?;
            downloaded += chunk.len() as u64;

            if last_emit.elapsed() >= Duration::from_millis(150) {
                last_emit = Instant::now();
                let pct = total_bytes
                    .map(|t| ((downloaded as f64 / t as f64) * 100.0) as u8)
                    .unwrap_or(0);
                let mapped = 5 + (pct as u16 * 45 / 100) as u8;
                on_progress(
                    mapped,
                    &format!("Downloading update for {}", self.app_name),
                    Some((downloaded, total_bytes)),
                );
            }
        }
        drop(file);

        on_progress(50, "Download complete, extracting...", None);

        // 2. Detect file type using Content-Type header, then filename extension, then magic bytes
        let mut magic_buf = [0u8; 16];
        let magic_len = {
            let mut f = std::fs::File::open(&download_path)
                .map_err(|e| AppError::CommandFailed(format!("Failed to reopen download: {}", e)))?;
            f.read(&mut magic_buf)
                .map_err(|e| AppError::CommandFailed(format!("Failed to read magic bytes: {}", e)))?
        };
        let file_type = detect_file_type(&content_type, &filename, &magic_buf[..magic_len]);

        let new_app_path = match file_type {
            FileType::Dmg => extract_from_dmg(&download_path, tmp_dir.path(), on_progress, &self.app_name)?,
            FileType::Zip => extract_from_zip(&download_path, tmp_dir.path())?,
            FileType::Pkg => {
                on_progress(60, "Installing package (requesting admin privileges)...", None);

                let dl_path_str = download_path.to_string_lossy().to_string();
                let pkg_args: Vec<&str> = vec!["-pkg", &dl_path_str, "-target", "/"];
                match crate::utils::sudo_session::run_elevated("/usr/sbin/installer", &pkg_args) {
                    Ok(pkg_output) if pkg_output.status.success() => {
                        on_progress(100, &format!("{} installed successfully", self.app_name), None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: true,
                            message: Some(format!("{} installed successfully via PKG", self.app_name)),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                        let msg = "Update cancelled \u{2014} administrator approval is required to install this package".to_string();
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Ok(pkg_output) => {
                        let pkg_stderr = String::from_utf8_lossy(&pkg_output.stderr).to_string();
                        let msg = format!("Package installation failed: {}", pkg_stderr);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(e) => {
                        let msg = format!("Failed to request admin privileges: {}", e);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                }
            }
            FileType::Unknown => {
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: false,
                    message: Some(format!("Unsupported archive format: {}", filename)),
                    source_type: self.source_type.clone(),
                    from_version: None,
                    to_version: None,
                    handled_relaunch: false,
                    delegated: false,
                });
            }
        };

        // 3. Check if app is running and quit gracefully before replacing
        let was_running = crate::utils::app_lifecycle::is_app_running(bundle_id);
        if was_running {
            on_progress(60, &format!("\u{26a0} {} is open \u{2014} closing to update...", self.app_name), None);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            on_progress(65, &format!("Quitting {}", self.app_name), None);
            crate::utils::app_lifecycle::quit_app_gracefully(&self.app_name, bundle_id);
        } else {
            on_progress(65, &format!("Preparing to replace {}", self.app_name), None);
        }

        on_progress(75, &format!("Replacing {}", self.app_name), None);

        // 4. Replace the app bundle
        let dest = Path::new(app_path);
        if dest.exists() {
            // Move old app to trash instead of deleting (safer)
            let trash_result = Command::new("osascript")
                .current_dir("/tmp")
                .args([
                    "-e",
                    &format!(
                        "tell application \"Finder\" to move POSIX file \"{}\" to trash",
                        app_path
                    ),
                ])
                .output();

            if trash_result.is_err() || !trash_result.unwrap().status.success() {
                // Fallback: remove directly
                let _ = std::fs::remove_dir_all(dest);
            }
        }

        let cp_output = Command::new("cp")
            .current_dir("/tmp")
            .args(["-R", &new_app_path.to_string_lossy(), app_path])
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to copy app: {}", e)))?;

        if !cp_output.status.success() {
            let stderr = String::from_utf8_lossy(&cp_output.stderr);
            let needs_elevation = stderr.contains("Permission denied")
                || stderr.contains("Operation not permitted");

            if needs_elevation {
                // Retry with administrator privileges
                on_progress(80, "Requesting administrator privileges...", None);

                let elevated_cmd = format!(
                    "rm -rf '{}' && cp -R '{}' '{}'",
                    app_path.replace('\'', "'\\''"),
                    new_app_path.to_string_lossy().replace('\'', "'\\''"),
                    app_path.replace('\'', "'\\''"),
                );

                match crate::utils::sudo_session::run_elevated_shell(&elevated_cmd) {
                    Ok(out) if out.status.success() => {
                        // Elevated copy succeeded — continue to quarantine removal + relaunch
                    }
                    Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                        let msg = "Update cancelled \u{2014} administrator approval is required to replace this app".to_string();
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Ok(out) => {
                        let osa_stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        let msg = format!("Failed to replace app (elevated): {}", osa_stderr);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(e) => {
                        let msg = format!("Failed to request admin privileges: {}", e);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: self.source_type.clone(),
                            from_version: None,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                }
            } else {
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: false,
                    message: Some(format!("Failed to replace app: {}", stderr)),
                    source_type: self.source_type.clone(),
                    from_version: None,
                    to_version: None,
                    handled_relaunch: false,
                    delegated: false,
                });
            }
        }

        // Remove quarantine attribute (best-effort, try elevated if needed)
        let xattr_output = Command::new("xattr")
            .current_dir("/tmp")
            .args(["-rd", "com.apple.quarantine", app_path])
            .output();
        if let Ok(ref out) = xattr_output {
            if !out.status.success() {
                // Try elevated quarantine removal
                let _ = crate::utils::sudo_session::run_elevated(
                    "xattr",
                    &["-rd", "com.apple.quarantine", app_path],
                );
            }
        }

        // Relaunch if the app was running before the update
        if was_running {
            on_progress(95, &format!("Relaunching {}", self.app_name), None);
            crate::utils::app_lifecycle::relaunch_app(app_path);
        }

        on_progress(100, &format!("{} updated successfully", self.app_name), None);

        Ok(UpdateResult {
            bundle_id: bundle_id.to_string(),
            success: true,
            message: Some(format!("{} updated successfully via direct download", self.app_name)),
            source_type: self.source_type.clone(),
            from_version: None,
            to_version: None,
            handled_relaunch: was_running,
            delegated: false,
        })
    }
}

pub(crate) fn extract_from_dmg(
    dmg_path: &Path,
    tmp_dir: &Path,
    on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    app_name: &str,
) -> AppResult<PathBuf> {
    let mount_point = tmp_dir.join("dmg_mount");
    std::fs::create_dir_all(&mount_point)
        .map_err(|e| AppError::CommandFailed(format!("Failed to create mount point: {}", e)))?;

    on_progress(52, &format!("Mounting disk image for {}...", app_name), None);

    // Use spawn + stdin pipe to auto-accept embedded license agreements
    let mut child = Command::new("hdiutil")
        .current_dir("/tmp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .args([
            "attach",
            "-nobrowse",
            "-noverify",
            "-noautoopen",
            "-mountpoint",
            &mount_point.to_string_lossy(),
            &dmg_path.to_string_lossy(),
        ])
        .spawn()
        .map_err(|e| AppError::CommandFailed(format!("Failed to mount DMG: {}", e)))?;

    // Write "Y\n" to accept any embedded license agreement
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"Y\n");
    }

    let output = child
        .wait_with_output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to mount DMG: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::CommandFailed(format!("hdiutil attach failed: {}", stderr)));
    }

    // Find the .app inside the mounted volume
    let app_path = find_app_in_dir(&mount_point)?;

    on_progress(60, &format!("Copying {} from disk image...", app_name), None);

    // Copy to a temp location before unmounting
    let dest = tmp_dir.join(app_path.file_name().unwrap_or_default());
    let cp_output = Command::new("cp")
        .current_dir("/tmp")
        .args(["-R", &app_path.to_string_lossy(), &dest.to_string_lossy()])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to copy from DMG: {}", e)))?;

    if !cp_output.status.success() {
        let stderr = String::from_utf8_lossy(&cp_output.stderr);
        let _ = Command::new("hdiutil")
            .current_dir("/tmp")
            .args(["detach", &mount_point.to_string_lossy(), "-quiet"])
            .output();
        return Err(AppError::CommandFailed(format!("cp from DMG failed: {}", stderr)));
    }

    on_progress(68, "Unmounting disk image...", None);

    // Unmount
    let _ = Command::new("hdiutil")
        .current_dir("/tmp")
        .args(["detach", &mount_point.to_string_lossy(), "-quiet"])
        .output();

    Ok(dest)
}

fn extract_from_zip(zip_path: &Path, tmp_dir: &Path) -> AppResult<PathBuf> {
    let extract_dir = tmp_dir.join("zip_extract");
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| AppError::CommandFailed(format!("Failed to create extract dir: {}", e)))?;

    let output = Command::new("ditto")
        .current_dir("/tmp")
        .args(["-xk", &zip_path.to_string_lossy(), &extract_dir.to_string_lossy()])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to extract zip: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::CommandFailed(format!("ditto extract failed: {}", stderr)));
    }

    find_app_in_dir(&extract_dir)
}

pub(crate) fn find_app_in_dir(dir: &Path) -> AppResult<PathBuf> {
    // Look for .app bundles at the top level
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("app") {
                return Ok(path);
            }
        }
    }

    // Look one level deeper
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(&sub) {
                    for sub_entry in sub_entries.flatten() {
                        let path = sub_entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("app") {
                            return Ok(path);
                        }
                    }
                }
            }
        }
    }

    Err(AppError::NotFound("No .app bundle found in archive".to_string()))
}

#[derive(Debug, PartialEq)]
pub(crate) enum FileType {
    Dmg,
    Zip,
    Pkg,
    Unknown,
}

/// Detect file type using Content-Type header, filename extension, and magic bytes (in that order).
pub(crate) fn detect_file_type(content_type: &str, filename: &str, bytes: &[u8]) -> FileType {
    // 1. Check Content-Type header (skip generic types)
    if !content_type.is_empty() && content_type != "application/octet-stream" {
        if content_type.contains("apple-diskimage") || content_type.contains("x-diskcopy") {
            return FileType::Dmg;
        }
        if content_type.contains("zip") || content_type.contains("x-zip") {
            return FileType::Zip;
        }
        if content_type.contains("apple.installer") {
            return FileType::Pkg;
        }
    }

    // 2. Check filename extension
    let lower = filename.to_lowercase();
    if lower.ends_with(".dmg") {
        return FileType::Dmg;
    }
    if lower.ends_with(".zip") {
        return FileType::Zip;
    }
    if lower.ends_with(".pkg") {
        return FileType::Pkg;
    }

    // 3. Check magic bytes as ultimate fallback
    if bytes.len() >= 4 {
        // ZIP: starts with PK\x03\x04
        if bytes[0..4] == [0x50, 0x4B, 0x03, 0x04] {
            return FileType::Zip;
        }
        // DMG (koly trailer is at end, but compressed DMGs often start with bzip2 or zlib)
        // Check for bzip2 magic (BZ) which is common in compressed DMGs
        if bytes.len() >= 2 && bytes[0..2] == [0x42, 0x5A] {
            return FileType::Dmg;
        }
        // XAR archive (PKG files are XAR): starts with "xar!"
        if bytes[0..4] == [0x78, 0x61, 0x72, 0x21] {
            return FileType::Pkg;
        }
    }

    FileType::Unknown
}
