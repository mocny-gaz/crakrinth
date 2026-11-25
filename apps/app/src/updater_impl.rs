use crate::api::Result;
use std::sync::{Arc, Mutex};
use tauri::http::HeaderValue;
use tauri::http::header::ACCEPT;
use tauri::{Manager, ResourceId, Runtime, Webview};
use tauri_plugin_http::reqwest;
use tauri_plugin_http::reqwest::ClientBuilder;
use tauri_plugin_updater::Error;
use tauri_plugin_updater::Update;
use tauri_plugin_updater::UpdaterExt;
use theseus::{
    LoadingBarType, emit_loading, init_loading, launcher_user_agent,
};
use tokio::time::Instant;

#[derive(Default)]
pub struct PendingUpdateData(pub Mutex<Option<(Arc<Update>, Vec<u8>)>>);

#[derive(serde::Serialize)]
pub struct AppVersionUpdate {
    pub rid: ResourceId,
    pub version: String,
}

fn other_error<T>(message: String) -> Result<T> {
    let error: theseus::Error = theseus::ErrorKind::OtherError(message).into();
    Err(error.into())
}

const RELEASE_URL_PREFIX: &str =
    "https://github.com/mocny-gaz/crakrinth/releases/download/";

/// Looks up one specific release of the fork through the `latest.json` asset
/// URL that GitHub lists for it. Any version other than the current one is
/// accepted, so this covers downgrades, upgrades and newer fork builds of the
/// current version.
#[tauri::command]
pub async fn check_app_version<R: Runtime>(
    webview: Webview<R>,
    url: String,
) -> Result<AppVersionUpdate> {
    if !url.starts_with(RELEASE_URL_PREFIX)
        || !url.ends_with("/latest.json")
        || url.contains("..")
    {
        return other_error(format!("Invalid release URL: {url}"));
    }

    let Ok(url) = url.parse::<tauri::Url>() else {
        return other_error("Invalid release URL".to_string());
    };

    let Some(update) = webview
        .updater_builder()
        .endpoints(vec![url])?
        .version_comparator(|current, update| update.version != current)
        .build()?
        .check()
        .await?
    else {
        return other_error("This version is already installed".to_string());
    };

    Ok(AppVersionUpdate {
        version: update.version.clone(),
        rid: webview.resources_table().add(update),
    })
}

// Reimplementation of Update::download mostly, minus the actual download part
#[tauri::command]
pub async fn get_update_size<R: Runtime>(
    webview: Webview<R>,
    rid: ResourceId,
) -> Result<Option<u64>> {
    let update = webview.resources_table().get::<Update>(rid)?;

    let mut headers = update.headers.clone();
    if !headers.contains_key(ACCEPT) {
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/octet-stream"),
        );
    }

    let mut request = ClientBuilder::new().user_agent(launcher_user_agent());
    if let Some(timeout) = update.timeout {
        request = request.timeout(timeout);
    }
    if let Some(ref proxy) = update.proxy {
        let proxy = reqwest::Proxy::all(proxy.as_str())?;
        request = request.proxy(proxy);
    }
    let response = request
        .build()?
        .head(update.download_url.clone())
        .headers(headers)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Error::Network(format!(
            "Download request failed with status: {}",
            response.status()
        ))
        .into());
    }

    let content_length = response
        .headers()
        .get("Content-Length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok());

    Ok(content_length)
}

#[tauri::command]
pub async fn enqueue_update_for_installation<R: Runtime>(
    webview: Webview<R>,
    rid: ResourceId,
) -> Result<()> {
    let pending_data = webview.state::<PendingUpdateData>().inner();

    let update = webview.resources_table().get::<Update>(rid)?;

    let progress = init_loading(
        LoadingBarType::LauncherUpdate {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
        },
        1.0,
        "Downloading update...",
    )
    .await?;

    let download_start = Instant::now();
    let update_data = update
        .download(
            |chunk_size, total_size| {
                let Some(total_size) = total_size else {
                    return;
                };
                if let Err(e) = emit_loading(
                    &progress,
                    chunk_size as f64 / total_size as f64,
                    None,
                ) {
                    tracing::error!(
                        "Failed to update download progress bar: {e}"
                    );
                }
            },
            || {},
        )
        .await?;
    let download_duration = download_start.elapsed();
    tracing::info!("Downloaded update in {download_duration:?}");

    pending_data
        .0
        .lock()
        .unwrap()
        .replace((update, update_data));

    Ok(())
}

#[tauri::command]
pub fn remove_enqueued_update<R: Runtime>(webview: Webview<R>) {
    let pending_data = webview.state::<PendingUpdateData>().inner();
    pending_data.0.lock().unwrap().take();
}
