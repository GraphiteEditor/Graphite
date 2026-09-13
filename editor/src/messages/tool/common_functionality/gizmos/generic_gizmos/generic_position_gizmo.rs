//! A generic, draggable point that edits a `DVec2` node parameter (e.g. a translation or a 2D offset).
//!
//! The slider and the dial both read a drag as one number: a distance along a ray, or a count of steps.
//! A position has no such reduction -- both components move independently -- so it is the one control that
//! could not be expressed as a variant of either, and the registry's [`GizmoType::Position`] stayed
//! unimplemented until this.
//!
//! Like the others it is driven entirely by registry data and knows nothing about the node it edits.

use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::read_dvec2_input;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoContext, GizmoInfo, GizmoState};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use std::collections::VecDeque;

/// Pixel radius within which the mouse is considered to be hovering the handle.
const POSITION_HANDLE_HOVER_THRESHOLD: f64 = 8.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenericPositionState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

/// A draggable point bound to one `DVec2` parameter of one node.
#[derive(Clone, Debug)]
pub struct GenericPositionGizmo {
	layer: LayerNodeIdentifier,
	node_id: NodeId,
	identifier: ProtoNodeIdentifier,
	info: GizmoInfo,
	state: GenericPositionState,
	/// The parameter value captured when the drag began. The drag adds a displacement to this rather than
	/// writing where the cursor sits, so grabbing the handle a pixel off centre does not shift the value.
	initial_value: DVec2,
	/// Where the cursor was on the first frame of the drag. Taken from the cursor rather than from the
	/// tool's `drag_start`, which has already been snapped to nearby geometry.
	drag_origin: Option<DVec2>,
}

impl GenericPositionGizmo {
	pub fn new(layer: LayerNodeIdentifier, node_id: NodeId, identifier: ProtoNodeIdentifier, info: GizmoInfo) -> Self {
		Self {
			layer,
			node_id,
			identifier,
			info,
			state: GenericPositionState::Inactive,
			initial_value: DVec2::ZERO,
			drag_origin: None,
		}
	}

	pub fn is_hovered(&self) -> bool {
		self.state == GenericPositionState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.state == GenericPositionState::Dragging
	}

	pub fn cleanup(&mut self) {
		self.state = GenericPositionState::Inactive;
		self.drag_origin = None;
	}

