use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::guide::{GuideLineDirection, GuideLineId};
use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, GuideLine)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GuideLineMessage {
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
	GuideLinesOverlays {
		context: OverlayContext,
	},
	ToggleGuideLinesVisibility,
	SetHoveredGuideLine {
		id: Option<GuideLineId>,
	},
}
