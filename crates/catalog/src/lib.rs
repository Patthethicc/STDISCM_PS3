//! Catalog of processed videos.
//!
//! The consumer workers are the only writers (`Catalog::add`), and the GUI only reads
//! (`read_catalog`). The catalog lives in `<data_dir>/catalog.json`, and every write
//! goes to a temp file that is then renamed over the old one, so the GUI process
//! never sees a half-written file.

mod model;
mod store;

pub use model::VideoEntry;
pub use store::{read_catalog, Catalog, CATALOG_FILE};
