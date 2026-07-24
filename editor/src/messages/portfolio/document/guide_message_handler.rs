use super::utility_types::guide::{GuideLine, GuideLineDirection, GuideLineId};
use crate::messages::portfolio::document::guide_message::{GuideLineMessage, GuideLineMessageDiscriminant};
use crate::messages::portfolio::document::overlays::guide_overlays::guide_lines_overlay;
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, ExtractField)]
#[serde(default)]
pub struct GuideLinesMessageHandler {
	#[serde(default)]
	pub guide_lines: Vec<GuideLine>,
	#[serde(default = "default_guide_lines_visible")]
	pub guide_lines_visible: bool,
	#[serde(skip)]
	pub hovered_guide_line_id: Option<GuideLineId>,
}

fn default_guide_lines_visible() -> bool {
	GuideLinesMessageHandler::default().guide_lines_visible
}

impl GuideLinesMessageHandler {
	pub fn hit_test(&self, viewport_position: DVec2, document_to_viewport: DAffine2) -> Option<(GuideLineId, GuideLineDirection)> {
		if !self.guide_lines_visible {
			return None;
		}
		let viewport_to_document = document_to_viewport.inverse();
		let document_position = viewport_to_document.transform_point2(viewport_position);
		let document_scale = viewport_to_document.matrix2.determinant().abs().sqrt();
		let tolerance = crate::consts::GUIDE_HIT_TOLERANCE * document_scale;

		self.guide_lines
			.iter()
			.find(|guide_line| match guide_line.direction {
				GuideLineDirection::Horizontal => (guide_line.position - document_position.y).abs() < tolerance,
				GuideLineDirection::Vertical => (guide_line.position - document_position.x).abs() < tolerance,
			})
			.map(|guide_line| (guide_line.id, guide_line.direction))
	}
}

impl Default for GuideLinesMessageHandler {
	fn default() -> Self {
		Self {
			guide_lines: Vec::new(),
			guide_lines_visible: true,
			hovered_guide_line_id: None,
		}
	}
}

#[derive(ExtractField)]
pub struct GuideLinesMessageContext {
	pub document_to_viewport: DAffine2,
}

#[message_handler_data]
impl MessageHandler<GuideLineMessage, GuideLinesMessageContext> for GuideLinesMessageHandler {
	fn actions(&self) -> ActionList {
		actions!(GuideLineMessageDiscriminant; ToggleGuideLinesVisibility)
	}

	fn process_message(&mut self, message: GuideLineMessage, responses: &mut VecDeque<Message>, context: GuideLinesMessageContext) {
		let GuideLinesMessageContext { document_to_viewport } = context;
		let viewport_to_document = document_to_viewport.inverse();

		let document_point = |mouse_x, mouse_y| {
			let viewport_point = DVec2::new(mouse_x, mouse_y);
			viewport_to_document.transform_point2(viewport_point)
		};

		match message {
			GuideLineMessage::CreateGuideLine { id, direction, mouse_x, mouse_y } => {
				let document_point = document_point(mouse_x, mouse_y);

				let document_position = match direction {
					GuideLineDirection::Horizontal => document_point.y,
					GuideLineDirection::Vertical => document_point.x,
				};

				responses.add(DocumentMessage::StartTransaction);
				let guide_line = GuideLine::with_id(id, direction, document_position);
				self.guide_lines.push(guide_line);
				responses.add(DocumentMessage::CommitTransaction);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GuideLineMessage::MoveGuideLine { id, mouse_x, mouse_y } => {
				let document_point = document_point(mouse_x, mouse_y);

				if let Some(guide_line) = self.guide_lines.iter_mut().find(|guide_line| guide_line.id == id) {
					guide_line.position = match guide_line.direction {
						GuideLineDirection::Horizontal => document_point.y,
						GuideLineDirection::Vertical => document_point.x,
					};
				}
				responses.add(OverlaysMessage::Draw);
			}
			GuideLineMessage::DeleteGuideLine { id } => {
				responses.add(DocumentMessage::StartTransaction);
				self.guide_lines.retain(|g| g.id != id);
				responses.add(DocumentMessage::CommitTransaction);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GuideLineMessage::GuideLinesOverlays { context: mut overlay_context } => {
				if self.guide_lines_visible {
					guide_lines_overlay(self, &mut overlay_context, document_to_viewport);
				}
			}
			GuideLineMessage::ToggleGuideLinesVisibility => {
				responses.add(DocumentMessage::StartTransaction);
				self.guide_lines_visible = !self.guide_lines_visible;
				responses.add(DocumentMessage::CommitTransaction);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				responses.add(MenuBarMessage::SendLayout);
			}
			GuideLineMessage::SetHoveredGuideLine { id } => {
				if self.hovered_guide_line_id != id {
					self.hovered_guide_line_id = id;
					responses.add(OverlaysMessage::Draw);
				}
			}
		}
	}
}
