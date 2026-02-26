use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use worker::console_error;
use worker::console_log;
use worker::wasm_bindgen_futures;
use worker::Env;
use worker::Range;
use workflows_rs::{from_value, WorkflowEvent, WorkflowStep};

#[derive(Deserialize, Clone)]
struct Input {
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Headers {
    headers: Vec<u8>,
    offset: u64,
    size: u64,
    data_size_pos: u64,
    num_channels: u16,
    bits_per_sample: u16,
}

#[derive(Serialize)]
struct WhisperInput {
    audio: Vec<u8>,
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
        let headers = step
            .exec("parse wav", None, move || {
                let env = env.clone();
                let payload = payload.clone();
                async move {
                    let header_bytes = env
                        .bucket("NOTES")
                        .map_err(|_| "Failed to get bucket.".to_string())?
                        .get(format!("raw/{}", payload.id))
                        .range(Range::Prefix { length: 65536 })
                        .execute()
                        .await
                        .map_err(|_| "Failed to get header bytes.".to_string())?
                        .ok_or("Failed to find header bytes.".to_string())?
                        .body()
                        .ok_or("Failed to get header bytes body.".to_string())?
                        .bytes()
                        .await
                        .map_err(|_| "Failed to get header bytes as bytes.".to_string())?;
                    let header_cursor = Cursor::new(header_bytes);
                    let mut header = BufReader::new(header_cursor);

                    let mut buf = [0u8; 4];
                    header
                        .read_exact(&mut buf)
                        .map_err(|_| "Failed to get riff start.".to_string())?;
                    if &buf != b"RIFF" {
                        return Err("Not a riff file".to_string());
                    }

                    header
                        .seek(SeekFrom::Current(8))
                        .map_err(|_| "Failed to move seeker.".to_string())?;

                    let mut num_channels = 0;
                    let mut bits_per_sample = 0;

                    let (data_size_pos, offset, size) = loop {
                        let mut id = [0u8; 4];
                        let mut size_buf = [0u8; 4];
                        header
                            .read_exact(&mut id)
                            .map_err(|e| format!("Failed to read id: {}", e.to_string()))?;
                        header
                            .read_exact(&mut size_buf)
                            .map_err(|_| "Failed to read size.".to_string())?;
                        let size = u32::from_le_bytes(size_buf) as u64;

                        if &id == b"fmt " {
                            let mut fmt = vec![0u8; size as usize];
                            header
                                .read_exact(&mut fmt)
                                .map_err(|_| "Failed to get fmt chunk.".to_string())?;
                            let audio_format = u16::from_le_bytes([fmt[0], fmt[1]]);
                            if audio_format != 1 {
                                return Err("Only PCM WAV is supported".to_string());
                            }
                            num_channels = u16::from_le_bytes([fmt[2], fmt[3]]);
                            bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
                            if size % 2 != 0 {
                                header
                                    .seek(SeekFrom::Current(1))
                                    .map_err(|_| "Failed to skip fmt padding.".to_string())?;
                            }
                            continue;
                        } else if &id == b"data" {
                            let data_offset = header
                                .stream_position()
                                .map_err(|_| "Failed to get offset.".to_string())?;
                            let size_pos = header
                                .stream_position()
                                .map_err(|_| "Failed to get stram position.".to_string())?
                                - 4;
                            break (size_pos, data_offset, size);
                        }
                        header
                            .seek(SeekFrom::Current(size as i64))
                            .map_err(|_| "Failed to move seeker.".to_string())?;
                        loop {
                            let mut peek = [0u8; 1];
                            if header.read_exact(&mut peek).is_err() {
                                break;
                            }
                            if peek[0] != 0x00 {
                                header
                                    .seek(SeekFrom::Current(-1))
                                    .map_err(|_| "Failed to move seeker.".to_string())?;
                                break;
                            }
                        }
                    };

                    let mut headers = vec![0u8; offset as usize];
                    header
                        .seek(SeekFrom::Start(0))
                        .map_err(|_| "Failed to move seeker.".to_string())?;
                    header
                        .read_exact(&mut headers)
                        .map_err(|_| "Failed to read headers.".to_string())?;

                    Ok(Headers {
                        headers,
                        offset,
                        size,
                        num_channels,
                        bits_per_sample,
                        data_size_pos,
                    })
                }
            })
            .await;

