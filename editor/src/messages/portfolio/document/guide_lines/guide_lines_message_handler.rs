use crate::messages::portfolio::document::guide_lines::GuideLinesMessage;
use crate::messages::portfolio::document::utility_types::guide::{GuideLine, GuideLineDirection, GuideLineId};
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};

/// Holds a document's guide lines, the user-placed lines spanning the canvas that things snap to.
///
/// Like grid settings, guides are view state rather than document content, so they are not undoable.
#[derive(Debug, Default, Clone, ExtractField, serde::Serialize, serde::Deserialize)]
pub struct GuideLinesMessageHandler {
	/// Every guide line in the document, in creation order.
	pub guide_lines: Vec<GuideLine>,
	/// The guide line the current drag moves. Transient, so it isn't part of the saved settings.
	#[serde(skip)]
	dragging: Option<GuideLineId>,
}

#[derive(ExtractField)]
pub struct GuideLinesMessageContext {
	pub document_to_viewport: DAffine2,
}

#[message_handler_data]
impl MessageHandler<GuideLinesMessage, GuideLinesMessageContext> for GuideLinesMessageHandler {
	fn process_message(&mut self, message: GuideLinesMessage, _responses: &mut VecDeque<Message>, context: GuideLinesMessageContext) {
		let GuideLinesMessageContext { document_to_viewport } = context;

		match message {
			GuideLinesMessage::BeginCreate { direction, viewport_position } => {
				let position = guide_position(direction, viewport_position, document_to_viewport);
				let guide_line = GuideLine::new(direction, position);
				self.dragging = Some(guide_line.id);
				self.guide_lines.push(guide_line);
			}
			GuideLinesMessage::BeginGrab { id } => {
				self.dragging = self.guide_lines.iter().find(|guide_line| guide_line.id == id).map(|guide_line| guide_line.id);
			}
			GuideLinesMessage::Drag { viewport_position } => {
				let Some(dragged) = self.dragging else { return };
				let Some(guide_line) = self.guide_lines.iter_mut().find(|guide_line| guide_line.id == dragged) else {
					return;
				};
				guide_line.position = guide_position(guide_line.direction, viewport_position, document_to_viewport);
			}
			GuideLinesMessage::EndDrag { discard } => {
				// Releasing back over the ruler, or canceling, throws away a line the drag drew rather than leaving it behind
				if discard && let Some(dragged) = self.dragging {
					self.guide_lines.retain(|guide_line| guide_line.id != dragged);
				}
				self.dragging = None;
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GuideLinesMessage;)
	}
}

/// The document-space position a guide line takes to pass through a viewport point: its Y when horizontal, its X when vertical.
fn guide_position(direction: GuideLineDirection, viewport_position: DVec2, document_to_viewport: DAffine2) -> f64 {
	let point = document_to_viewport.inverse().transform_point2(viewport_position);
	match direction {
		GuideLineDirection::Horizontal => point.y,
		GuideLineDirection::Vertical => point.x,
	}
}

#[cfg(test)]
mod test {
	use super::*;

	fn create(handler: &mut GuideLinesMessageHandler, direction: GuideLineDirection, position: DVec2, document_to_viewport: DAffine2) -> GuideLineId {
		handler.process_message(
			GuideLinesMessage::BeginCreate {
				direction,
				viewport_position: position,
			},
			&mut VecDeque::new(),
			GuideLinesMessageContext { document_to_viewport },
		);
		handler.guide_lines.last().expect("a begun guide is added").id
	}

	fn process(handler: &mut GuideLinesMessageHandler, message: GuideLinesMessage, document_to_viewport: DAffine2) {
		handler.process_message(message, &mut VecDeque::new(), GuideLinesMessageContext { document_to_viewport });
	}

