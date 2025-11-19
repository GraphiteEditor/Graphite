use crate::messages::prelude::*;

#[impl_message(Message, Viewport)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ViewportMessage {
	Update { x: f64, y: f64, width: f64, height: f64, scale: f64 },
	RepropagateUpdate,
}
