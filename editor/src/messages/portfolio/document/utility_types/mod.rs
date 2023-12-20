pub mod clipboards;
pub mod document_metadata;
pub mod error;
pub mod layer_panel;
pub mod misc;
pub mod transformation;

// TODO: Remove this entirely
/// A number that identifies a layer.
/// This does not technically need to be unique globally, only within a folder.
pub type LayerId = u64;
