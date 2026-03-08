use leptos::{prelude::*, task::spawn_local};
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::get_data::{get_data, Note};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/notes-cf.css"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    let (notes, set_notes) = signal(None);
    Effect::new(move || {
        spawn_local(async move {
            match get_data().await {
                Ok(n) => set_notes.set(Some(n)),
                Err(_) => set_notes.set(None),
            }
        })
    });

    view! {
        <div class="min-h-screen bg-linear-to-br from-slate-50 to-slate-100 p-6">
            <div class="max-w-2xl mx-auto">
                <h1 class="text-4xl font-bold text-slate-900 mb-8">My Notes</h1>

                {move || match notes.get() {
                    None => view! { <div class="text-center py-12"><p class="text-slate-500">Loading notes...</p></div> }.into_any(),
                    Some(note_list) => {
                        if note_list.is_empty() {
                            view! {
                                <div class="text-center py-12"><p class="text-slate-500">No notes yet</p></div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="space-y-4">
                                    <For
                                        each=move || note_list.clone()
                                        key=|note| note.title.clone()
                                        children=move |note| {
                                            view! {
                                                <NoteCard note=note />
                                            }
                                        }
                                    />
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn NoteCard(note: Note) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);

    let truncate_text = |text: &str, max_length: usize| -> String {
        if text.len() > max_length {
            format!("{}...", &text[..max_length])
        } else {
            text.to_string()
        }
    };

    let summary_preview = truncate_text(&note.summary, 150);

    view! {
    <div
        class="bg-white rounded-lg shadow-sm hover:shadow-md transition-shadow cursor-pointer border border-slate-200"
        on:click=move |_| set_expanded.update(|v| *v = !*v)
    >
        <div class="p-5">
            <div class="flex items-center justify-between">
                <h2 class="text-xl font-semibold text-slate-900 flex-1 pr-3">{note.title.clone()}</h2>
                <span class="text-2xl text-slate-400 shrink-0">
                    {move || if expanded.get() { "▼" } else { "▶" }}
                </span>
            </div>

            <p class="text-slate-600 mt-2 text-sm">{summary_preview}</p>

            {move || if expanded.get() {
                view! {
                    <div class="mt-4 pt-4 border-t border-slate-200 space-y-4">
                        <div>
                            <h3 class="font-semibold text-slate-900 mb-2">Summary</h3>
                            <p class="text-slate-600 text-sm whitespace-pre-wrap">{note.summary.clone()}</p>
                        </div>
                        <div>
                            <h3 class="font-semibold text-slate-900 mb-2">Full Content</h3>
                            <div class="bg-slate-50 p-3 rounded border border-slate-200 text-sm text-slate-700 max-h-64 overflow-y-auto whitespace-pre-wrap">
                                {note.cleaned.clone()}
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { }.into_any()
            }}
        </div>
    </div>
        }
}
