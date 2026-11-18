use leptos::prelude::*;

const CSS: &str = include_str!("../style/main.css");
const JS: &str = include_str!("../public/app.js");

/// Renders a Leptos view to an HTML string.
pub fn render<V: IntoView + 'static>(f: impl FnOnce() -> V + 'static) -> String {
    Owner::new().with(|| f().into_view().to_html())
}

/// Wraps page content in the full HTML document.
pub fn document(title: &str, body: String) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><style>{CSS}</style></head><body>{body}<script>{JS}</script></body></html>",
        escape(title)
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
