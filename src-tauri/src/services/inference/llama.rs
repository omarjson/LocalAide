use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use llama_cpp_rs::LLama;
use llama_cpp_rs::options::{ModelOptions, PredictOptions};

use super::{EngineStatus, GenerationMetrics, InferenceEngine, InferenceToken};
use crate::app::error::AppError;

enum WorkerCommand {
    Load {
        path: PathBuf,
        context_size: i32,
        reply: Sender<Result<String, AppError>>,
    },
    Unload {
        reply: Sender<Result<(), AppError>>,
    },
    Generate {
        prompt: String,
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        token_tx: Sender<InferenceToken>,
        reply: Sender<Result<GenerationMetrics, AppError>>,
    },
    Status {
        reply: Sender<EngineStatus>,
    },
}

pub struct LlamaWorkerEngine {
    cmd_tx: Mutex<Sender<WorkerCommand>>,
}

impl LlamaWorkerEngine {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx): (Sender<WorkerCommand>, Receiver<WorkerCommand>) = channel();

        // Spawn dedicated OS thread for all llama.cpp interactions
        thread::Builder::new()
            .name("localaide-llama-worker".to_string())
            .spawn(move || {
                Self::worker_loop(cmd_rx);
            })
            .expect("Failed to spawn inference worker thread");

        Self {
            cmd_tx: Mutex::new(cmd_tx),
        }
    }

    fn worker_loop(rx: Receiver<WorkerCommand>) {
        let mut loaded_model: Option<LLama> = None;
        let mut loaded_path: Option<PathBuf> = None;
        let mut active_context_size = 2048;

        while let Ok(cmd) = rx.recv() {
            match cmd {
                WorkerCommand::Load {
                    path,
                    context_size,
                    reply,
                } => {
                    let mut opts = ModelOptions::default();
                    opts.context_size = context_size;
                    opts.n_batch = 512;
                    opts.embeddings = false;

                    let res = LLama::new(path.to_string_lossy().to_string(), &opts).map_err(|e| {
                        AppError::model_load_failed(&path.to_string_lossy(), &e.to_string())
                    });

                    match res {
                        Ok(llm) => {
                            loaded_model = Some(llm);
                            loaded_path = Some(path.clone());
                            active_context_size = context_size;
                            let _ =
                                reply.send(Ok(format!("Successfully loaded {}", path.display())));
                        }
                        Err(err) => {
                            let _ = reply.send(Err(err));
                        }
                    }
                }
                WorkerCommand::Unload { reply } => {
                    loaded_model = None;
                    loaded_path = None;
                    let _ = reply.send(Ok(()));
                }
                WorkerCommand::Generate {
                    prompt,
                    max_tokens,
                    temperature,
                    cancel_flag,
                    token_tx,
                    reply,
                } => {
                    let Some(ref llm) = loaded_model else {
                        let _ = reply.send(Err(AppError::model_not_installed(
                            "No model loaded in inference engine",
                        )));
                        continue;
                    };

                    let mut opts = PredictOptions::default();
                    opts.threads = 4;
                    opts.tokens = max_tokens;
                    opts.temperature = temperature;
                    opts.top_p = 0.9;
                    opts.repeat = 64;
                    opts.seed = -1;

                    let start_time = Instant::now();
                    let ttft_mutex = Arc::new(Mutex::new(None));
                    let ttft_cb = ttft_mutex.clone();
                    let token_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let counter_cb = token_counter.clone();

                    let cancel_cb = cancel_flag.clone();
                    let token_tx_cb = token_tx.clone();

                    opts.set_token_callback(Some(Box::new(move |token: String| -> bool {
                        // Check cancellation flag first: abort if requested
                        if cancel_cb.load(Ordering::Relaxed) {
                            return false;
                        }

                        let count = counter_cb.fetch_add(1, Ordering::Relaxed);
                        if count == 0 {
                            let elapsed = start_time.elapsed();
                            let mut t = ttft_cb.lock().unwrap_or_else(|e| e.into_inner());
                            *t = Some(elapsed);
                        }

                        let _ = token_tx_cb.send(InferenceToken {
                            token,
                            index: count,
                        });
                        true
                    })));

                    let eval_res = llm.eval(prompt, &mut opts);
                    let elapsed = start_time.elapsed();
                    let total_tokens = token_counter.load(Ordering::Relaxed);
                    let ttft = ttft_mutex.lock().unwrap_or_else(|e| e.into_inner()).take();
                    let cancelled = cancel_flag.load(Ordering::Relaxed);

                    let tokens_per_sec = if elapsed.as_secs_f64() > 0.0 {
                        total_tokens as f64 / elapsed.as_secs_f64()
                    } else {
                        0.0
                    };

                    match eval_res {
                        Ok(()) => {
                            let _ = reply.send(Ok(GenerationMetrics {
                                total_tokens,
                                ttft,
                                tokens_per_sec,
                                cancelled,
                            }));
                        }
                        Err(e) => {
                            if cancelled {
                                let _ = reply.send(Ok(GenerationMetrics {
                                    total_tokens,
                                    ttft,
                                    tokens_per_sec,
                                    cancelled: true,
                                }));
                            } else {
                                let _ = reply.send(Err(AppError::internal(e.to_string())));
                            }
                        }
                    }
                }
                WorkerCommand::Status { reply } => {
                    let status = EngineStatus {
                        loaded: loaded_model.is_some(),
                        active_model: loaded_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string()),
                        context_size: active_context_size,
                    };
                    let _ = reply.send(status);
                }
            }
        }
    }
}

