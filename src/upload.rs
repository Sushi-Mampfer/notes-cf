use std::sync::Arc;

use crate::AppState;
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
};

use leptos::prelude::*;
use leptos_axum::extract;
use serde::{Deserialize, Serialize};
use workflows_rs::{EnvWorkflowExt, WorkflowInstanceCreateOptions};

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Input {
    pub name: String,
    pub audio: String,
}

#[derive(Serialize)]
struct WorkflowInput {
    pub id: String,
    pub audio: String,
}

#[server]
pub async fn upload(data: Input) -> Result<String, ServerFnError> {
    use axum::Extension;

    let env = expect_context::<Extension<Arc<worker::Env>>>();
    let kv = env.kv("NOTESKV").unwrap();
    let id = match kv.get("count").text().await.unwrap() {
        Some(n) => n.parse::<i32>().unwrap() + 1,
        None => 0_i32,
    };
    kv.put("count", &id).unwrap().execute().await.unwrap();
    kv.put(&id.to_string(), data.name)
        .unwrap()
        .execute()
        .await
        .unwrap();
    let workflow = env.workflow("PARSEWORKFLOW").unwrap();
    workflow
        .create(Some(WorkflowInstanceCreateOptions {
            id: None,
            params: Some(WorkflowInput {
                id: id.to_string(),
                audio: data.audio,
            }),
        }))
        .await;
    Ok("Yay".to_string())
}