        let headers = match headers {
            Ok(t) => t,
            Err(e) => return Err(JsValue::from_str(&format!("Failed to get headers: {}", e))),
        };

        let frame_size = (headers.bits_per_sample as u64 / 8) * headers.num_channels as u64;
        let max_audio_bytes =
            60 * 16000 * (headers.bits_per_sample as u64 / 8) * headers.num_channels as u64;
        let audio_per_chunk = (max_audio_bytes / frame_size) * frame_size;
        let num_chunks = headers.size.div_ceil(audio_per_chunk);
        let mut transcript = String::new();

        for i in 0..num_chunks {
            let mut bytes_to_read = audio_per_chunk.min(headers.size - i * audio_per_chunk);
            bytes_to_read -= bytes_to_read % frame_size;
            if bytes_to_read == 0 {
                return Ok(String::new());
            }

            let offset = headers.offset;
            let data_size_pos = headers.data_size_pos;
            let header = headers.headers.clone();

            let env = self.env.clone();
            let payload = event.payload.clone();
            let transcription = step
                .exec(format!("transcribe {}", i), None, move || {
                    let env = env.clone();
                    let payload = payload.clone();
                    let mut header = header.clone();
                    async move {
                        let mut audio = env
                            .bucket("NOTES")
                            .map_err(|_| "Failed to get bucket.".to_string())?
                            .get(format!("raw/{}", payload.id))
                            .range(Range::OffsetWithLength {
                                offset: offset + i * audio_per_chunk,
                                length: bytes_to_read,
                            })
                            .execute()
                            .await
                            .map_err(|_| "Failed to get raw.".to_string())?
                            .ok_or("Failed to find raw.".to_string())?
                            .body()
                            .ok_or("Failed to get raw body.".to_string())?
                            .bytes()
                            .await
                            .map_err(|_| "Failed to convert raw to bytes.".to_string())?;

                        let total_len = header.len() + audio.len();
                        let riff_size = (total_len - 8) as u32;
                        header[4..8].copy_from_slice(&riff_size.to_le_bytes());

                        let pos = data_size_pos as usize;
                        if pos + 4 <= header.len() {
                            header[pos..pos + 4]
                                .copy_from_slice(&(audio.len() as u32).to_le_bytes());
                        }

                        header.append(&mut audio);

                        let ai = env
                            .ai("AI")
                            .map_err(|_| "Failed to get AI binding.".to_string())?;
                        match ai
                            .run::<WhisperInput, WhisperOutput>(
                                "@cf/openai/whisper",
                                WhisperInput { audio: header },
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
            let tmp = match transcription {
                Ok(t) => t,
                Err(e) => {
                    return Err(JsValue::from_str(&format!(
                        "Failed to transcribe audiofile: {}",
                        e
                    )))
                }
            };
            transcript.push_str(&tmp);
        }

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
                            "@cf/meta/llama-4-scout-17b-16e-instruct",
                            LlamaInput { messages: vec![LlamaMessage { role: "system".to_string(), content: "You are a teaching assistant.\n\nYour job is to clean up the text given to you.\nAfter you're done it should be one coherent lecture.\n\nRemove filler words and repetitions\nFix punctuation and capitalization\nMerge broken sentences\n\nDo NOT remove any content or meaning!\nDo NOT summarize.\nDo NOT add anything from your knowledge that wasn explicitly said.".to_string() }, LlamaMessage { role: "user".to_string(), content: transcript }] },
                        )
                        .await
                    {
                        Ok(cleaned) => Ok(cleaned.response),
                        Err(e) => {
                            console_error!("Failed to clean audiofile {}.", e);
                            Err("Failed to clean audiofile.".to_string())
                        }
                    }
                }
            })
            .await;
        let cleaned = match cleaning {
            Ok(t) => t,
            Err(e) => {
                return Err(JsValue::from_str(&format!(
                    "Failed to clean audiofile: {}",
                    e
                )))
            }
        };

        console_error!("{}", &cleaned);

        let env = self.env.clone();
        let payload = event.payload.clone();
        let cleaned_pass = cleaned.clone();
        let _ = step
            .exec("upload cleaned", None, move || {
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
                            "@cf/meta/llama-4-scout-17b-16e-instruct",
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
                    "Failed to summarize audiofile: {}",
                    e
                )))
            }
        };
        let env = self.env.clone();
        let payload = event.payload.clone();
        let _ = step
            .exec("upload summary", None, move || {
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
