mod node_graph_message;
mod node_graph_message_handler;

#[doc(inline)]
pub use node_graph_message::{NodeGraphMessage, NodeGraphMessageDiscriminant};
#[doc(inline)]
pub use node_graph_message_handler::*;

mod graph_operation_message;
mod graph_operation_message_handler;

#[doc(inline)]
pub use graph_operation_message::*;
#[doc(inline)]
pub use graph_operation_message_handler::*;
