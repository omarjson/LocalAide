use serde::Serialize;
use std::path::Path;

use crate::models_dir;

#[derive(Debug, Clone, Serialize)]
pub struct SttResult {
    pub text: String,
    pub language: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsResult {
    pub audio_path: String,
    pub duration_ms: u64,
}

pub struct VoiceManager {
    stt_model_path: std::path::PathBuf,
    tts_model_path: std::path::PathBuf,
}

impl VoiceManager {
    pub fn new() -> Self {
        let mut stt = models_dir();
        stt.push("whisper-tiny-q5.bin");
        let mut tts = models_dir();
        tts.push("piper-small.onnx");
        Self {
            stt_model_path: stt,
            tts_model_path: tts,
        }
    }

    pub fn transcribe(&self, audio_path: &str) -> Result<SttResult, String> {
        let path = Path::new(audio_path);
        if !path.exists() {
            return Err(format!("audio file not found: {audio_path}"));
        }
        #[cfg(feature = "voice")]
        {
            if self.stt_model_path.exists() {
                // Real whisper-rs path
                return self.transcribe_whisper(audio_path);
            }
        }
        Ok(SttResult {
            text: "[transcription stub – enable feature 'voice' and provide whisper model]"
                .to_string(),
            language: "en".to_string(),
            duration_ms: 0,
        })
    }

    #[cfg(feature = "voice")]
    fn transcribe_whisper(&self, audio_path: &str) -> Result<SttResult, String> {
        use hound::{WavReader, WavSpec};
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

        let model_path = self.stt_model_path.to_string_lossy().to_string();
        let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
            .map_err(|e| format!("whisper load error: {e}"))?;
        let mut state = ctx
            .create_state()
            .map_err(|e| format!("whisper state error: {e}"))?;

        let reader = WavReader::open(audio_path).map_err(|e| format!("wav open error: {e}"))?;
        let spec = reader.spec();
        if spec.sample_rate != 16000 {
            return Err("whisper requires 16kHz audio".to_string());
        }
        let samples: Vec<i16> = reader
            .into_samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("wav read error: {e}"))?;
        let mut audio_f32 = vec![0f32; samples.len()];
        whisper_rs::convert_integer_to_float_audio(&samples, &mut audio_f32)
            .map_err(|e| format!("audio convert error: {e}"))?;
        let audio_mono = if spec.channels == 2 {
            let mut out = vec![0f32; audio_f32.len() / 2];
            whisper_rs::convert_stereo_to_mono_audio(&audio_f32, &mut out)
                .map_err(|e| format!("stereo convert error: {e}"))?;
            out
        } else {
            audio_f32
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        state
            .full(params, &audio_mono)
            .map_err(|e| format!("whisper inference error: {e}"))?;

        let mut text = String::new();
        for i in 0..state
            .full_n_segments()
            .map_err(|e| format!("segment count error: {e}"))?
        {
            let seg = state
                .full_get_segment_text(i)
                .map_err(|e| format!("segment text error: {e}"))?;
            text.push_str(seg);
            text.push(' ');
        }

        // Duration estimate from sample count
        let sample_rate = spec.sample_rate as u64;
        let duration_ms = if sample_rate == 0 {
            0
        } else {
            (samples.len() as u64 * 1000) / sample_rate
        };
        Ok(SttResult {
            text: text.trim().to_string(),
            language: "en".to_string(),
            duration_ms,
        })
    }

    #[cfg(not(feature = "voice"))]
    fn transcribe_whisper(&self, _audio_path: &str) -> Result<SttResult, String> {
        Ok(SttResult {
            text: "[voice feature disabled]".to_string(),
            language: "en".to_string(),
            duration_ms: 0,
        })
    }

    pub fn synthesize(&self, text: &str, voice_id: &str) -> Result<TtsResult, String> {
        if text.trim().is_empty() {
            return Err("empty text".to_string());
        }
        #[cfg(feature = "voice")]
        {
            if self.tts_model_path.exists() {
                return self.synthesize_piper(text, voice_id);
            }
        }
        Ok(TtsResult {
            audio_path: format!("/tmp/{voice_id}_tts.wav"),
            duration_ms: 0,
        })
    }

    #[cfg(feature = "voice")]
    fn synthesize_piper(&self, text: &str, voice_id: &str) -> Result<TtsResult, String> {
        // Real piper-rs integration point
        // For now return stub path; actual ONNX inference would be here
        Ok(TtsResult {
            audio_path: format!("/tmp/{voice_id}_tts.wav"),
            duration_ms: 0,
        })
    }

    #[cfg(not(feature = "voice"))]
    fn synthesize_piper(&self, _text: &str, voice_id: &str) -> Result<TtsResult, String> {
        Ok(TtsResult {
            audio_path: format!("/tmp/{voice_id}_tts.wav"),
            duration_ms: 0,
        })
    }
}
