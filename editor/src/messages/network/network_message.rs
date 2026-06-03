use std::pin::Pin;

use dyn_any::WasmNotSend;

use crate::messages::network::utility_types::Client;
use crate::messages::prelude::*;

#[impl_message(Message, Network)]
#[derive(derivative::Derivative, serde::Serialize, serde::Deserialize)]
#[derivative(Debug, PartialEq)]
pub enum NetworkMessage {
	Request {
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		request: Option<RequestFn>,
	},
}
impl NetworkMessage {
	pub fn request<F, Fut>(f: F) -> Self
	where
		F: FnOnce(Client) -> Fut + WasmNotSend + 'static,
		Fut: Future<Output = Message> + WasmNotSend + 'static,
	{
		NetworkMessage::Request {
			request: Some(Box::new(move |c| Box::pin(f(c)))),
		}
	}
}

#[cfg(not(target_family = "wasm"))]
type RequestFuture = Pin<Box<dyn Future<Output = Message> + Send>>;
#[cfg(target_family = "wasm")]
type RequestFuture = Pin<Box<dyn Future<Output = Message>>>;

#[cfg(not(target_family = "wasm"))]
type RequestFn = Box<dyn FnOnce(Client) -> RequestFuture + Send>;
#[cfg(target_family = "wasm")]
type RequestFn = Box<dyn FnOnce(Client) -> RequestFuture>;

impl Clone for NetworkMessage {
	fn clone(&self) -> Self {
		match self {
			NetworkMessage::Request { .. } => NetworkMessage::Request { request: None },
		}
	}
}
