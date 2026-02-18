use crate::utils::http_client::APP_USER_AGENT;

/// Extract the latest version for a given app key or bundle ID from macadmins.software/latest.xml.
/// Returns (version, download_url) if found.
pub async fn check_macadmins_version(
    app_key: &str,
    bundle_id: &str,
    client: &reqwest::Client,
) -> Option<String> {
    let resp = client
        .get("https://macadmins.software/latest.xml")
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        log::info!(
            "macadmins feed: fetch returned status {} for {}",
            resp.status(),
            bundle_id
        );
        return None;
    }

    let xml_text = resp.text().await.ok()?;
    extract_version_from_xml(&xml_text, app_key, bundle_id)
}

/// Extract the latest version for a given app key from the macadmins.software XML.
/// Matches by <title> containing app_key or by <cfbundleidentifier> matching bundle_id.
fn extract_version_from_xml(xml: &str, app_key: &str, bundle_id: &str) -> Option<String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let app_key_lower = app_key.to_lowercase();
    let bundle_id_lower = bundle_id.to_lowercase();
    let mut in_package = false;
    let mut current_title = String::new();
    let mut current_version = String::new();
    let mut current_cfbundle = String::new();
    let mut reading_title = false;
    let mut reading_version = false;
    let mut reading_cfbundle = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "package" => {
                        in_package = true;
                        current_title.clear();
                        current_version.clear();
                        current_cfbundle.clear();
                    }
                    "title" if in_package => reading_title = true,
                    "version" if in_package => reading_version = true,
                    "cfbundleidentifier" if in_package => reading_cfbundle = true,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if reading_title {
                    current_title = match e.decode() {
                        Ok(s) => s.trim().to_string(),
                        Err(err) => {
                            log::warn!("macadmins feed: failed to decode <title>: {}", err);
                            String::new()
                        }
                    };
                    reading_title = false;
                } else if reading_version {
                    current_version = match e.decode() {
                        Ok(s) => s.trim().to_string(),
                        Err(err) => {
                            log::warn!("macadmins feed: failed to decode <version>: {}", err);
                            String::new()
                        }
                    };
                    reading_version = false;
                } else if reading_cfbundle {
                    current_cfbundle = match e.decode() {
                        Ok(s) => s.trim().to_string(),
                        Err(err) => {
                            log::warn!("macadmins feed: failed to decode <cfbundleidentifier>: {}", err);
                            String::new()
                        }
                    };
                    reading_cfbundle = false;
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "package" && in_package {
                    let title_match = current_title.to_lowercase().contains(&app_key_lower);
                    let bundle_match = !current_cfbundle.is_empty()
                        && current_cfbundle.to_lowercase() == bundle_id_lower;

                    if (title_match || bundle_match) && !current_version.is_empty() {
                        return Some(current_version.clone());
                    }
                    in_package = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    None
}