	#[test]
	fn test_created_guide_takes_the_document_position_behind_a_viewport_point() {
		let mut handler = GuideLinesMessageHandler::default();
		let document_to_viewport = DAffine2::from_translation(DVec2::new(100., 50.));

		create(&mut handler, GuideLineDirection::Horizontal, DVec2::new(30., 40.), document_to_viewport);

		assert_eq!(handler.guide_lines.len(), 1);
		let expected = document_to_viewport.inverse().transform_point2(DVec2::new(30., 40.)).y;
		assert!(
			(handler.guide_lines[0].position - expected).abs() < 1e-6,
			"a horizontal guide takes the document Y, expected {expected}, got {}",
			handler.guide_lines[0].position
		);
	}

	#[test]
	fn test_guide_created_on_a_tilted_canvas_lands_where_the_pointer_was() {
		let mut handler = GuideLinesMessageHandler::default();
		let document_to_viewport = DAffine2::from_angle_translation(std::f64::consts::FRAC_PI_4, DVec2::new(20., -10.));

		create(&mut handler, GuideLineDirection::Vertical, DVec2::new(100., 60.), document_to_viewport);

		let expected = document_to_viewport.inverse().transform_point2(DVec2::new(100., 60.)).x;
		assert!((handler.guide_lines[0].position - expected).abs() < 1e-6, "a vertical guide should follow the tilted transform");
	}

	#[test]
	fn test_dragged_guide_follows_the_pointer_under_its_own_id() {
		let mut handler = GuideLinesMessageHandler::default();
		let first = create(&mut handler, GuideLineDirection::Vertical, DVec2::new(10., 0.), DAffine2::IDENTITY);
		let second = create(&mut handler, GuideLineDirection::Vertical, DVec2::new(80., 0.), DAffine2::IDENTITY);

		// Grab the first guide by id and move it, leaving the second where it was
		process(&mut handler, GuideLinesMessage::BeginGrab { id: first }, DAffine2::IDENTITY);
		process(
			&mut handler,
			GuideLinesMessage::Drag {
				viewport_position: DVec2::new(55., 0.),
			},
			DAffine2::IDENTITY,
		);
		process(&mut handler, GuideLinesMessage::EndDrag { discard: false }, DAffine2::IDENTITY);

		assert!((handler.guide_lines[0].position - 55.).abs() < 1e-6, "the grabbed guide should have moved");
		assert!(
			(handler.guide_lines[1].position - 80.).abs() < 1e-6,
			"dragging one guide must not move the next, which is what naming it by index got wrong"
		);
		assert_eq!(handler.guide_lines[1].id, second);
	}

	#[test]
	fn test_throwing_away_a_drag_removes_the_guide_it_drew() {
		let mut handler = GuideLinesMessageHandler::default();
		let kept = create(&mut handler, GuideLineDirection::Horizontal, DVec2::new(0., 10.), DAffine2::IDENTITY);
		process(&mut handler, GuideLinesMessage::EndDrag { discard: false }, DAffine2::IDENTITY);

		// A later drag released where it began leaves nothing behind, and leaves the earlier guide alone
		create(&mut handler, GuideLineDirection::Horizontal, DVec2::new(0., 40.), DAffine2::IDENTITY);
		process(&mut handler, GuideLinesMessage::EndDrag { discard: true }, DAffine2::IDENTITY);

		assert_eq!(handler.guide_lines.len(), 1);
		assert_eq!(handler.guide_lines[0].id, kept, "the guide the discarded drag drew should be the one removed");
	}

	#[test]
	fn test_drag_that_ends_without_a_guide_is_ignored() {
		let mut handler = GuideLinesMessageHandler::default();

		// Stray moves and releases, such as from a pointer capture that outlived its drag, must not panic or invent a guide
		process(
			&mut handler,
			GuideLinesMessage::Drag {
				viewport_position: DVec2::new(5., 5.),
			},
			DAffine2::IDENTITY,
		);
		process(&mut handler, GuideLinesMessage::EndDrag { discard: true }, DAffine2::IDENTITY);
		process(&mut handler, GuideLinesMessage::BeginGrab { id: GuideLineId::new() }, DAffine2::IDENTITY);

		assert!(handler.guide_lines.is_empty());
	}
}
