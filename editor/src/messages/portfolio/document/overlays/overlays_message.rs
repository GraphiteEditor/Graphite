use super::utility_types::{OverlayProvider, empty_provider};
use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, Overlays)]
#[derive(derivative::Derivative, Clone, serde::Serialize, serde::Deserialize)]
#[derivative(Debug, PartialEq)]
pub enum OverlaysMessage {
	Draw,
	// Serde functionality isn't used but is required by the message system macros
	AddProvider(
		#[serde(skip, default = "empty_provider")]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		OverlayProvider,
	),
	RemoveProvider(
		#[serde(skip, default = "empty_provider")]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		OverlayProvider,
	),
}
