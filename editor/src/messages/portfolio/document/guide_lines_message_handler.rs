use super::utility_types::guide_line::{GuideLine, GuideLineDirection, GuideLineId};
use crate::messages::portfolio::document::guide_lines_message::{GuideLinesMessage, GuideLinesMessageDiscriminant};
use crate::messages::portfolio::document::overlays::guide_line_overlays::guide_line_overlay;
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::prelude::*;
use glam::DVec2;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, ExtractField)]
#[serde(default)]
pub struct GuideLinesMessageHandler {
	pub guide_lines: Vec<GuideLine>,
	pub guide_lines_visible: bool,
	#[serde(skip)]
	pub hovered_guide_line_id: Option<GuideLineId>,
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
pub struct GuideLinesMessageContext<'a> {
	pub navigation_handler: &'a NavigationMessageHandler,
	pub document_ptz: &'a PTZ,
	pub viewport: &'a ViewportMessageHandler,
}

#[message_handler_data]
impl MessageHandler<GuideLinesMessage, GuideLinesMessageContext<'_>> for GuideLinesMessageHandler {
	fn actions(&self) -> ActionList {
		actions!(GuideLinesMessageDiscriminant; ToggleGuideLinesVisibility)
	}

	fn process_message(&mut self, message: GuideLinesMessage, responses: &mut VecDeque<Message>, context: GuideLinesMessageContext) {
		let GuideLinesMessageContext {
			navigation_handler,
			document_ptz,
			viewport,
		} = context;

		let viewport_to_document_point = |mouse_x: f64, mouse_y: f64| -> DVec2 {
			let document_to_viewport = navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), document_ptz);
			document_to_viewport.inverse().transform_point2(DVec2::new(mouse_x, mouse_y))
		};

		match message {
			GuideLinesMessage::CreateGuideLine { id, direction, mouse_x, mouse_y } => {
				let document_point = viewport_to_document_point(mouse_x, mouse_y);

				let document_position = match direction {
					GuideLineDirection::Horizontal => document_point.y,
					GuideLineDirection::Vertical => document_point.x,
				};

				let guide = GuideLine::with_id(id, direction, document_position);
				self.guide_lines.push(guide);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GuideLinesMessage::MoveGuideLine { id, mouse_x, mouse_y } => {
				let document_point = viewport_to_document_point(mouse_x, mouse_y);

				if let Some(guide) = self.guide_lines.iter_mut().find(|guide| guide.id == id) {
					guide.position = match guide.direction {
						GuideLineDirection::Horizontal => document_point.y,
						GuideLineDirection::Vertical => document_point.x,
					};
				}
				responses.add(OverlaysMessage::Draw);
			}
			GuideLinesMessage::DeleteGuideLine { id } => {
				self.guide_lines.retain(|g| g.id != id);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GuideLinesMessage::GuideLineOverlays { context: mut overlay_context } => {
				if self.guide_lines_visible {
					let document_to_viewport = navigation_handler.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), document_ptz);
					guide_line_overlay(self, &mut overlay_context, document_to_viewport);
				}
			}
			GuideLinesMessage::ToggleGuideLinesVisibility => {
				self.guide_lines_visible = !self.guide_lines_visible;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				responses.add(MenuBarMessage::SendLayout);
			}
			GuideLinesMessage::SetHoveredGuideLine { id } => {
				if self.hovered_guide_line_id != id {
					self.hovered_guide_line_id = id;
					responses.add(OverlaysMessage::Draw);
				}
			}
		}
	}
}
