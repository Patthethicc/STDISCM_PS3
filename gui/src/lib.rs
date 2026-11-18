//! Consumer GUI: a web page showing the uploaded videos.
//!
//! Runs as its own process on the consumer VM and reads the consumer's data folder
//! (catalog.json + uploads/previews/thumbnails). Pages are rendered on the server with
//! Leptos; `public/app.js` handles the hover preview, the player modal and auto-refresh.

pub mod app;
pub mod components;
pub mod pages;
pub mod server;
