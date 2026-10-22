pub mod data_source;
pub mod inline_menu;
pub mod item;
#[cfg(feature = "full_text")]
pub mod macros;
pub mod mixer;
pub mod result_renderer;
#[cfg(feature = "full_text")]
pub mod searcher;

// Re-export paste for use by macros.
pub use paste;
// Re-export tantivy for use by macros.
#[cfg(all(not(target_family = "wasm"), feature = "full_text"))]
pub use tantivy;
