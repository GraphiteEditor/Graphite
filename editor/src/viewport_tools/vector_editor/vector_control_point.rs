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
	// pub kurbo_element_id: usize,
	// The sibling element if this is a handle
	pub position: glam::DVec2,
	// The path to the overlay for this point rendering
	// pub overlay_path: Option<Vec<LayerId>>,
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
			// kurbo_element_id: 0,
			position: DVec2::ZERO,
			// overlay_path: None,
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
		self.is_selected = selected;
	}
}
