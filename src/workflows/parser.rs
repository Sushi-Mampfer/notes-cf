use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use worker::console_error;
use worker::console_log;
use worker::wasm_bindgen_futures;
use worker::Env;
use workflows_rs::{from_value, WorkflowEvent, WorkflowStep};

#[derive(Deserialize, Clone)]
struct Input {
    pub id: String,
}

#[derive(Serialize)]
struct WhisperInput {
    audio: AudioField,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AudioField {
    Base64(String),
    Raw { body: Vec<u8>, contentType: String },
}

#[derive(Deserialize)]
struct WhisperOutput {
    pub text: String,
}

#[derive(Serialize)]
struct LlamaInput {
    pub messages: Vec<LlamaMessage>,
}

#[derive(Deserialize)]
struct LlamaOutput {
    pub response: String,
}

#[derive(Serialize)]
struct LlamaMessage {
    pub role: String,
    pub content: String,
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
        let event: WorkflowEvent<Input> = from_value(event).unwrap();

        let env = self.env.clone();
        let payload = event.payload.clone();
        let transcription = step
            .exec("transcribe", None, move || {
                let env = env.clone();
                let payload = payload.clone();
                async move {
                    let audio = env
                        .bucket("NOTES")
                        .map_err(|_| "Failed to get bucket.".to_string())?
                        .get(format!("raw/{}", payload.id))
                        .execute()
                        .await
                        .map_err(|_| "Failed to get raw.".to_string())?
                        .ok_or("Failed to find raw.".to_string())?
                        .body()
                        .ok_or("Failed to get raw body.".to_string())?
                        .bytes()
                        .await
                        .map_err(|_| "Failed to convert raw to bytes.".to_string())?;
                    let ai = env
                        .ai("AI")
                        .map_err(|_| "Failed to get AI binding.".to_string())?;
                    match ai
                        .run::<WhisperInput, WhisperOutput>(
                            "@cf/openai/whisper-large-v3-turbo",
                            WhisperInput {
                                audio: AudioField::Raw {
                                    body: audio,
                                    contentType: "audio/wav".into(),
                                },
                            },
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
        let cleaning = step
            .exec("cleanup", None, move || {
                let env = env.clone();
                let transcript = transcript.clone();
                async move {
                    let ai = env
                        .ai("AI")
                        .map_err(|_| "Failed to get AI binding.".to_string())?;
                    match ai
                        .run::<LlamaInput, LlamaOutput>(
                            "@cf/facebook/bart-large-cnn",
                            LlamaInput { messages: vec![LlamaMessage { role: "system".to_string(), content: "You are a teaching assistant.\n\nYour job is to clean up the text given to you.\nAfter you're done it should be one coherent lecture.\n\nRemove filler words and repetitions\nFix punctuation and capitalization\nMerge broken sentences\n\nDo NOT remove any content or meaning!\nDo NOT summarize.\nDo NOT add anything from your knowledge that wasn explicitly said.".to_string() }, LlamaMessage { role: "user".to_string(), content: transcript }] },
                        )
                        .await
                    {
                        Ok(cleaned) => Ok(cleaned.response),
                        Err(e) => {
                            console_error!("Failed to summarize audiofile {}.", e);
                            Err("Failed to summarize audiofile.".to_string())
                        }
                    }
                }
            })
            .await;
        let cleaned = match cleaning {
            Ok(t) => t,
            Err(e) => {
                return Err(JsValue::from_str(&format!(
                    "Failed to transcribe audiofile: {}",
                    e
                )))
            }
        };
        let env = self.env.clone();
        let payload = event.payload.clone();
        let cleaned_pass = cleaned.clone();
        let _ = step
            .exec("upload_cleaned", None, move || {
                let env = env.clone();
                let cleaned = cleaned_pass.clone();
                let payload = payload.clone();
                async move {
                    env.bucket("NOTES")
                        .map_err(|_| "Failed to get bucket.".to_string())?
                        .put(format!("cleaned/{}", payload.id), cleaned)
                        .execute()
                        .await
                        .map_err(|_| "Failed to insert cleaned.".to_string())?;
                    Ok("".to_string())
                }
            })
            .await;
        let env = self.env.clone();
        let summarizing = step
            .exec("summarizing", None, move || {
                let env = env.clone();
                let cleaned = cleaned.clone();
                async move {
                    let ai = env
                        .ai("AI")
                        .map_err(|_| "Failed to get AI binding.".to_string())?;
                    match ai
                        .run::<LlamaInput, LlamaOutput>(
                            "@cf/facebook/bart-large-cnn",
                            LlamaInput { messages: vec![LlamaMessage { role: "system".to_string(), content: "You are a study assistant.\n\nYou create text files that explain the content of the lession you get as an input.\nIf there is any knowledge needed to understand the lession you take it as granted and don't explain it.\n\nYou HAVE to include everything asked for.\nYou ONLY answer in raw text without any styling except for line breakes.\nYou do NOT remove any content or create new content.\nYou add single choice questions at the end with the answers written in flipped askii. It can be however many question you find adequate, but it should only be basic understanding questions.".to_string() }, LlamaMessage { role: "user".to_string(), content: cleaned }] },
                        )
                        .await
                    {
                        Ok(summary) => Ok(summary.response),
                        Err(e) => {
                            console_error!("Failed to summarize audiofile {}.", e);
                            Err("Failed to summarize audiofile.".to_string())
                        }
                    }
                }
            })
            .await;
        let summary = match summarizing {
            Ok(t) => t,
            Err(e) => {
                return Err(JsValue::from_str(&format!(
                    "Failed to transcribe audiofile: {}",
                    e
                )))
            }
        };
        let env = self.env.clone();
        let payload = event.payload.clone();
        let _ = step
            .exec("upload_cleaned", None, move || {
                let env = env.clone();
                let summary = summary.clone();
                let payload = payload.clone();
                async move {
                    env.bucket("NOTES")
                        .map_err(|_| "Failed to get bucket.".to_string())?
                        .put(format!("summary/{}", payload.id), summary)
                        .execute()
                        .await
                        .map_err(|_| "Failed to insert cleaned.".to_string())?;
                    Ok("".to_string())
                }
            })
            .await;
        console_log!("Finished!");
        Ok("hi".to_string())
    }
}
