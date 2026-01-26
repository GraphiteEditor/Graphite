use super::utility_types::guide::{Guide, GuideDirection, GuideId};
use crate::messages::portfolio::document::guide_message::{GuideMessage, GuideMessageDiscriminant};
use crate::messages::portfolio::document::overlays::guide_overlays::guide_overlay;
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::prelude::*;
use glam::DVec2;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, ExtractField)]
#[serde(default)]
pub struct GuideMessageHandler {
	#[serde(default)]
	pub guides: Vec<Guide>,
	#[serde(default = "default_guides_visible")]
	pub guides_visible: bool,
	#[serde(skip)]
	pub hovered_guide_id: Option<GuideId>,
}

fn default_guides_visible() -> bool {
	true
}

impl Default for GuideMessageHandler {
	fn default() -> Self {
		Self {
			guides: Vec::new(),
			guides_visible: true,
			hovered_guide_id: None,
		}
	}
}

#[derive(ExtractField)]
pub struct GuideMessageContext<'a> {
	pub navigation_handler: &'a NavigationMessageHandler,
	pub document_ptz: &'a PTZ,
	pub viewport: &'a ViewportMessageHandler,
}

#[message_handler_data]
impl MessageHandler<GuideMessage, GuideMessageContext<'_>> for GuideMessageHandler {
	fn actions(&self) -> ActionList {
		actions!(GuideMessageDiscriminant; ToggleGuidesVisibility)
	}

	fn process_message(&mut self, message: GuideMessage, responses: &mut VecDeque<Message>, context: GuideMessageContext) {
		let GuideMessageContext {
			navigation_handler,
			document_ptz,
			viewport,
		} = context;

		match message {
			GuideMessage::CreateGuide { id, direction, mouse_x, mouse_y } => {
				let document_to_viewport = navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), document_ptz);
				let viewport_to_document = document_to_viewport.inverse();

				let viewport_point = DVec2::new(mouse_x, mouse_y);
				let document_point = viewport_to_document.transform_point2(viewport_point);

				let document_position = match direction {
					GuideDirection::Horizontal => document_point.y,
					GuideDirection::Vertical => document_point.x,
				};

				let guide = Guide::with_id(id, direction, document_position);
				self.guides.push(guide);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GuideMessage::MoveGuide { id, mouse_x, mouse_y } => {
				let document_to_viewport = navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), document_ptz);
				let viewport_to_document = document_to_viewport.inverse();

				let viewport_point = DVec2::new(mouse_x, mouse_y);
				let document_point = viewport_to_document.transform_point2(viewport_point);

				if let Some(guide) = self.guides.iter_mut().find(|guide| guide.id == id) {
					guide.position = match guide.direction {
						GuideDirection::Horizontal => document_point.y,
						GuideDirection::Vertical => document_point.x,
					};
				}
				responses.add(OverlaysMessage::Draw);
			}
			GuideMessage::DeleteGuide { id } => {
				self.guides.retain(|g| g.id != id);
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GuideMessage::GuideOverlays { context: mut overlay_context } => {
				if self.guides_visible {
					let document_to_viewport = navigation_handler.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), document_ptz);
					guide_overlay(self, &mut overlay_context, document_to_viewport);
				}
			}
			GuideMessage::ToggleGuidesVisibility => {
				self.guides_visible = !self.guides_visible;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				responses.add(MenuBarMessage::SendLayout);
			}
			GuideMessage::SetHoveredGuide { id } => {
				if self.hovered_guide_id != id {
					self.hovered_guide_id = id;
					responses.add(OverlaysMessage::Draw);
				}
			}
		}
	}
}
