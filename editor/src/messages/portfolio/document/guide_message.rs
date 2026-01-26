use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::guide::{GuideDirection, GuideId};
use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, Guide)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GuideMessage {
	CreateGuide { id: GuideId, direction: GuideDirection, mouse_x: f64, mouse_y: f64 },
	MoveGuide { id: GuideId, mouse_x: f64, mouse_y: f64 },
	DeleteGuide { id: GuideId },
	GuideOverlays { context: OverlayContext },
	ToggleGuidesVisibility,
	SetHoveredGuide { id: Option<GuideId> },
}
