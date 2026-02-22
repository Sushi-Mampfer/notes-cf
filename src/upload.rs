use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use memchr::memmem;
use regex::bytes::Regex;
use serde::Serialize;
use workflows_rs::{EnvWorkflowExt, WorkflowInstanceCreateOptions};

use crate::AppState;

#[derive(Serialize)]
struct WorkflowInput {
    pub id: String,
}

#[axum::debug_handler]
#[worker::send]
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let boundary_raw = format!(
        "--{}",
        headers
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .split_once("=")
            .unwrap()
            .1
    );
    let boundary = boundary_raw.as_bytes();

    let mut parts: Vec<Vec<u8>> = Vec::new();

    let mut finds = memmem::find_iter(&body, boundary);
    let mut last = finds.next().unwrap();
    for i in finds {
        parts.push(body[last + boundary.len()..i].to_vec());
        last = i;
    }
    for i in parts {
        let re = Regex::new("; name=\"(.*?)\"").unwrap();
        let (_, [name]) = re.captures(&i).unwrap().extract();
        let kv = state.env.kv("NOTESKV").unwrap();
        let id = match kv.get("count").text().await.unwrap() {
            Some(n) => n.parse::<i32>().unwrap() + 1,
            None => 0_i32,
        };
        kv.put("count", &id).unwrap().execute().await.unwrap();
        kv.put(&id.to_string(), String::from_utf8_lossy(name))
            .unwrap()
            .execute()
            .await
            .unwrap();
        let header_end = memmem::find(&i, b"\r\n\r\n").unwrap();
        let data = &i[header_end + 4..];
        let data = data.strip_suffix(b"\r\n").unwrap_or(data);
        state
            .env
            .bucket("NOTES")
            .unwrap()
            .put(format!("raw/{}", id), data.to_vec())
            .execute()
            .await
            .unwrap();
        let workflow = state.env.workflow("PARSEWORKFLOW").unwrap();
        workflow
            .create(Some(WorkflowInstanceCreateOptions {
                id: None,
                params: Some(WorkflowInput { id: id.to_string() }),
            }))
            .await
            .unwrap();
    }
    StatusCode::OK
}
