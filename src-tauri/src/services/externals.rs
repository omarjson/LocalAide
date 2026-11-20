//! Programs LocalAide fetches once, on request, with pinned checksums:
//! the sherpa-onnx speech engine (local voice input) and cloudflared
//! (Share my assistant). Each pin names the exact release file for this
//! platform and its SHA-256; a file that does not match is deleted and
//! never run. Fetched programs live in `~/.aiassistant/tools/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Serialize;

use crate::app::error::AppError;
use crate::app::paths::app_dir;
use crate::services::models::download::{DownloadProgress, ModelDownloader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Archive {
    /// The file is the program itself.
    Plain,
    TarBz2,
    TarGz,
}

/// One pinned release file.
#[derive(Debug, Clone, Copy)]
pub struct Pin {
    pub tool: &'static str,
    pub version: &'static str,
    url: &'static str,
    sha256: &'static str,
    pub size: u64,
    archive: Archive,
    /// Folder at the top of the archive, if any.
    root: &'static str,
}

const SHERPA: &str = "sherpa-onnx";
const CLOUDFLARED: &str = "cloudflared";

/// The sherpa-onnx v2.0.2 build for this platform (shared libraries, no
/// text-to-speech), from github.com/k2-fsa/sherpa-onnx/releases.
pub fn sherpa_pin() -> Option<Pin> {
    let (url, sha256, size, root) = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-linux-x64-shared-no-tts.tar.bz2",
            "c6725b8994a5e559678f72ddffbf1c14b41fc3f6f650618b98eeb61581b63da2",
            27_230_798,
            "sherpa-onnx-v2.0.2-linux-x64-shared-no-tts",
        )
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-osx-arm64-shared-no-tts.tar.bz2",
            "1a803809572335ef545978b62b0271ad119969fef6f9153a94c174c3adc95608",
            20_197_239,
            "sherpa-onnx-v2.0.2-osx-arm64-shared-no-tts",
        )
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-osx-universal2-shared-no-tts.tar.bz2",
            "282ed64d37b0885505637521686a50c9a9d6171320d01566f87a3190817de4dd",
            43_266_230,
            "sherpa-onnx-v2.0.2-osx-universal2-shared-no-tts",
        )
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-win-x64-shared-MT-MinSizeRel-no-tts.tar.bz2",
            "acfd6923924fd1eccc917e383742758cb972adc5a42ee151044a74032498ee16",
            23_636_378,
            "sherpa-onnx-v2.0.2-win-x64-shared-MT-MinSizeRel-no-tts",
        )
    } else {
        return None;
    };
    Some(Pin {
        tool: SHERPA,
        version: "2.0.2",
        url,
        sha256,
        size,
        archive: Archive::TarBz2,
        root,
    })
}

/// cloudflared 2026.11.1 for this platform, from
/// github.com/cloudflare/cloudflared/releases.
pub fn cloudflared_pin() -> Option<Pin> {
    let (url, sha256, size, archive) = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        (
            "https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-linux-amd64",
            "97bb3b02d23a5980fc9c15a1a270e692da1923f103f920d5b16d4931d5ab55e7",
            40_346_301,
            Archive::Plain,
        )
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        (
            "https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-darwin-arm64.tgz",
            "02fe4bcf08f2c09699cff30658228a739a0c47fb75f3653d1b8772a27ade4462",
            19_898_712,
            Archive::TarGz,
        )
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        (
            "https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-darwin-amd64.tgz",
            "5e0403b188dfee71fbd9b995e7dc9910d61b661c52371935ab6475d41185632f",
            21_857_340,
            Archive::TarGz,
        )
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        (
            "https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-windows-amd64.exe",
            "341deda3db5ec498a0482254d6c38c0b7cf01a30c37bb5dd95d2216d08955e35",
            55_483_112,
            Archive::Plain,
        )
    } else {
        return None;
    };
    Some(Pin {
        tool: CLOUDFLARED,
        version: "2026.11.1",
        url,
        sha256,
        size,
        archive,
        root: "",
    })
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Where a fetched tool is unpacked.
fn tool_dir(pin: &Pin) -> PathBuf {
    app_dir()
        .join("tools")
        .join(format!("{}-{}", pin.tool, pin.version))
}

