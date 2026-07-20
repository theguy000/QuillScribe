use futures_util::StreamExt;
use reqwest::header::{ACCEPT, USER_AGENT};
use self_update::{backends::github::ReleaseList, update::ReleaseAsset};
#[cfg(target_os = "linux")]
use serde::Deserialize;
#[cfg(target_os = "linux")]
use std::{
    env, fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use std::{env::consts::EXE_SUFFIX, path::PathBuf};
use tokio::io::AsyncWriteExt;

const REPO_OWNER: &str = "theguy000";
const REPO_NAME: &str = "QuillScribe";
const BIN_NAME: &str = "quillscribe";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const USER_AGENT_VALUE: &str = concat!("QuillScribe/", env!("CARGO_PKG_VERSION"));
#[cfg(target_os = "linux")]
const LINUX_TARBALL_ASSET: &str = "quillscribe-x86_64-unknown-linux-gnu.tar.gz";

#[derive(Clone, Debug)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub download_url: String,
    pub can_install: bool,
    pub install_hint: String,
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

    #[cfg(target_os = "linux")]
    if !selected.update.can_install {
        return Err(selected.update.install_hint);
    }

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

    #[cfg(target_os = "linux")]
    install_linux_update(&new_exe)?;

    #[cfg(not(target_os = "linux"))]
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

        let Some(asset) = select_release_asset(&release, target) else {
            continue;
        };

        let should_replace = latest
            .as_ref()
            .map(|(latest_version, _)| is_newer(latest_version, &version))
            .unwrap_or(true);

        if should_replace {
            let notes = release.body.unwrap_or_default();
            let install_support = install_support();
            latest = Some((
                version.clone(),
                SelectedRelease {
                    update: AvailableUpdate {
                        version,
                        notes,
                        asset_name: asset.name.clone(),
                        download_url: asset.download_url.clone(),
                        can_install: install_support.can_install,
                        install_hint: install_support.hint,
                    },
                    asset,
                },
            ));
        }
    }

    Ok(latest.map(|(_, selected)| selected))
}

fn select_release_asset(
    release: &self_update::update::Release,
    _target: &str,
) -> Option<ReleaseAsset> {
    #[cfg(target_os = "linux")]
    {
        release
            .assets
            .iter()
            .find(|asset| asset.name == LINUX_TARBALL_ASSET)
            .cloned()
    }

    #[cfg(not(target_os = "linux"))]
    {
        release.asset_for(_target, None)
    }
}

struct InstallSupport {
    can_install: bool,
    hint: String,
}

fn install_support() -> InstallSupport {
    #[cfg(target_os = "linux")]
    {
        match read_managed_linux_install_metadata() {
            Ok(_) => InstallSupport {
                can_install: true,
                hint: String::new(),
            },
            Err(reason) => InstallSupport {
                can_install: false,
                hint: format!(
                    "Automatic Linux updates require a managed tarball install. {reason}"
                ),
            },
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        InstallSupport {
            can_install: true,
            hint: String::new(),
        }
    }
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

#[cfg(target_os = "linux")]
#[derive(Deserialize)]
struct InstallMetadata {
    managed_by: String,
    selected_format: String,
    binary_path: PathBuf,
}

#[cfg(target_os = "linux")]
fn install_metadata_path() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|path| path.join("quillscribe").join("install.json"))
        .ok_or_else(|| "Could not locate the user data directory.".to_string())
}

#[cfg(target_os = "linux")]
fn read_managed_linux_install_metadata() -> Result<InstallMetadata, String> {
    let metadata_path = install_metadata_path()?;
    let contents = fs::read_to_string(&metadata_path).map_err(|_| {
        format!(
            "Rerun the one-line installer so it can create {}.",
            metadata_path.display()
        )
    })?;
    let metadata: InstallMetadata =
        serde_json::from_str(&contents).map_err(|e| format!("Install metadata is invalid: {e}"))?;

    if metadata.managed_by != "quillscribe-installer" {
        return Err("The current install is not managed by the QuillScribe installer.".to_string());
    }

    if metadata.selected_format != "tarball" {
        return Err(
            "AppImage and package-manager installs should be updated outside the app.".to_string(),
        );
    }

    let current_exe = env::current_exe()
        .map_err(|e| format!("Could not determine the running executable: {e}"))?;
    let current_exe = current_exe
        .canonicalize()
        .map_err(|e| format!("Could not resolve the running executable path: {e}"))?;
    let managed_binary = metadata
        .binary_path
        .canonicalize()
        .map_err(|e| format!("Could not resolve managed binary path: {e}"))?;

    if current_exe != managed_binary {
        return Err(format!(
            "The running executable is {}, but the managed install is {}.",
            current_exe.display(),
            managed_binary.display()
        ));
    }

    Ok(metadata)
}

#[cfg(target_os = "linux")]
fn install_linux_update(new_exe: &Path) -> Result<(), String> {
    let metadata = read_managed_linux_install_metadata()?;
    let install_path = metadata.binary_path;
    let install_dir = install_path
        .parent()
        .ok_or_else(|| format!("Managed binary path is invalid: {}", install_path.display()))?;
    let staged_path = install_dir.join(format!(".{BIN_NAME}.new"));
    let backup_path = install_dir.join(format!(".{BIN_NAME}.previous"));

    fs::copy(new_exe, &staged_path).map_err(|e| {
        format!(
            "Failed to stage updated binary at {}: {e}",
            staged_path.display()
        )
    })?;
    ensure_executable(&staged_path)?;
    validate_binary_health_check(&staged_path)?;

    fs::copy(&install_path, &backup_path).map_err(|e| {
        format!(
            "Failed to back up current binary to {}: {e}",
            backup_path.display()
        )
    })?;
    ensure_executable(&backup_path)?;

    fs::rename(&staged_path, &install_path).map_err(|e| {
        format!(
            "Failed to replace {} with the staged update: {e}",
            install_path.display()
        )
    })?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_binary_health_check(path: &Path) -> Result<(), String> {
    let mut child = Command::new(path)
        .arg("--health-check")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to run update health check: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("Failed while waiting for update health check: {e}"))?
        {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("Update health check failed with status {status}"))
            };
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Update health check timed out.".to_string());
        }

        thread::sleep(Duration::from_millis(50));
    }
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
