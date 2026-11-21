//! Programs LocalAide fetches once, on request, with pinned checksums:
//! the llama.cpp model engine (llama-server, picked for this computer's
//! processor), the sherpa-onnx speech engine (local voice input) and
//! cloudflared (Share my assistant). Each pin names the exact release file for this
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
    Zip,
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
pub const LLAMA: &str = "llama.cpp";
/// The llama.cpp release LocalAide runs models with.
pub const LLAMA_BUILD: &str = "b12359";

/// The processor family of this computer, which can differ from the app's
/// own build: an x64 app on Windows on ARM runs under emulation, and an
/// Intel Mac build runs under Rosetta on Apple silicon. The model engine is
/// a separate program, so it can always use the native build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuArch {
    X64,
    Arm64,
    Other,
}

impl CpuArch {
    pub fn label(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Arm64 => "ARM64",
            Self::Other => std::env::consts::ARCH,
        }
    }
}

/// The native processor family, seeing through emulation.
pub fn native_arch() -> CpuArch {
    #[cfg(windows)]
    {
        // IsWow64Process2 reports the machine's own architecture even to an
        // emulated x64 process. Windows 10 1709 and later.
        use std::ffi::c_void;
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut c_void;
            fn IsWow64Process2(process: *mut c_void, own: *mut u16, native: *mut u16) -> i32;
        }
        let (mut own, mut native) = (0u16, 0u16);
        // SAFETY: both pointers are valid for the call; the handle is the
        // pseudo-handle of this process.
        let ok = unsafe { IsWow64Process2(GetCurrentProcess(), &mut own, &mut native) };
        if ok != 0 {
            match native {
                0xAA64 => return CpuArch::Arm64,
                0x8664 => return CpuArch::X64,
                _ => {}
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // 1 when this process runs under Rosetta on Apple silicon.
        let mut translated: libc::c_int = 0;
        let mut size = std::mem::size_of::<libc::c_int>();
        let name = c"sysctl.proc_translated";
        // SAFETY: the buffer and its size match; the name is a C string.
        let rc = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                (&mut translated as *mut libc::c_int).cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 && translated == 1 {
            return CpuArch::Arm64;
        }
    }
    match std::env::consts::ARCH {
        "x86_64" => CpuArch::X64,
        "aarch64" => CpuArch::Arm64,
        _ => CpuArch::Other,
    }
}

/// The official llama.cpp build for this computer, from
/// github.com/ggml-org/llama.cpp/releases. The x64 Windows and Linux builds
/// carry one CPU library per instruction set (SSE4.2 up to AVX-512) and
/// pick the one this processor supports when they start, so older
/// processors without AVX2 work too.
pub fn llama_pin() -> Option<Pin> {
    llama_pin_for(std::env::consts::OS, native_arch())
}

fn llama_pin_for(os: &str, arch: CpuArch) -> Option<Pin> {
    let (url, sha256, size, archive, root) = match (os, arch) {
        ("windows", CpuArch::X64) => (
            "https://github.com/ggml-org/llama.cpp/releases/download/b12359/llama-b12359-bin-win-cpu-x64.zip",
            "837c1655e87df3b66989f4be33a792a1399c785bf259038dc75c900096683e69",
            25_097_509,
            Archive::Zip,
            "",
        ),
        ("windows", CpuArch::Arm64) => (
            "https://github.com/ggml-org/llama.cpp/releases/download/b12359/llama-b12359-bin-win-cpu-arm64.zip",
            "d27564c754478d2832789dfcebfc5ace018e36a224c41d810b7cbc1ba8243ec0",
            13_455_326,
            Archive::Zip,
            "",
        ),
        ("macos", CpuArch::Arm64) => (
            "https://github.com/ggml-org/llama.cpp/releases/download/b12359/llama-b12359-bin-macos-arm64.tar.gz",
            "82b00abc740160e1b9980e6cc79c117345fa34af058c80e6c0a69aa8ddccde75",
            16_907_811,
            Archive::TarGz,
            "llama-b12359",
        ),
        ("macos", CpuArch::X64) => (
            "https://github.com/ggml-org/llama.cpp/releases/download/b12359/llama-b12359-bin-macos-x64.tar.gz",
            "25320d529077ef5a3d16e7b5da26438f221d88c8c0930a32b0c79457f5eede56",
            12_661_621,
            Archive::TarGz,
            "llama-b12359",
        ),
        ("linux", CpuArch::X64) => (
            "https://github.com/ggml-org/llama.cpp/releases/download/b12359/llama-b12359-bin-ubuntu-x64.tar.gz",
            "0bc1f091e8916102b6d8f2cf944233a620e90a0f856c72d0ce41150c65c2e72e",
            23_311_084,
            Archive::TarGz,
            "llama-b12359",
        ),
        ("linux", CpuArch::Arm64) => (
            "https://github.com/ggml-org/llama.cpp/releases/download/b12359/llama-b12359-bin-ubuntu-arm64.tar.gz",
            "dc7aa17a67884619b7223870aca9fd6addbc01717713b04a714add1c31137359",
            18_949_862,
            Archive::TarGz,
            "llama-b12359",
        ),
        _ => return None,
    };
    Some(Pin {
        tool: LLAMA,
        version: LLAMA_BUILD,
        url,
        sha256,
        size,
        archive,
        root,
    })
}

/// The fetched llama-server program, when it is on this computer.
pub fn llama_binary() -> Option<PathBuf> {
    let pin = llama_pin()?;
    let p = tool_dir(&pin).join(exe("llama-server"));
    p.is_file().then_some(p)
}

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
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-win-arm64-shared-MT-Release-no-tts.tar.bz2",
            "19f8bdf7871e6f57f71cf4e1a7221e499fa49fe9f95eb58c584faae585b9f201",
            23_864_664,
            "sherpa-onnx-v2.0.2-win-arm64-shared-MT-Release-no-tts",
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
    } else if cfg!(target_os = "windows") {
        // There is no ARM64 Windows build; Windows on ARM runs this one
        // under its x64 emulation.
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
        LLAMA => (llama_binary(), llama_pin()),
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
    let flag = match kind {
        Archive::TarBz2 => "-xjf",
        Archive::TarGz => "-xzf",
        // bsdtar (Windows, macOS) reads zip files too.
        _ => "-xf",
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
                let program = if pin.tool == LLAMA {
                    "llama-server"
                } else {
                    pin.tool
                };
                for sub in ["bin", ""] {
                    let p = if sub.is_empty() {
                        staging.join(exe(program))
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
        LLAMA => llama_binary(),
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
    fn every_desktop_gets_a_model_engine() {
        for os in ["windows", "macos", "linux"] {
            for arch in [CpuArch::X64, CpuArch::Arm64] {
                let pin = llama_pin_for(os, arch).expect("a pinned build");
                assert!(pin.url.contains(LLAMA_BUILD));
                let wants = match arch {
                    CpuArch::X64 => "x64",
                    _ => "arm64",
                };
                assert!(pin.url.contains(wants), "{os} {arch:?}: {}", pin.url);
                assert_eq!(pin.archive == Archive::Zip, os == "windows");
            }
        }
        assert!(llama_pin_for("freebsd", CpuArch::X64).is_none());
    }

    /// Downloads the model engine for this computer and checks it starts.
    /// `cargo test -- --ignored real_llama_fetch` (needs the network).
    #[test]
    #[ignore]
    fn real_llama_fetch() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let pin = llama_pin().unwrap();
        let bin = rt
            .block_on(fetch(pin, Arc::new(AtomicBool::new(false)), |_| {}))
            .unwrap();
        let out = Command::new(&bin).arg("--version").output().unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        println!("{}", text.trim());
        assert!(text.contains("build 12359"), "{text}");
    }

    #[test]
    fn pins_are_well_formed() {
        for pin in [sherpa_pin(), cloudflared_pin(), llama_pin()]
            .into_iter()
            .flatten()
        {
            assert!(pin.url.starts_with("https://github.com/"));
            assert_eq!(pin.sha256.len(), 64);
            assert!(pin.size > 1_000_000);
        }
    }
}