/// The speech engine program, when it is on this computer.
pub fn sherpa_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LOCALAIDE_SHERPA_ONNX") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let pin = sherpa_pin()?;
    let p = tool_dir(&pin).join("bin").join(exe("sherpa-onnx-offline"));
    p.is_file().then_some(p)
}

/// cloudflared: next to the app, fetched by LocalAide, or on the PATH.
pub fn cloudflared_binary() -> Option<PathBuf> {
    let name = exe(CLOUDFLARED);
    if let Ok(cur) = std::env::current_exe()
        && let Some(dir) = cur.parent()
        && dir.join(&name).is_file()
    {
        return Some(dir.join(&name));
    }
    if let Some(pin) = cloudflared_pin() {
        let p = tool_dir(&pin).join(&name);
        if p.is_file() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<&'static str>,
    pub download_bytes: Option<u64>,
    /// This platform has a pinned build to fetch.
    pub available: bool,
}

pub fn status_of(which: &str) -> ToolStatus {
    let (bin, pin) = match which {
        SHERPA => (sherpa_binary(), sherpa_pin()),
        _ => (cloudflared_binary(), cloudflared_pin()),
    };
    ToolStatus {
        installed: bin.is_some(),
        path: bin.map(|p| p.to_string_lossy().into_owned()),
        version: pin.map(|p| p.version),
        download_bytes: pin.map(|p| p.size),
        available: pin.is_some(),
    }
}

/// Unpack with the system's tar (built into macOS, Linux and Windows 10+).
fn untar(archive: &Path, into: &Path, kind: Archive) -> Result<(), AppError> {
    let flag = if kind == Archive::TarBz2 {
        "-xjf"
    } else {
        "-xzf"
    };
    let tar = if cfg!(windows) {
        std::env::var_os("SystemRoot")
            .map(|r| PathBuf::from(r).join("System32").join("tar.exe"))
            .filter(|p| p.is_file())
            .unwrap_or_else(|| PathBuf::from("tar"))
    } else {
        PathBuf::from("tar")
    };
    let mut cmd = Command::new(tar);
    cmd.arg(flag).arg(archive).arg("-C").arg(into);
    crate::app::lifecycle::hide_console_window(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| AppError::internal(format!("Could not unpack the download: {e}")))?;
    if !out.status.success() {
        return Err(AppError::internal(format!(
            "Could not unpack the download: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Download and unpack a pinned tool. The checksum is checked before
/// anything is unpacked or run.
pub async fn fetch<F>(
    pin: Pin,
    cancel: Arc<AtomicBool>,
    on_progress: F,
) -> Result<PathBuf, AppError>
where
    F: FnMut(DownloadProgress) + Send + 'static,
{
    let dir = tool_dir(&pin);
    let parent = dir
        .parent()
        .ok_or_else(|| AppError::internal("No folder for tools"))?
        .to_path_buf();
    std::fs::create_dir_all(&parent).map_err(|e| AppError::storage(e.to_string()))?;
    let file_name = pin.url.rsplit('/').next().unwrap_or("download");
    let download = parent.join(format!(".{file_name}"));
    let audit = crate::services::cloud_audit::start(
        "GitHub",
        pin.url,
        "",
        "download",
        &format!(
            "{} {} for this computer (no personal content)",
            pin.tool, pin.version
        ),
        0,
        None,
    );
    let res = ModelDownloader::new()
        .download_atomic(
            pin.url,
            &download,
            pin.size,
            pin.sha256,
            cancel,
            on_progress,
        )
        .await;
    match &res {
        Ok(_) => audit.finish(true, "checksum matches"),
        Err(e) => audit.finish(false, &e.message),
    }
    res?;

    let staging = parent.join(format!(".{}-{}.unpack", pin.tool, pin.version));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| AppError::storage(e.to_string()))?;
    let result = (|| {
        match pin.archive {
            Archive::Plain => {
                let target = staging.join(exe(pin.tool));
                std::fs::rename(&download, &target)
                    .or_else(|_| std::fs::copy(&download, &target).map(|_| ()))
                    .map_err(|e| AppError::storage(e.to_string()))?;
                make_executable(&target);
            }
            kind => {
                untar(&download, &staging, kind)?;
                // Keep the archive's own folder layout (bin and lib), without
                // its top folder.
                let inner = if pin.root.is_empty() {
                    staging.clone()
                } else {
                    staging.join(pin.root)
                };
                if inner != staging {
                    let moved = staging.with_extension("inner");
                    let _ = std::fs::remove_dir_all(&moved);
                    std::fs::rename(&inner, &moved)
                        .map_err(|e| AppError::storage(e.to_string()))?;
                    std::fs::remove_dir_all(&staging)
                        .map_err(|e| AppError::storage(e.to_string()))?;
                    std::fs::rename(&moved, &staging)
                        .map_err(|e| AppError::storage(e.to_string()))?;
                }
                // Headers are not needed to run.
                let _ = std::fs::remove_dir_all(staging.join("include"));
                for sub in ["bin", ""] {
                    let p = if sub.is_empty() {
                        staging.join(exe(pin.tool))
                    } else {
                        staging.join(sub)
                    };
                    if p.is_dir() {
                        for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
                            make_executable(&e.path());
                        }
                    } else if p.is_file() {
                        make_executable(&p);
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&staging, &dir).map_err(|e| AppError::storage(e.to_string()))?;
        Ok::<_, AppError>(())
    })();
    let _ = std::fs::remove_file(&download);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result?;
    match pin.tool {
        SHERPA => sherpa_binary(),
        _ => cloudflared_binary(),
    }
    .ok_or_else(|| {
        AppError::internal(format!(
            "{} was unpacked but its program was not found.",
            pin.tool
        ))
    })
}

/// Environment for running the speech engine: its libraries live next to it.
pub fn sherpa_command(bin: &Path) -> Command {
    let mut cmd = Command::new(bin);
    if let Some(root) = bin.parent().and_then(Path::parent) {
        let lib = root.join("lib");
        if cfg!(target_os = "linux") {
            cmd.env("LD_LIBRARY_PATH", &lib);
        } else if cfg!(target_os = "macos") {
            cmd.env("DYLD_LIBRARY_PATH", &lib);
        }
    }
    crate::app::lifecycle::hide_console_window(&mut cmd);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Downloads both pinned tools for real and checks they run.
    /// `cargo test -- --ignored real_fetch` (needs the network).
    #[test]
    #[ignore]
    fn real_fetch() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for pin in [sherpa_pin().unwrap(), cloudflared_pin().unwrap()] {
            let bin = rt
                .block_on(fetch(pin, Arc::new(AtomicBool::new(false)), |_| {}))
                .unwrap();
            println!("{} -> {}", pin.tool, bin.display());
            let out = if pin.tool == SHERPA {
                sherpa_command(&bin).arg("--help").output().unwrap()
            } else {
                Command::new(&bin).arg("--version").output().unwrap()
            };
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            println!("{}", text.lines().next().unwrap_or(""));
            assert!(!text.is_empty());
        }
    }

    #[test]
    fn pins_are_well_formed() {
        for pin in [sherpa_pin(), cloudflared_pin()].into_iter().flatten() {
            assert!(pin.url.starts_with("https://github.com/"));
            assert_eq!(pin.sha256.len(), 64);
            assert!(pin.size > 1_000_000);
        }
    }
}