impl InferenceEngine for LlamaWorkerEngine {
    fn load(&self, model_path: &Path, context_size: i32) -> Result<String, AppError> {
        let (tx, rx) = channel();
        let cmd = WorkerCommand::Load {
            path: model_path.to_path_buf(),
            context_size,
            reply: tx,
        };
        let guard = self
            .cmd_tx
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        guard
            .send(cmd)
            .map_err(|e| AppError::internal(e.to_string()))?;
        rx.recv().map_err(|e| AppError::internal(e.to_string()))?
    }

    fn unload(&self) -> Result<(), AppError> {
        let (tx, rx) = channel();
        let cmd = WorkerCommand::Unload { reply: tx };
        let guard = self
            .cmd_tx
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        guard
            .send(cmd)
            .map_err(|e| AppError::internal(e.to_string()))?;
        rx.recv().map_err(|e| AppError::internal(e.to_string()))?
    }

    fn generate(
        &self,
        prompt: &str,
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        mut on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError> {
        let (token_tx, token_rx) = channel();
        let (reply_tx, reply_rx) = channel();

        let cmd = WorkerCommand::Generate {
            prompt: prompt.to_string(),
            max_tokens,
            temperature,
            cancel_flag,
            token_tx,
            reply: reply_tx,
        };

        {
            let guard = self
                .cmd_tx
                .lock()
                .map_err(|e| AppError::internal(e.to_string()))?;
            guard
                .send(cmd)
                .map_err(|e| AppError::internal(e.to_string()))?;
        }

        // Receive tokens in real time on calling thread until generation completes
        loop {
            // Check if generation reply has arrived
            if let Ok(result) = reply_rx.try_recv() {
                // Drain any remaining buffered tokens
                while let Ok(tok) = token_rx.try_recv() {
                    on_token(tok);
                }
                return result;
            }

            // Otherwise wait up to 10ms for next token
            match token_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(tok) => on_token(tok),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Worker disconnected or finished, wait for reply
                    return reply_rx
                        .recv()
                        .map_err(|e| AppError::internal(e.to_string()))?;
                }
            }
        }
    }

    fn status(&self) -> EngineStatus {
        let (tx, rx) = channel();
        let cmd = WorkerCommand::Status { reply: tx };
        if let Ok(guard) = self.cmd_tx.lock() {
            if guard.send(cmd).is_ok() {
                if let Ok(st) = rx.recv() {
                    return st;
                }
            }
        }
        EngineStatus {
            loaded: false,
            active_model: None,
            context_size: 2048,
        }
    }
}
