use futures_util::StreamExt;
use reqwest::header::{ACCEPT, USER_AGENT};
use self_update::{backends::github::ReleaseList, update::ReleaseAsset};
use std::{env::consts::EXE_SUFFIX, path::PathBuf};
use tokio::io::AsyncWriteExt;

const REPO_OWNER: &str = "theguy000";
const REPO_NAME: &str = "QuillScribe";
const BIN_NAME: &str = "quillscribe";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const USER_AGENT_VALUE: &str = concat!("QuillScribe/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub download_url: String,
}

struct SelectedRelease {
    update: AvailableUpdate,
    asset: ReleaseAsset,
}

pub fn check_for_update() -> Result<Option<AvailableUpdate>, String> {
    find_latest_update().map(|selected| selected.map(|selected| selected.update))
}

pub async fn install_update<F>(mut progress_cb: F) -> Result<String, String>
where
    F: FnMut(f32) + Send + 'static,
{
    progress_cb(5.0);
    let selected =
        find_latest_update()?.ok_or_else(|| "You're already on the latest version.".to_string())?;

    let temp_dir = self_update::TempDir::new()
        .map_err(|e| format!("Failed to create update temp dir: {e}"))?;
    let archive_path = temp_dir.path().join(asset_file_name(&selected.asset.name)?);

    download_asset(
        &selected.asset.download_url,
        &archive_path,
        |download_progress| {
            progress_cb(5.0 + (download_progress * 0.85));
        },
    )
    .await?;

    progress_cb(90.0);
    let bin_path = bin_path_in_archive();
    self_update::Extract::from_source(&archive_path)
        .extract_file(temp_dir.path(), &bin_path)
        .map_err(|e| format!("Failed to extract update asset: {e}"))?;

    let new_exe = temp_dir.path().join(&bin_path);
    ensure_executable(&new_exe)?;
    progress_cb(98.0);

    self_update::self_replace::self_replace(&new_exe)
        .map_err(|e| format!("Failed to replace current executable: {e}"))?;

    progress_cb(100.0);
    Ok(selected.update.version)
}

fn find_latest_update() -> Result<Option<SelectedRelease>, String> {
    let target = self_update::get_target();
    let releases = ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .map_err(|e| format!("Failed to configure update check: {e}"))?
        .fetch()
        .map_err(|e| format!("Failed to fetch GitHub releases: {e}"))?;

    let mut latest: Option<(String, SelectedRelease)> = None;

    for release in releases {
        let Some(version) = normalize_version(&release.version) else {
            continue;
        };

        if !is_newer(CURRENT_VERSION, &version) {
            continue;
        }

        let Some(asset) = release.asset_for(target, None) else {
            continue;
        };

        let should_replace = latest
            .as_ref()
            .map(|(latest_version, _)| is_newer(latest_version, &version))
            .unwrap_or(true);

        if should_replace {
            let notes = release.body.unwrap_or_default();
            latest = Some((
                version.clone(),
                SelectedRelease {
                    update: AvailableUpdate {
                        version,
                        notes,
                        asset_name: asset.name.clone(),
                        download_url: asset.download_url.clone(),
                    },
                    asset,
                },
            ));
        }
    }

    Ok(latest.map(|(_, selected)| selected))
}

fn normalize_version(version: &str) -> Option<String> {
    let trimmed = version.trim().trim_start_matches('v');
    let start = trimmed.find(|ch: char| ch.is_ascii_digit())?;
    let version = &trimmed[start..];
    let end = version
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '+'))
        .unwrap_or(version.len());
    let version = &version[..end];

    if self_update::version::bump_is_greater("0.0.0", version).is_ok() {
        Some(version.to_string())
    } else {
        None
    }
}

fn is_newer(current: &str, other: &str) -> bool {
    self_update::version::bump_is_greater(current, other).unwrap_or(false)
}

async fn download_asset<F>(
    download_url: &str,
    archive_path: &PathBuf,
    mut progress_cb: F,
) -> Result<(), String>
where
    F: FnMut(f32),
{
    let response = reqwest::Client::new()
        .get(download_url)
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header(ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("Failed to download update asset: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download update asset: GitHub returned {}",
            response.status()
        ));
    }

    let total = response.content_length();
    let mut downloaded = 0_u64;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(archive_path)
        .await
        .map_err(|e| format!("Failed to create update asset file: {e}"))?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Failed while downloading update asset: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write update asset: {e}"))?;
        downloaded += chunk.len() as u64;

        if let Some(total) = total.filter(|total| *total > 0) {
            progress_cb((downloaded as f32 / total as f32 * 100.0).clamp(0.0, 100.0));
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("Failed to flush update asset: {e}"))?;
    progress_cb(100.0);
    Ok(())
}

fn asset_file_name(asset_name: &str) -> Result<PathBuf, String> {
    PathBuf::from(asset_name)
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| format!("Update asset name is invalid: {asset_name}"))
}

fn bin_path_in_archive() -> PathBuf {
    PathBuf::from(format!("{BIN_NAME}{EXE_SUFFIX}"))
}

#[cfg(unix)]
fn ensure_executable(path: &PathBuf) -> Result<(), String> {
    use std::{fs, os::unix::fs::PermissionsExt};

    let mut permissions = fs::metadata(path)
        .map_err(|e| format!("Failed to inspect extracted update binary: {e}"))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|e| format!("Failed to mark update binary executable: {e}"))
}

#[cfg(not(unix))]
fn ensure_executable(_path: &PathBuf) -> Result<(), String> {
    Ok(())
}
