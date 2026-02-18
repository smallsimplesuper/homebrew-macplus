pub mod app_lifecycle;
pub mod askpass;
pub mod brew;
pub mod command;
pub mod error;
pub mod http_client;
pub mod plist_parser;
pub mod sanitize;
pub mod sudo_session;

pub use error::{AppError, AppResult};

/// Browser extension bundle ID prefixes (Chrome, Brave, Edge, Chromium, Arc, Firefox, Opera, Vivaldi)
const BROWSER_EXTENSION_PREFIXES: &[&str] = &[
    "com.google.Chrome.app.",
    "com.brave.Browser.app.",
    "com.microsoft.Edge.app.",
    "org.chromium.Chromium.app.",
];

/// Returns true if the bundle ID belongs to a browser extension.
/// Browser extensions should not be matched against Homebrew casks.
pub fn is_browser_extension(bundle_id: &str) -> bool {
    BROWSER_EXTENSION_PREFIXES
        .iter()
        .any(|p| bundle_id.starts_with(p))
}

/// Check whether Xcode Command Line Tools are installed.
/// Uses spawn + poll + kill pattern to avoid hanging if xcode-select blocks.
pub fn is_xcode_clt_installed() -> bool {
    let mut child = match std::process::Command::new("xcode-select")
        .arg("-p")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(_) => return false,
        }
    }
}
