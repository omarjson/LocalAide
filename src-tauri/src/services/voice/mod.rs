//! Voice input: transcription models, local and online.
//!
//! Local models run in the sherpa-onnx engine on this computer and never use
//! the network. Online models use the user's own key; "Local only" in
//! Settings → Voice blocks them entirely. Every online call is written to
//! the online log (what left this computer).

pub mod live;
pub mod local;
pub mod online;

use serde::Serialize;

/// Live: text streams while the user speaks. Batch: record, then send.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Live,
    Batch,
}

/// An online transcription provider and the models it offers.
#[derive(Debug, Clone, Serialize)]
pub struct OnlineProvider {
    pub id: &'static str,
    pub label: &'static str,
    /// Where to get a key.
    pub key_url: &'static str,
    pub host: &'static str,
    pub models: &'static [OnlineModel],
}

#[derive(Debug, Clone, Serialize)]
pub struct OnlineModel {
    pub id: &'static str,
    pub label: &'static str,
    pub delivery: Delivery,
    pub note: &'static str,
}

/// Model ids checked against each provider's documentation in November
/// 2026. OpenAI retires whisper-1 and gpt-4o-transcribe on 26 February 2027.
pub const ONLINE: &[OnlineProvider] = &[
    OnlineProvider {
        id: "gemini",
        label: "Google Gemini",
        key_url: "https://aistudio.google.com/apikey",
        host: "generativelanguage.googleapis.com",
        models: &[
            OnlineModel {
                id: "gemini-3.5-transcribe-live",
                label: "Gemini 3.5 Transcribe Live",
                delivery: Delivery::Live,
                note: "Streams while you speak; finds the language, Arabic included.",
            },
            OnlineModel {
                id: "gemini-3.5-transcribe",
                label: "Gemini 3.5 Transcribe",
                delivery: Delivery::Batch,
                note: "One request for the whole recording.",
            },
        ],
    },
    OnlineProvider {
        id: "deepgram",
        label: "Deepgram",
        key_url: "https://console.deepgram.com/",
        host: "api.deepgram.com",
        models: &[
            OnlineModel {
                id: "nova-3",
                label: "Nova-3 (live)",
                delivery: Delivery::Live,
                note: "Streams while you speak.",
            },
            OnlineModel {
                id: "nova-3",
                label: "Nova-3",
                delivery: Delivery::Batch,
                note: "One request for the whole recording.",
            },
        ],
    },
    OnlineProvider {
        id: "groq",
        label: "Groq",
        key_url: "https://console.groq.com/keys",
        host: "api.groq.com",
        models: &[OnlineModel {
            id: "whisper-large-v3-turbo",
            label: "Whisper large-v3-turbo",
            delivery: Delivery::Batch,
            note: "Very fast. About 100 languages.",
        }],
    },
    OnlineProvider {
        id: "openai",
        label: "OpenAI",
        key_url: "https://platform.openai.com/api-keys",
        host: "api.openai.com",
        models: &[
            OnlineModel {
                id: "gpt-transcribe",
                label: "GPT-Transcribe",
                delivery: Delivery::Batch,
                note: "OpenAI's current model for recordings.",
            },
            OnlineModel {
                id: "gpt-4o-transcribe",
                label: "GPT-4o Transcribe",
                delivery: Delivery::Batch,
                note: "Retires on 26 February 2027.",
            },
            OnlineModel {
                id: "whisper-1",
                label: "Whisper",
                delivery: Delivery::Batch,
                note: "Retires on 26 February 2027.",
            },
        ],
    },
    OnlineProvider {
        id: "soniox",
        label: "Soniox",
        key_url: "https://console.soniox.com/",
        host: "stt-rt.soniox.com",
        models: &[OnlineModel {
            id: "stt-rt-v6",
            label: "Soniox v6 (live)",
            delivery: Delivery::Live,
            note: "Streams while you speak; 60+ languages.",
        }],
    },
];

/// A model choice as stored in settings: a local catalog id, or
/// "online:<provider>:<model>:<live|batch>".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Local(String),
    Online {
        provider: String,
        model: String,
        delivery: Delivery,
    },
}

