use leptos::prelude::Await;
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use worker::console_error;
use worker::wasm_bindgen_futures;
use worker::Env;
use workflows_rs::Retries;
use workflows_rs::WorkflowStepConfig;
use workflows_rs::{from_value, WorkflowEvent, WorkflowStep};

#[derive(Serialize)]
struct WhisperInput {
    pub audio: String,
}

#[derive(Deserialize)]
struct WhisperOutput {
    pub text: String,
}

#[derive(Serialize)]
struct BartInput {
    pub input_text: String,
}

#[derive(Deserialize)]
struct BartOutput {
    pub summary: String,
}

#[wasm_bindgen]
pub struct ParseWorkflow {
    env: Env,
}

#[wasm_bindgen]
impl ParseWorkflow {
    #[wasm_bindgen(constructor)]
    pub fn new(_ctx: JsValue, env: Env) -> Self {
        Self { env: env }
    }

    pub async fn run(&self, event: JsValue, step: WorkflowStep) -> Result<String, JsValue> {
        let event: WorkflowEvent<String> = from_value(event).unwrap();
        let payload = event.payload;
        let env = self.env.clone();
        let transcription = step
            .exec("transcribe", None, move || {
                let env = env.clone();
                let payload = payload.clone();
                async move {
                    let ai = env
                        .ai("AI")
                        .map_err(|_| "Failed to get AI binding.".to_string())?;
                    match ai
                        .run::<WhisperInput, WhisperOutput>(
                            "@cf/openai/whisper-large-v3-turbo",
                            WhisperInput { audio: payload },
                        )
                        .await
                    {
                        Ok(transcript) => Ok(transcript.text),
                        Err(e) => {
                            console_error!("Failed to transcribe audiofile {}.", e);
                            Err("Failed to transcribe audiofile.".to_string())
                        }
                    }
                }
            })
            .await;
        let transcript = match transcription {
            Ok(t) => t,
            Err(e) => {
                return Err(JsValue::from_str(&format!(
                    "Failed to transcribe audiofile: {}",
                    e
                )))
            }
        };
        let env = self.env.clone();
        let summarization = step
            .exec("summarize", None, move || {
                let env = env.clone();
                let transcript = transcript.clone();
                async move {
                    let ai = env
                        .ai("AI")
                        .map_err(|_| "Failed to get AI binding.".to_string())?;
                    match ai
                        .run::<BartInput, BartOutput>(
                            "@cf/facebook/bart-large-cnn",
                            BartInput {
                                input_text: transcript,
                            },
                        )
                        .await
                    {
                        Ok(transcript) => Ok(transcript.summary),
                        Err(e) => {
                            console_error!("Failed to summarize audiofile {}.", e);
                            Err("Failed to summarize audiofile.".to_string())
                        }
                    }
                }
            })
            .await;
        let summary = match summarization {
            Ok(t) => t,
            Err(e) => {
                return Err(JsValue::from_str(&format!(
                    "Failed to transcribe audiofile: {}",
                    e
                )))
            }
        };
        Ok(summary)
    }
}
