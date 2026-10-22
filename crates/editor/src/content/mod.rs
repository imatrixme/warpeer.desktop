pub mod anchor;
pub mod buffer;
pub mod core;
mod cursor;
pub mod diff;
pub mod edit;
pub mod find;
pub mod hidden_lines_model;
pub mod markdown;
#[cfg(feature = "mermaid")]
pub mod mermaid_diagram;
#[cfg(not(feature = "mermaid"))]
#[path = "mermaid_diagram_disabled.rs"]
pub mod mermaid_diagram;
pub mod outline;
mod segmentation;
pub mod selection;
pub mod selection_model;
pub mod text;
pub mod undo;
mod validation;
pub mod version;
