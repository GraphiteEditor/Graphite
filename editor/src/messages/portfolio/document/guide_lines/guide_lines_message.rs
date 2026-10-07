use crate::messages::portfolio::document::utility_types::guide::{GuideLineDirection, GuideLineId};
use crate::messages::prelude::*;
use glam::DVec2;

/// Operations on a document's guide lines.
///
/// A drag is three messages: one that starts it, any number that carry the pointer, and one that ends it. The handler
/// remembers which line the drag holds, so the frontend never names a line it creates. Positions are viewport space so the
/// backend converts them, which keeps the conversion correct on a tilted canvas.
#[impl_message(Message, DocumentMessage, GuideLines)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuideLinesMessage {
	/// Starts drawing a new guide line, which follows the pointer until the drag ends.
	BeginCreate { direction: GuideLineDirection, viewport_position: DVec2 },
	/// Starts dragging the guide line with this id, which the ruler read off the line it was drawn from.
	BeginGrab { id: GuideLineId },
	/// Moves whichever guide line the current drag holds.
	Drag { viewport_position: DVec2 },
	/// Ends the current drag. A guide drawn by a discarded drag is removed rather than left behind.
	EndDrag { discard: bool },
}
