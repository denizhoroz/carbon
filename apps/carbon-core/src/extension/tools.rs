//! Tools an extension downloads when they are not installed: language
//! server programs (`[language_servers.download.<target>]`). Lookup order
//! (`.agents/documents/extensions.md`): the user's path setting, then `PATH`,
//! then a downloaded copy; a download happens only after the user agrees.
//!
//! A download goes to `<tools>/<extension id>/<server id>/<version>/`
//! through a staging folder: fetched over HTTPS, checked against the
//! manifest's SHA-256 before anything is unpacked, unpacked (zip, gzip,
//! tar.gz or the file itself), and the program marked executable. Other
//! versions are removed once the new one is in place.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{
    Archive, Download, ExtensionDirs, LanguageServerDef, Loaded, check_package_path, current_target,
};

/// Largest download accepted.
pub const MAX_TOOL_BYTES: u64 = 512 * 1024 * 1024;
/// Gives up when the server does not answer within this time.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared between the thread that downloads and the UI.
#[derive(Default)]
pub struct ToolProgress {
    received: AtomicU64,
    /// Size the server announced; 0 when unknown.
    total: AtomicU64,
    cancel: AtomicBool,
}

impl ToolProgress {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// (bytes received, bytes expected or 0).
    pub fn bytes(&self) -> (u64, u64) {
        (
            self.received.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }
}

/// The build of `server` for this OS and CPU, if the manifest has one.
pub fn tool_download(server: &LanguageServerDef) -> Option<&Download> {
    server.download.get(current_target()?)
}

/// The file name the program is saved as: `bin`'s last part, else the
/// command (with `.exe` on Windows).
fn program_name(server: &LanguageServerDef, download: &Download) -> String {
    match &download.bin {
        Some(bin) => bin.rsplit('/').next().unwrap_or(bin).to_string(),
        None if cfg!(windows) => format!("{}.exe", server.command),
        None => server.command.clone(),
    }
}

/// `<tools>/<extension>/<server>`.
fn server_dir(dirs: &ExtensionDirs, loaded: &Loaded, server: &LanguageServerDef) -> PathBuf {
    dirs.tools_dir()
        .join(loaded.def.id.as_str())
        .join(&server.id)
}

/// Where the program of this server's download is (or would be) saved.
fn program_path(
    dirs: &ExtensionDirs,
    loaded: &Loaded,
    server: &LanguageServerDef,
    download: &Download,
) -> PathBuf {
    let dir = server_dir(dirs, loaded, server).join(&download.version);
    match (&download.archive, &download.bin) {
        (Archive::Zip | Archive::TarGz, Some(bin)) => dir.join(bin),
        _ => dir.join(program_name(server, download)),
    }
}

/// The downloaded program, if this version was downloaded before.
pub fn downloaded_program(
    dirs: &ExtensionDirs,
    loaded: &Loaded,
    server: &LanguageServerDef,
) -> Option<PathBuf> {
    let download = tool_download(server)?;
    let path = program_path(dirs, loaded, server, download);
    path.is_file().then_some(path)
}

/// The host a download comes from (for the prompt and the `net` grant).
pub fn download_host(download: &Download) -> String {
    download
        .url
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .map(|host| host.rsplit('@').next().unwrap_or(host))
        .map(|host| host.split(':').next().unwrap_or(host).to_ascii_lowercase())
        .unwrap_or_default()
}

/// Downloads, checks and unpacks `server`'s build for this OS and CPU;
/// returns the program. Blocking and network-bound: run it off the UI thread.
pub fn download_tool(
    dirs: &ExtensionDirs,
    loaded: &Loaded,
    server: &LanguageServerDef,
    progress: &ToolProgress,
) -> Result<PathBuf, String> {
    let download = tool_download(server).ok_or_else(|| {
        format!(
            "{} has no download for this computer ({})",
            server.name,
            current_target().unwrap_or("unknown")
        )
    })?;
    if !download.url.starts_with("https://") {
        return Err(format!("{}: only https:// downloads", download.url));
    }
    let staging = staging_dir(dirs, loaded, server)?;
    let result = fetch(&download.url, progress, &staging)
        .and_then(|file| install_tool_file(dirs, loaded, server, &file));
    let _ = fs::remove_dir_all(&staging);
    result
}

/// A new staging folder next to the server's versions.
fn staging_dir(
    dirs: &ExtensionDirs,
    loaded: &Loaded,
    server: &LanguageServerDef,
) -> Result<PathBuf, String> {
    let base = server_dir(dirs, loaded, server);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_nanos())
        .unwrap_or_default();
    let staging = base.join(format!(".staging-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&staging).map_err(|err| format!("{}: {err}", staging.display()))?;
    Ok(staging)
}

/// Checks a downloaded file against the manifest's SHA-256 for this
/// computer and unpacks it as the server's program; returns the program.
pub fn install_tool_file(
    dirs: &ExtensionDirs,
    loaded: &Loaded,
    server: &LanguageServerDef,
    file: &Path,
) -> Result<PathBuf, String> {
    let download = tool_download(server)
        .ok_or_else(|| format!("{} has no download for this computer", server.name))?;
    let base = server_dir(dirs, loaded, server);
    let staging = staging_dir(dirs, loaded, server)?;
    let result = (|| {
        let digest = sha256_file(file)?;
        if digest != download.sha256 {
            return Err(format!(
                "the download does not match the extension's SHA-256 (got {digest})"
            ));
        }
        let unpacked = staging.join("out");
        unpack(file, download, &program_name(server, download), &unpacked)?;
        let target = base.join(&download.version);
        if target.exists() {
            fs::remove_dir_all(&target).map_err(|err| format!("{}: {err}", target.display()))?;
        }
        fs::rename(&unpacked, &target).map_err(|err| format!("{}: {err}", target.display()))?;
        Ok(target)
    })();
    let _ = fs::remove_dir_all(&staging);
    let target = result?;
    let program = program_path(dirs, loaded, server, download);
    if !program.is_file() {
        return Err(format!(
            "{} is not in the download",
            download.bin.as_deref().unwrap_or("the program")
        ));
    }
    make_executable(&program)?;
    // Other versions and leftovers of earlier attempts.
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path != target && path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }
    Ok(program)
}

/// Streams `url` into `<staging>/download`, counting and honouring cancel.
fn fetch(url: &str, progress: &ToolProgress, staging: &Path) -> Result<PathBuf, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("Carbon/", env!("CARGO_PKG_VERSION")))
        .timeout(None)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|err| err.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("{url}: {}", chain(&err)))?;
    let total = response.content_length().unwrap_or(0);
    if total > MAX_TOOL_BYTES {
        return Err(format!("{url} is larger than {} MB", MAX_TOOL_BYTES >> 20));
    }
    progress.total.store(total, Ordering::Relaxed);
    let path = staging.join("download");
    let mut file = fs::File::create(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut buffer = vec![0; 64 * 1024];
    let mut received = 0u64;
    loop {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let read = response
            .read(&mut buffer)
            .map_err(|err| format!("{url}: {err}"))?;
        if read == 0 {
            break;
        }
        received += read as u64;
        if received > MAX_TOOL_BYTES {
            return Err(format!("{url} is larger than {} MB", MAX_TOOL_BYTES >> 20));
        }
        file.write_all(&buffer[..read])
            .map_err(|err| format!("{}: {err}", path.display()))?;
        progress.received.store(received, Ordering::Relaxed);
    }
    Ok(path)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    std::io::copy(&mut file, &mut hasher).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Unpacks the checked download into `out`. Archive entries follow the
/// package path rules (no `..`, no links), so nothing lands outside `out`.
fn unpack(file: &Path, download: &Download, name: &str, out: &Path) -> Result<(), String> {
    let fail = |err: &dyn std::fmt::Display| format!("{}: {err}", file.display());
    fs::create_dir_all(out).map_err(|err| fail(&err))?;
    let open = || fs::File::open(file).map_err(|err| fail(&err));
    match download.archive {
        Archive::None => {
            fs::copy(file, out.join(name)).map_err(|err| fail(&err))?;
        }
        Archive::Gz => {
            let mut decoder = flate2::read::GzDecoder::new(open()?);
            let mut target = fs::File::create(out.join(name)).map_err(|err| fail(&err))?;
            copy_limited(&mut decoder, &mut target).map_err(|err| fail(&err))?;
        }
        Archive::Zip => {
            let mut archive = zip::ZipArchive::new(open()?).map_err(|err| fail(&err))?;
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).map_err(|err| fail(&err))?;
                let entry_name = entry.name().map_err(|err| fail(&err))?.replace('\\', "/");
                if entry.is_dir() {
                    continue;
                }
                if entry.is_symlink() {
                    return Err(fail(&format!("`{entry_name}`: links are not unpacked")));
                }
                check_package_path(&entry_name).map_err(|err| fail(&err))?;
                write_entry(out, &entry_name, &mut entry).map_err(|err| fail(&err))?;
            }
        }
        Archive::TarGz => {
            let decoder = flate2::read::GzDecoder::new(open()?);
            let mut archive = tar::Archive::new(decoder);
            for entry in archive.entries().map_err(|err| fail(&err))? {
                let mut entry = entry.map_err(|err| fail(&err))?;
                let kind = entry.header().entry_type();
                if kind.is_dir() {
                    continue;
                }
                let entry_name = entry
                    .path()
                    .map_err(|err| fail(&err))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let entry_name = entry_name.trim_start_matches("./").to_string();
                if !kind.is_file() {
                    return Err(fail(&format!(
                        "`{entry_name}`: only plain files are unpacked"
                    )));
                }
                check_package_path(&entry_name).map_err(|err| fail(&err))?;
                write_entry(out, &entry_name, &mut entry).map_err(|err| fail(&err))?;
            }
        }
    }
    Ok(())
}

fn write_entry(out: &Path, name: &str, reader: &mut impl Read) -> Result<(), String> {
    let target = out.join(name);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let mut file =
        fs::File::create(&target).map_err(|err| format!("{}: {err}", target.display()))?;
    copy_limited(reader, &mut file).map_err(|err| format!("{}: {err}", target.display()))
}

/// Copies at most `MAX_TOOL_BYTES` (a small archive may unpack to a huge file).
fn copy_limited(reader: &mut impl Read, writer: &mut impl Write) -> std::io::Result<()> {
    let copied = std::io::copy(&mut reader.take(MAX_TOOL_BYTES + 1), writer)?;
    if copied > MAX_TOOL_BYTES {
        return Err(std::io::Error::other(format!(
            "unpacks to more than {} MB",
            MAX_TOOL_BYTES >> 20
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = fs::metadata(path)
        .map_err(|err| format!("{}: {err}", path.display()))?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(path, permissions).map_err(|err| format!("{}: {err}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<(), String> {
    Ok(())
}

/// An error and its causes (reqwest's own message is often just "error
/// sending request").
fn chain(err: &dyn std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        if !text.contains(&cause_text) {
            text.push_str(": ");
            text.push_str(&cause_text);
        }
        source = cause.source();
    }
    text
}
