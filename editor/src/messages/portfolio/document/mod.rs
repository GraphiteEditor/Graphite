pub(crate) mod document_diff;
mod document_history;
mod document_message;
mod document_message_handler;
#[cfg(test)]
mod storage;

pub mod data_panel;
pub mod graph_operation;
pub mod navigation;
pub mod node_graph;
pub mod overlays;
pub mod properties_panel;
pub mod resource;
pub mod utility_types;

pub(crate) use document_diff::diff_networks;
pub(crate) use document_history::DocumentHistory;
#[doc(inline)]
pub use document_message::{DocumentMessage, DocumentMessageDiscriminant};
#[doc(inline)]
pub use document_message_handler::{DocumentMessageContext, DocumentMessageHandler};
