#[cfg(feature = "ssr")]
use axum::Extension;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::extract;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[cfg(feature = "ssr")]
use worker::Env;

#[derive(Serialize, Deserialize, Clone)]
pub struct Note {
    pub title: String,
    pub cleaned: String,
    pub summary: String,
}

#[cfg_attr(feature = "ssr", worker::send)]
#[server]
pub async fn get_data() -> Result<Vec<Note>, ServerFnError> {
    let Extension(env): Extension<Arc<Env>> = extract().await.unwrap();
    let kv = env.kv("NOTESKV").unwrap();
    let r2 = env.bucket("NOTES").unwrap();
    let count: i32 = kv
        .get("count")
        .text()
        .await
        .unwrap()
        .unwrap()
        .parse()
        .unwrap();

    let mut out = Vec::new();
    for i in 0..count {
        let title = match kv.get(&i.to_string()).text().await.unwrap() {
            Some(t) => t,
            None => continue,
        };

        let cleaned = match r2.get(format!("cleaned/{}", i)).execute().await.unwrap() {
            Some(t) => t.body().unwrap().text().await.unwrap(),
            None => continue,
        };

        let summary = match r2.get(format!("summary/{}", i)).execute().await.unwrap() {
            Some(t) => t.body().unwrap().text().await.unwrap(),
            None => continue,
        };

        out.push(Note {
            title,
            cleaned,
            summary,
        });
    }

    Ok(out)
}
