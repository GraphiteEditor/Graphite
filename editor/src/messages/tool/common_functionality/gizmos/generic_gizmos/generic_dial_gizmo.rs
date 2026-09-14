//! A dial that edits a `u32` node parameter, such as a polygon's side count.
//!
//! It sits at the layer's origin and turns a horizontal drag into integer steps: right to increase, left to
//! decrease.

use crate::consts::{GIZMO_HIDE_THRESHOLD, NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::read_u32_input;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoContext, GizmoInfo, GizmoState};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use std::collections::VecDeque;

/// Horizontal drag distance (viewport px) that corresponds to one integer step.
const DIAL_PIXELS_PER_STEP: f64 = 25.;
/// Viewport radius of the drawn dial indicator.
const DIAL_INDICATOR_RADIUS: f64 = NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH;
/// Viewport radius of the clickable hit area. Larger than the drawn indicator so the handle is easy to grab
/// and the press does not fall through to the layer-move behavior.
const DIAL_HOVER_RADIUS: f64 = NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH + 8.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenericDialState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

/// A dial bound to one `u32` parameter of one node.
#[derive(Clone, Debug)]
pub struct GenericDialGizmo {
	layer: LayerNodeIdentifier,
	node_id: NodeId,
	identifier: ProtoNodeIdentifier,
	info: GizmoInfo,
	state: GenericDialState,
	/// Parameter value captured when the drag began.
	initial_value: u32,
}

impl GenericDialGizmo {
	pub fn new(layer: LayerNodeIdentifier, node_id: NodeId, identifier: ProtoNodeIdentifier, info: GizmoInfo) -> Self {
		Self {
			layer,
			node_id,
			identifier,
			info,
			state: GenericDialState::Inactive,
			initial_value: 0,
		}
	}

	pub fn is_hovered(&self) -> bool {
		self.state == GenericDialState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.state == GenericDialState::Dragging
	}

	pub fn cleanup(&mut self) {
		self.state = GenericDialState::Inactive;
	}

	pub fn handle_click(&mut self) {
		if self.state == GenericDialState::Hover {
			self.state = GenericDialState::Dragging;
		}
	}

	/// The registry entry's parameter, re-paired with the node it was declared for. A gizmo picks its
	/// parameter from the registry at runtime, so it cannot name a parameter symbol at the call site.
	fn parameter(&self) -> ParameterRef {
		ParameterRef {
			node_identifier: self.identifier.clone(),
			input_index: self.info.parameter_index,
		}
	}

	fn context<'a>(&self, document: &'a DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&'a ShapeState>) -> GizmoContext<'a> {
		GizmoContext {
			layer: self.layer,
			document,
			parameter: self.parameter(),
			state: match self.state {
				GenericDialState::Inactive => GizmoState::Inactive,
				GenericDialState::Hover => GizmoState::Hover,
				GenericDialState::Dragging => GizmoState::Dragging,
			},
			mouse_position,
			shape_editor,
			handle_index: 0,
		}
	}

	fn current_value(&self, document: &DocumentMessageHandler) -> Option<u32> {
		read_u32_input(self.layer, document, &self.identifier, self.info.parameter_index)
	}

	/// Whether this gizmo is grabbed along a region rather than at a point, which decides priority against an
	/// overlapping handle. See `GizmoBehavior::extended_target`.
	pub fn is_extended_target(&self) -> bool {
		self.info.behavior.extended_target
	}

	/// The cursor's distance to the dial's centre when it is a hover candidate, else `None`. The dial occupies
	/// a disc of `DIAL_HOVER_RADIUS` around the layer origin. Mutates nothing.
	pub fn hover_distance(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<f64> {
		self.current_value(document)?;

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		// Once the shape is this small the hit disc covers the whole of it, and a press meant for the layer
		// would be swallowed by the dial.
		let bounds = document.metadata().bounding_box_viewport(self.layer)?;
		if (bounds[1] - bounds[0]).max_element() / 2. < GIZMO_HIDE_THRESHOLD {
			return None;
		}

		let distance = mouse_position.distance(center);
		(distance <= DIAL_HOVER_RADIUS).then_some(distance)
	}

	/// Enter the hovered state, unless already hovered or dragging. The reference value is captured here
	/// because `handle_click`, which starts the drag, has no access to the document.
	pub fn enter_hover(&mut self, document: &DocumentMessageHandler, _mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		if self.state != GenericDialState::Inactive {
			return;
		}
		let Some(value) = self.current_value(document) else { return };

		self.state = GenericDialState::Hover;
		self.initial_value = value;
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
	}

	/// Transition out of the hovered state. Leaves an in-progress drag untouched.
	pub fn exit_hover(&mut self, responses: &mut VecDeque<Message>) {
		if self.state == GenericDialState::Hover {
			self.state = GenericDialState::Inactive;
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}

	/// Convert the drag into integer steps, clamped to the registry's bounds. The magnitude comes from the
	/// total drag distance, so the dial answers motion in any direction, while the horizontal component
	/// decides the sign: right increases, left decreases.
	pub fn handle_update(&self, drag_start: DVec2, _document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let drag = input.mouse.position - drag_start;
		let direction = (input.mouse.position.x - drag_start.x).signum();
		let steps = ((drag.length() / DIAL_PIXELS_PER_STEP).round() * direction) as i64;

		let min = self.info.min.map(|min| min as i64).unwrap_or(0);
		// u32::MAX, not i64::MAX: the cast below would wrap anything above it.
		let max = self.info.max.map(|max| max as i64).unwrap_or(u32::MAX as i64);
		let new_value = (self.initial_value as i64 + steps).clamp(min, max) as u32;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.parameter()),
			input: NodeInput::value(TaggedValue::U32(new_value), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Draw the dial at the layer origin: an outer ring plus a filled centre dot, so it reads as draggable.
	pub fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&ShapeState>, overlay_context: &mut OverlayContext) {
		if let Some(overlay) = self.info.behavior.overlay {
			overlay(&self.context(document, mouse_position, shape_editor), overlay_context);
		}

		if self.state == GenericDialState::Inactive {
			return;
		}

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		overlay_context.circle(center, DIAL_INDICATOR_RADIUS, None, None);
		overlay_context.manipulator_handle(center, self.state == GenericDialState::Dragging, None);
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self.state {
			GenericDialState::Hover | GenericDialState::Dragging => Some(MouseCursorIcon::EWResize),
			GenericDialState::Inactive => None,
		}
	}
}