	pub fn handle_click(&mut self) {
		if self.state == GenericPositionState::Hover {
			self.state = GenericPositionState::Dragging;
		}
	}

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
				GenericPositionState::Inactive => GizmoState::Inactive,
				GenericPositionState::Hover => GizmoState::Hover,
				GenericPositionState::Dragging => GizmoState::Dragging,
			},
			mouse_position,
			shape_editor,
			handle_index: 0,
		}
	}

	fn current_value(&self, document: &DocumentMessageHandler) -> Option<DVec2> {
		read_dvec2_input(self.layer, document, &self.identifier, self.info.parameter_index)
	}

	/// Where the handle sits in viewport space.
	///
	/// A translation is expressed in the coordinate space the node's transform is built from, which is the
	/// layer's parent space, so the handle is placed by the parent transform rather than the layer's own.
	/// Using the layer transform would fold the very translation being edited into the handle's position,
	/// and the handle would then run away from the cursor as it was dragged.
	fn handle_viewport_position(&self, document: &DocumentMessageHandler, value: DVec2) -> DVec2 {
		let parent = document.metadata().downstream_transform_to_viewport(self.layer);
		parent.transform_point2(value)
	}

	/// Whether this gizmo is grabbed along a region rather than at a point. A position handle is always a
	/// point, so it outranks any band it happens to overlap.
	pub fn is_extended_target(&self) -> bool {
		self.info.behavior.extended_target
	}

	/// Pure hover test: the mouse's distance to the handle when it is a candidate, else `None`.
	/// Performs no state mutation; the manager uses it to resolve overlapping handles.
	pub fn hover_distance(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<f64> {
		let value = self.current_value(document)?;
		let distance = self.handle_viewport_position(document, value).distance(mouse_position);

		(distance <= POSITION_HANDLE_HOVER_THRESHOLD).then_some(distance)
	}

	pub fn enter_hover(&mut self, document: &DocumentMessageHandler, _mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		if self.state != GenericPositionState::Inactive {
			return;
		}
		let Some(value) = self.current_value(document) else { return };

		self.state = GenericPositionState::Hover;
		self.initial_value = value;
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Move });
	}

	pub fn exit_hover(&mut self, responses: &mut VecDeque<Message>) {
		if self.state != GenericPositionState::Hover {
			return;
		}
		self.state = GenericPositionState::Inactive;
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}

	/// Clamp each component to the declared bounds. The registry carries one `min`/`max` pair rather than
	/// one per axis, so a bounded position is bounded to a square.
	fn clamp(&self, value: DVec2) -> DVec2 {
		let min = self.info.min.unwrap_or(f64::NEG_INFINITY);
		let max = self.info.max.unwrap_or(f64::INFINITY);

		DVec2::new(value.x.clamp(min, max), value.y.clamp(min, max))
	}

	pub fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if self.state != GenericPositionState::Dragging {
			return;
		}

		// Take the reference from the cursor's own first frame. The tool's `drag_start` has been snapped to
		// nearby geometry, so using it makes a click that never moved still shift the value.
		if self.drag_origin.is_none() {
			self.drag_origin = Some(input.mouse.position);
			if let Some(value) = self.current_value(document) {
				self.initial_value = value;
			}
		}
		let origin = self.drag_origin.unwrap_or(drag_start);

		let parent = document.metadata().downstream_transform_to_viewport(self.layer);
		let Some(inverse) = parent.inverse().is_finite().then(|| parent.inverse()) else { return };

		// Displacement rather than absolute position, for the same reason the slider measures travel.
		let travelled = inverse.transform_point2(input.mouse.position) - inverse.transform_point2(origin);
		let value = self.clamp(self.initial_value + travelled);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.parameter()),
			input: NodeInput::value(TaggedValue::DVec2(value), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&ShapeState>, overlay_context: &mut OverlayContext) {
		if let Some(overlay) = self.info.behavior.overlay {
			overlay(&self.context(document, mouse_position, shape_editor), overlay_context);
		}
		if self.info.behavior.draws_own_handle {
			return;
		}

		let Some(value) = self.current_value(document) else { return };
		let position = self.handle_viewport_position(document, value);

		overlay_context.manipulator_handle(position, self.state != GenericPositionState::Inactive, None);
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		(self.state != GenericPositionState::Inactive).then_some(MouseCursorIcon::Move)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoBehavior, GizmoType, PositionHint};

	fn info(min: Option<f64>, max: Option<f64>) -> GizmoInfo {
		GizmoInfo {
			parameter_index: 1,
			gizmo_type: GizmoType::Position,
			name: "Translation",
			min,
			max,
			behavior: GizmoBehavior::NONE,
			position_hint: PositionHint::ParameterDerived,
		}
	}

	fn gizmo(min: Option<f64>, max: Option<f64>) -> GenericPositionGizmo {
		GenericPositionGizmo {
			layer: LayerNodeIdentifier::ROOT_PARENT,
			node_id: NodeId(0),
			identifier: ProtoNodeIdentifier::new("test"),
			info: info(min, max),
			state: GenericPositionState::Inactive,
			initial_value: DVec2::ZERO,
			drag_origin: None,
		}
	}

	#[test]
	fn unbounded_positions_pass_through_untouched() {
		let g = gizmo(None, None);
		assert_eq!(g.clamp(DVec2::new(-500., 900.)), DVec2::new(-500., 900.));
	}

	#[test]
	fn bounds_apply_to_both_components() {
		let g = gizmo(Some(0.), Some(10.));
		assert_eq!(g.clamp(DVec2::new(-5., 25.)), DVec2::new(0., 10.));
		assert_eq!(g.clamp(DVec2::new(4., 6.)), DVec2::new(4., 6.));
	}

	#[test]
	fn a_position_is_always_a_point_target() {
		// It must outrank any band it overlaps, the way an arc's sweep endpoints outrank its radius.
		assert!(!gizmo(None, None).is_extended_target());
	}
}
