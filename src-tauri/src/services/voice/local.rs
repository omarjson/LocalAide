//! Local transcription with the sherpa-onnx engine. The recording is written
//! to a private temporary file, transcribed, and deleted. No network.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::app::error::AppError;
use crate::services::externals;

/// Which engine flags a model needs, from its catalog id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Whisper,
    /// Moonshine v1: preprocessor, encoder, two decoders.
    Moonshine,
    /// Moonshine v2: encoder and a merged decoder.
    MoonshineV2,
    /// NeMo transducer (Parakeet TDT).
    Transducer,
}

pub fn family(id: &str) -> Option<Family> {
    if id.starts_with("stt-whisper-") {
        Some(Family::Whisper)
    } else if id == "stt-moonshine-base-ar" {
        Some(Family::MoonshineV2)
    } else if id.starts_with("stt-moonshine-") {
        Some(Family::Moonshine)
    } else if id.starts_with("stt-parakeet-") {
        Some(Family::Transducer)
    } else {
        None
    }
}

/// The engine arguments for a model whose files are in `dir`, named
/// `<id>.<part>` as the catalog's `dest` fields say.
pub fn model_args(id: &str, dir: &Path) -> Option<Vec<String>> {
    let f = |part: &str| {
        dir.join(format!("{id}.{part}"))
            .to_string_lossy()
            .into_owned()
    };
    let tokens = format!("--tokens={}", f("tokens.txt"));
    let args = match family(id)? {
        Family::Whisper => vec![
            format!("--whisper-encoder={}", f("encoder.onnx")),
            format!("--whisper-decoder={}", f("decoder.onnx")),
            tokens,
            "--model-type=whisper".into(),
        ],
        Family::Moonshine => vec![
            format!("--moonshine-preprocessor={}", f("preprocess.onnx")),
            format!("--moonshine-encoder={}", f("encoder.onnx")),
            format!(
                "--moonshine-uncached-decoder={}",
                f("uncached-decoder.onnx")
            ),
            format!("--moonshine-cached-decoder={}", f("cached-decoder.onnx")),
            tokens,
        ],
        Family::MoonshineV2 => vec![
            format!("--moonshine-encoder={}", f("encoder.ort")),
            format!("--moonshine-merged-decoder={}", f("merged-decoder.ort")),
            tokens,
        ],
        Family::Transducer => vec![
            format!("--encoder={}", f("encoder.onnx")),
            format!("--decoder={}", f("decoder.onnx")),
            format!("--joiner={}", f("joiner.onnx")),
            tokens,
            "--model-type=nemo_transducer".into(),
        ],
    };
    Some(args)
}

/// The text from the engine's output: the last JSON line with a "text" field.
pub fn parse_output(stdout: &str) -> Option<(String, Option<String>)> {
    stdout.lines().rev().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let text = v.get("text")?.as_str()?.trim().to_string();
        let lang = v
            .get("lang")
            .and_then(|l| l.as_str())
            .filter(|l| !l.is_empty())
            .map(str::to_string);
        Some((text, lang))
    })
}

pub struct LocalResult {
    pub text: String,
    pub language: Option<String>,
    pub ms: u128,
}

fn temp_wav() -> PathBuf {
    let dir = crate::app::paths::app_dir().join("voice");
    let _ = std::fs::create_dir_all(&dir);
    let mut n = [0u8; 8];
    let _ = getrandom::fill(&mut n);
    dir.join(format!("rec-{}.wav", hex::encode(n)))
}

/// Transcribe a 16 kHz mono WAV with a downloaded model.
pub fn transcribe(id: &str, models_dir: &Path, wav: &[u8]) -> Result<LocalResult, AppError> {
    let bin = externals::sherpa_binary().ok_or_else(|| {
        AppError::new(
            "VOICE_ENGINE_MISSING",
            "The voice engine is not downloaded yet. Get it in Settings → Voice.",
        )
    })?;
    let args = model_args(id, models_dir)
        .ok_or_else(|| AppError::invalid_input(format!("{id} is not a voice model.")))?;
    let path = temp_wav();
    std::fs::write(&path, wav)
        .map_err(|e| AppError::storage(format!("Could not save the recording: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 4))
        .unwrap_or(2);
    let started = Instant::now();
    let mut cmd = externals::sherpa_command(&bin);
    cmd.arg(format!("--num-threads={threads}"))
        .args(&args)
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let result = (|| {
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::internal(format!("The voice engine did not start: {e}")))?;
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                return Err(AppError::internal(
                    "Transcription took longer than 3 minutes and was stopped.",
                ));
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let out = child
            .wait_with_output()
            .map_err(|e| AppError::internal(e.to_string()))?;
        // The engine prints its result on stderr in some builds, stdout in others.
        let all = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        match parse_output(&all) {
            Some((text, language)) => Ok(LocalResult {
                text,
                language,
                ms: started.elapsed().as_millis(),
            }),
            None => {
                let tail: String = all.lines().rev().take(3).collect::<Vec<_>>().join(" ");
                Err(AppError::internal(format!(
                    "The voice engine gave no text. {}",
                    tail.trim()
                )))
            }
        }
    })();
    let _ = std::fs::remove_file(&path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_and_args() {
        let dir = Path::new("/m");
        assert_eq!(family("stt-whisper-small"), Some(Family::Whisper));
        assert_eq!(family("stt-moonshine-tiny"), Some(Family::Moonshine));
        assert_eq!(family("stt-moonshine-base-ar"), Some(Family::MoonshineV2));
        assert_eq!(family("stt-parakeet-tdt-0.6b-v2"), Some(Family::Transducer));
        assert_eq!(family("qwen3"), None);
        let a = model_args("stt-whisper-tiny", dir).unwrap();
        assert!(a[0].ends_with("stt-whisper-tiny.encoder.onnx"));
        let a = model_args("stt-parakeet-tdt-0.6b-v2", dir).unwrap();
        assert!(a.iter().any(|x| x == "--model-type=nemo_transducer"));
    }

    /// Real transcription with a real engine and model. Run with
    /// LOCALAIDE_SHERPA_ONNX=<engine> LOCALAIDE_TEST_STT_DIR=<models folder>
    /// LOCALAIDE_TEST_STT_WAV=<16 kHz wav> cargo test -- --ignored real_
    #[test]
    #[ignore]
    fn real_local_transcription() {
        let dir = std::env::var("LOCALAIDE_TEST_STT_DIR").expect("models folder");
        let wav = std::fs::read(std::env::var("LOCALAIDE_TEST_STT_WAV").expect("wav")).unwrap();
        for id in [
            "stt-whisper-tiny",
            "stt-moonshine-tiny",
            "stt-parakeet-tdt-0.6b-v2",
        ] {
            if !Path::new(&dir).join(format!("{id}.tokens.txt")).is_file() {
                continue;
            }
            let r = transcribe(id, Path::new(&dir), &wav).unwrap();
            println!("{id}: {} ms: {}", r.ms, r.text);
            assert!(
                r.text.to_lowercase().contains("yellow lamps") || r.text.len() > 10,
                "{id}: {}",
                r.text
            );
        }
    }

    #[test]
    fn reads_engine_output() {
        let out = "/tmp/x.wav\n----\nElapsed seconds: 0.4\n{\"lang\": \"en\", \"text\": \" Hello there.\", \"tokens\": []}\n";
        assert_eq!(
            parse_output(out),
            Some(("Hello there.".into(), Some("en".into())))
        );
        assert_eq!(parse_output("nothing"), None);
    }
}
