mod document_message;
mod document_message_handler;
#[cfg(test)]
mod storage_round_trip_tests;

pub mod data_panel;
pub mod graph_operation;
pub mod navigation;
pub mod node_graph;
pub mod overlays;
pub mod properties_panel;
pub mod resource;
pub mod utility_types;

#[doc(inline)]
pub use document_message::{DocumentMessage, DocumentMessageDiscriminant};
pub(crate) use document_message_handler::diff_networks;
#[doc(inline)]
pub use document_message_handler::{DocumentMessageContext, DocumentMessageHandler};
