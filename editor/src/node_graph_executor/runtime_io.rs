use super::*;
use std::sync::mpsc::{Receiver, Sender};

/// Handles communication with the NodeRuntime
#[derive(Debug)]
pub struct NodeRuntimeIO {
	// Send to
	sender: Sender<GraphRuntimeRequest>,
	receiver: Receiver<NodeGraphUpdate>,
}

impl Default for NodeRuntimeIO {
	fn default() -> Self {
		Self::new()
	}
}

impl NodeRuntimeIO {
	/// Creates a new NodeRuntimeIO instance
	pub fn new() -> Self {
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		let (request_sender, request_receiver) = std::sync::mpsc::channel();
		futures::executor::block_on(replace_node_runtime(NodeRuntime::new(request_receiver, response_sender)));

		Self {
			sender: request_sender,
			receiver: response_receiver,
		}
	}
	#[cfg(test)]
	pub fn with_channels(sender: Sender<GraphRuntimeRequest>, receiver: Receiver<NodeGraphUpdate>) -> Self {
		Self { sender, receiver }
	}

	/// Sends a message to the NodeRuntime
	pub fn send(&self, message: GraphRuntimeRequest) -> Result<(), String> {
		self.sender.send(message).map_err(|e| e.to_string())
	}

	/// Receives any pending updates from the NodeRuntime
	pub fn receive(&self) -> impl Iterator<Item = NodeGraphUpdate> + use<'_> {
		self.receiver.try_iter()
	}
}
