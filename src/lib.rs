mod api;
mod app;
mod components;
#[cfg(feature = "ssr")]
mod upload;
#[cfg(feature = "ssr")]
mod workflows;
use axum::extract::FromRef;
#[cfg(feature = "ssr")]
use leptos::config::LeptosOptions;
#[cfg(feature = "ssr")]
pub use workflows::*;

#[cfg(feature = "ssr")]
#[derive(FromRef, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub env: worker::Env,
}

#[cfg(feature = "ssr")]
#[worker::event(fetch)]
async fn fetch(
    req: worker::HttpRequest,
    env: worker::Env,
    _ctx: worker::Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    use std::sync::Arc;

    use axum::{Extension, Router, routing::post};
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use tower_service::Service;

    use app::{shell, App};

    use crate::upload::upload;

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);
    let state = AppState{ leptos_options: leptos_options.clone(), env: env.clone() };

    // build our application with a route
    let mut router = Router::new()
        .route("/upload", post(upload))
        .leptos_routes(&state, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .with_state(state)
        .layer(Extension(Arc::new(env))); // <- Allow leptos server functions to access Worker stuff

    Ok(router.call(req).await?)
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(app::App);
}
