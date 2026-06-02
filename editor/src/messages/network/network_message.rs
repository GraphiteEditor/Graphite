use std::pin::Pin;

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
		F: FnOnce(Client) -> Fut + Send + 'static,
		Fut: Future<Output = Message> + Send + 'static,
	{
		NetworkMessage::Request {
			request: Some(Box::new(move |c| Box::pin(f(c)))),
		}
	}
}

type RequestFuture = Pin<Box<dyn Future<Output = Message> + Send>>;
type RequestFn = Box<dyn FnOnce(Client) -> RequestFuture + Send>;

// Custom clone implementation to avoid cloning the request function
impl Clone for NetworkMessage {
	fn clone(&self) -> Self {
		match self {
			NetworkMessage::Request { .. } => NetworkMessage::Request { request: None },
		}
	}
}