impl Choice {
    pub fn parse(value: &str) -> Option<Self> {
        if let Some(rest) = value.strip_prefix("online:") {
            let mut parts = rest.splitn(3, ':');
            let provider = parts.next()?.to_string();
            let model = parts.next()?.to_string();
            let delivery = match parts.next()? {
                "live" => Delivery::Live,
                "batch" => Delivery::Batch,
                _ => return None,
            };
            let known = ONLINE
                .iter()
                .find(|p| p.id == provider)?
                .models
                .iter()
                .any(|m| m.id == model && m.delivery == delivery);
            known.then_some(Self::Online {
                provider,
                model,
                delivery,
            })
        } else if value.starts_with("stt-")
            && crate::services::models::catalog::is_safe_model_id(value)
        {
            Some(Self::Local(value.to_string()))
        } else {
            None
        }
    }

    pub fn key(provider: &str, model: &str, delivery: Delivery) -> String {
        format!(
            "online:{provider}:{model}:{}",
            if delivery == Delivery::Live {
                "live"
            } else {
                "batch"
            }
        )
    }
}

/// Seconds of audio in a 16 kHz mono 16-bit recording of this many bytes.
pub fn pcm_seconds(bytes: usize) -> f32 {
    bytes as f32 / 32_000.0
}

/// Check a WAV recording from the window: RIFF/WAVE, 16-bit PCM, mono,
/// 16 kHz, at most 15 minutes. Returns the PCM samples' byte range.
pub fn check_wav(wav: &[u8]) -> Result<std::ops::Range<usize>, String> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("The recording is not a WAV file.".into());
    }
    let mut pos = 12;
    let mut fmt_ok = false;
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let len =
            u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]) as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= wav.len() {
            let format = u16::from_le_bytes([wav[body], wav[body + 1]]);
            let channels = u16::from_le_bytes([wav[body + 2], wav[body + 3]]);
            let rate =
                u32::from_le_bytes([wav[body + 4], wav[body + 5], wav[body + 6], wav[body + 7]]);
            let bits = u16::from_le_bytes([wav[body + 14], wav[body + 15]]);
            if format != 1 || channels != 1 || rate != 16_000 || bits != 16 {
                return Err("The recording must be 16 kHz mono 16-bit PCM.".into());
            }
            fmt_ok = true;
        } else if id == b"data" {
            if !fmt_ok {
                return Err("The recording has no format block.".into());
            }
            let end = (body + len).min(wav.len());
            if end - body > 16_000 * 2 * 60 * 15 {
                return Err("Recordings can be up to 15 minutes long.".into());
            }
            return Ok(body..end);
        }
        pos = body + len + (len & 1);
    }
    Err("The recording has no audio.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn wav(samples: usize) -> Vec<u8> {
        let data = samples * 2;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&((36 + data) as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&16_000u32.to_le_bytes());
        v.extend_from_slice(&32_000u32.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data as u32).to_le_bytes());
        v.resize(v.len() + data, 0);
        v
    }

    #[test]
    fn choices_parse_and_reject_unknown_ids() {
        assert_eq!(
            Choice::parse("stt-whisper-tiny"),
            Some(Choice::Local("stt-whisper-tiny".into()))
        );
        assert!(matches!(
            Choice::parse("online:deepgram:nova-3:live"),
            Some(Choice::Online {
                delivery: Delivery::Live,
                ..
            })
        ));
        assert!(Choice::parse("online:deepgram:nova-9:live").is_none());
        assert!(
            Choice::parse("online:soniox:stt-rt-v6:batch").is_none(),
            "Soniox is live only"
        );
        assert!(Choice::parse("../etc").is_none());
        assert!(
            Choice::parse("qwen3.5-0.8b-q4").is_none(),
            "chat models are not voice models"
        );
    }

    #[test]
    fn wav_checks() {
        let w = wav(16_000);
        assert_eq!(check_wav(&w).unwrap().len(), 32_000);
        assert!(check_wav(b"not a wav at all, definitely not a wav file....").is_err());
        let mut stereo = wav(10);
        stereo[22] = 2;
        assert!(check_wav(&stereo).is_err());
    }
}
