use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::guide_line::{GuideLineDirection, GuideLineId};
use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, GuideLines)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GuideLinesMessage {
	CreateGuideLine {
		id: GuideLineId,
		direction: GuideLineDirection,
		mouse_x: f64,
		mouse_y: f64,
	},
	MoveGuideLine {
		id: GuideLineId,
		mouse_x: f64,
		mouse_y: f64,
	},
	DeleteGuideLine {
		id: GuideLineId,
	},
	GuideLineOverlays {
		context: OverlayContext,
	},
	ToggleGuideLinesVisibility,
	SetHoveredGuideLine {
		id: Option<GuideLineId>,
	},
}
