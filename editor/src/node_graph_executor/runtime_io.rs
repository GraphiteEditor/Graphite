use super::*;
use std::sync::mpsc::{Receiver, Sender};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
	// invoke with arguments (default)
	#[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
	async fn invoke(cmd: &str, args: JsValue) -> JsValue;
	#[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name="invoke")]
	async fn invoke_without_arg(cmd: &str) -> JsValue;
}

/// Handles communication with the NodeRuntime, either locally or via Tauri
#[derive(Debug)]
pub struct NodeRuntimeIO {
	// Send to
	#[cfg(any(not(feature = "tauri"), test))]
	sender: Sender<NodeRuntimeMessage>,
	#[cfg(all(feature = "tauri", not(test)))]
	sender: Sender<NodeGraphUpdate>,
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
		#[cfg(any(not(feature = "tauri"), test))]
		{
			let (response_sender, response_receiver) = std::sync::mpsc::channel();
			let (request_sender, request_receiver) = std::sync::mpsc::channel();
			futures::executor::block_on(replace_node_runtime(NodeRuntime::new(request_receiver, response_sender)));

			Self {
				sender: request_sender,
				receiver: response_receiver,
			}
		}

		#[cfg(all(feature = "tauri", not(test)))]
		{
			let (response_sender, response_receiver) = std::sync::mpsc::channel();
			Self {
				sender: response_sender,
				receiver: response_receiver,
			}
		}
	}
	#[cfg(test)]
	pub fn with_channels(sender: Sender<NodeRuntimeMessage>, receiver: Receiver<NodeGraphUpdate>) -> Self {
		Self { sender, receiver }
	}

	/// Sends a message to the NodeRuntime
	pub fn send(&self, message: NodeRuntimeMessage) -> Result<(), String> {
		#[cfg(any(not(feature = "tauri"), test))]
		{
			self.sender.send(message).map_err(|e| e.to_string())
		}

		#[cfg(all(feature = "tauri", not(test)))]
		{
			let serialized = ron::to_string(&message).map_err(|e| e.to_string()).unwrap();
			wasm_bindgen_futures::spawn_local(async move {
				let js_message = create_message_object(&serialized);
				invoke("runtime_message", js_message).await;
			});
			Ok(())
		}
	}

	/// Receives any pending updates from the NodeRuntime
	pub fn receive(&self) -> impl Iterator<Item = NodeGraphUpdate> + use<'_> {
		// TODO: This introduces extra latency
		#[cfg(all(feature = "tauri", not(test)))]
		{
			let sender = self.sender.clone();
			// In the Tauri case, responses are handled separately via poll_node_runtime_updates
			wasm_bindgen_futures::spawn_local(async move {
				let messages = invoke_without_arg("poll_node_graph").await;
				let vec: Vec<_> = ron::from_str(&messages.as_string().unwrap()).unwrap();
				for message in vec {
					sender.send(message).unwrap();
				}
			});
		}
		self.receiver.try_iter()
	}
}

#[cfg(feature = "tauri")]
pub fn create_message_object(message: &str) -> JsValue {
	let obj = js_sys::Object::new();
	js_sys::Reflect::set(&obj, &JsValue::from_str("message"), &JsValue::from_str(message)).unwrap();
	obj.into()
}
