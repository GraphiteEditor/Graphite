use super::constants::ControlPointType;
use crate::{
	consts::COLOR_ACCENT,
	message_prelude::{DocumentMessage, Message},
};

use graphene::{
	color::Color,
	layers::style::{Fill, PathStyle, Stroke},
	LayerId, Operation,
};

use glam::DVec2;
use std::collections::VecDeque;

/// VectorControlPoint represents any grabbable point, anchor or handle
#[derive(PartialEq, Clone, Debug)]
pub struct VectorControlPoint {
	// The associated position in the BezPath
	pub kurbo_element_id: usize,
	// The sibling element if this is a handle
	pub position: glam::DVec2,
	// The path to the overlay for this point rendering
	pub overlay_path: Option<Vec<LayerId>>,
	// The type of manipulator this point is
	pub manipulator_type: ControlPointType,
	// Can be selected
	pub can_be_selected: bool,
	// Is this point currently selected?
	pub is_selected: bool,
}

impl Default for VectorControlPoint {
	fn default() -> Self {
		Self {
			kurbo_element_id: 0,
			position: DVec2::ZERO,
			overlay_path: None,
			manipulator_type: ControlPointType::Anchor,
			can_be_selected: true,
			is_selected: false,
		}
	}
}

const POINT_STROKE_WIDTH: f32 = 2.0;

impl VectorControlPoint {
	/// Sets if this point is selected and updates the overlay to represent that
	pub fn set_selected(&mut self, selected: bool, responses: &mut VecDeque<Message>) {
		if selected {
			self.set_overlay_style(POINT_STROKE_WIDTH + 1.0, COLOR_ACCENT, COLOR_ACCENT, responses);
		} else {
			self.set_overlay_style(POINT_STROKE_WIDTH, COLOR_ACCENT, Color::WHITE, responses);
		}
		self.is_selected = selected;
	}

	/// Sets the overlay style for this point
	pub fn set_overlay_style(&self, stroke_width: f32, stroke_color: Color, fill_color: Color, responses: &mut VecDeque<Message>) {
		if let Some(overlay_path) = &self.overlay_path {
			responses.push_back(
				DocumentMessage::Overlays(
					Operation::SetLayerStyle {
						path: overlay_path.clone(),
						style: PathStyle::new(Some(Stroke::new(stroke_color, stroke_width)), Fill::solid(fill_color)),
					}
					.into(),
				)
				.into(),
			);
		}
	}
}
