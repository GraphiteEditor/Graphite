use super::utility_types::{empty_provider, OverlayProvider};
use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, Overlays)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum OverlaysMessage {
	Draw,

	// Serde functionality isn't used but is required by the message system macros
	AddProvider(#[serde(skip, default = "empty_provider")] OverlayProvider),
	RemoveProvider(#[serde(skip, default = "empty_provider")] OverlayProvider),
}
