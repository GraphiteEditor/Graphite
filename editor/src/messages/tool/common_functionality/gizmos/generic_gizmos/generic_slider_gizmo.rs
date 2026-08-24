//! A generic, draggable handle that edits a continuous `f64` node parameter (e.g. a radius).
//!
//! Unlike the hand-written shape gizmos it replaced, this gizmo is fully driven by data
//! from the [gizmo registry](crate::messages::tool::common_functionality::gizmos::gizmo_registry):
//! it knows nothing about the specific node it edits beyond the node id, the parameter index, and
//! the registry's [`GizmoInfo`]. This is what lets any node opt into a slider with zero custom code.

use crate::consts::{GIZMO_HIDE_THRESHOLD, POINT_RADIUS_HANDLE_SNAP_THRESHOLD};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::GraphOperationMessage;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::read_number_input;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{DragInput, GizmoContext, GizmoInfo, GizmoState, PositionHint};
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use glam::{DAffine2, DVec2};
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use std::collections::VecDeque;

/// Pixel radius within which the mouse is considered to be hovering the handle.
const SLIDER_HANDLE_HOVER_THRESHOLD: f64 = 8.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenericSliderState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

/// A draggable slider handle bound to one `f64` parameter of one node.
#[derive(Clone, Debug)]
pub struct GenericSliderGizmo {
	layer: LayerNodeIdentifier,
	node_id: NodeId,
	identifier: ProtoNodeIdentifier,
	info: GizmoInfo,
	state: GenericSliderState,
	/// The parameter value captured when the drag began, used as the clamping/anchor reference.
	initial_value: f64,
	/// Values this drag should snap to, resolved once when the gizmo is first hovered. They depend on the
	/// layer's *other* parameters, so they are captured alongside `initial_value` rather than recomputed
	/// per frame while the value being dragged is already in flux.
	snap_targets: Vec<f64>,
	/// Which grab point the user took hold of, indexing `handle_positions`. Zero for the common case of a
	/// parameter with a single handle.
	handle_index: usize,
	/// The node's inputs as they stood when the drag began. A drag that writes several parameters needs
	/// them, since the live values are the ones it already wrote.
	initial_parameters: Vec<Option<TaggedValue>>,
	/// Cursor position last frame, for accumulating swept angle.
	previous_mouse_position: DVec2,
	/// Angle swept around the layer origin since the drag began, in degrees.
	total_angle: f64,
	/// This frame's rotation about the layer origin, in degrees.
	angle_delta: f64,
	/// Where the gesture is currently measured from. Normally where the drag began, but a shape that
	/// re-anchors mid-drag moves it.
	drag_origin: Option<DVec2>,
}

impl GenericSliderGizmo {
	pub fn new(layer: LayerNodeIdentifier, node_id: NodeId, identifier: ProtoNodeIdentifier, info: GizmoInfo) -> Self {
		Self {
			layer,
			node_id,
			identifier,
			info,
			state: GenericSliderState::Inactive,
			initial_value: 0.,
			snap_targets: Vec::new(),
			handle_index: 0,
			initial_parameters: Vec::new(),
			previous_mouse_position: DVec2::ZERO,
			total_angle: 0.,
			angle_delta: 0.,
			drag_origin: None,
		}
	}

	pub fn is_hovered(&self) -> bool {
		self.state == GenericSliderState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.state == GenericSliderState::Dragging
	}

	pub fn cleanup(&mut self) {
		self.state = GenericSliderState::Inactive;
		self.snap_targets.clear();
		self.initial_parameters.clear();
		self.total_angle = 0.;
		self.drag_origin = None;
	}

	/// Begin a drag if currently hovered.
	pub fn handle_click(&mut self) {
		if self.state == GenericSliderState::Hover {
			self.state = GenericSliderState::Dragging;
		}
	}

	/// The registry entry's parameter, re-paired with the node it was declared for. `ParameterRef` is the
	/// runtime form of a parameter symbol: the generic gizmos choose their parameter from the registry at
	/// runtime, so they cannot name a symbol at the call site, but the identifier and index still travel together.
	fn parameter(&self) -> ParameterRef {
		ParameterRef {
			node_identifier: self.identifier.clone(),
			input_index: self.info.parameter_index,
		}
	}

	/// Snapshot every input of this gizmo's node, indexed the way its parameter symbols are.
	fn read_all_parameters(&self, document: &DocumentMessageHandler) -> Vec<Option<TaggedValue>> {
		NodeGraphLayer::new(self.layer, &document.network_interface)
			.find_node_inputs(&DefinitionIdentifier::ProtoNode(self.identifier.clone()))
			.map(|inputs| inputs.iter().map(|input| input.as_value().cloned()).collect())
			.unwrap_or_default()
	}

	fn context<'a>(&self, document: &'a DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&'a ShapeState>) -> GizmoContext<'a> {
		GizmoContext {
			layer: self.layer,
			document,
			parameter: self.parameter(),
			state: match self.state {
				GenericSliderState::Inactive => GizmoState::Inactive,
				GenericSliderState::Hover => GizmoState::Hover,
				GenericSliderState::Dragging => GizmoState::Dragging,
			},
			mouse_position,
			shape_editor,
			handle_index: self.handle_index,
		}
	}

	fn current_value(&self, document: &DocumentMessageHandler) -> Option<f64> {
		read_number_input(self.layer, document, &self.identifier, self.info.parameter_index)
	}

	/// Every point in the layer's local space where this parameter can be grabbed.
	///
	/// Shapes that place their handles on their own geometry supply them; everything else gets the single
	/// default handle, sitting `value` out along the local +X axis.
	fn handle_positions(&self, document: &DocumentMessageHandler, value: f64) -> Vec<DVec2> {
		match self.info.behavior.handle_positions {
			Some(positions) => positions(&self.context(document, DVec2::ZERO, None), value),
			None => vec![match self.info.position_hint {
				// A length-like parameter: place the handle that far out along the local +X axis.
				PositionHint::ParameterDerived => DVec2::new(value.abs(), 0.),
				// Generic fall-backs map the value onto the local +X axis as well; bounding-box-aware
				// hints are refined as more node types adopt the slider.
				_ => DVec2::new(value.abs(), 0.),
			}],
		}
	}

	/// The grab point currently in play, in local space.
	fn active_handle_local(&self, document: &DocumentMessageHandler, value: f64) -> Option<DVec2> {
		let handles = self.handle_positions(document, value);
		handles.get(self.handle_index).copied().or_else(|| handles.first().copied())
	}

	/// Pure hover test: returns the mouse's distance to the handle when it is a hover candidate, or
	/// `None` otherwise. The manager uses this distance to resolve priority when several gizmos
	/// overlap (the closest handle wins). This performs no state mutation.
	pub fn hover_distance(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<f64> {
		let value = self.current_value(document)?;

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		self.hover_distances(document, value, mouse_position, viewport, center)
			.into_iter()
			.flatten()
			.min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
	}

	/// Distance from the cursor to each grab point, or `None` for grab points that are unavailable or too
	/// small on screen to aim at.
	fn hover_distances(&self, document: &DocumentMessageHandler, value: f64, mouse_position: DVec2, viewport: DAffine2, center: DVec2) -> Vec<Option<f64>> {
		if let Some(distances) = self.info.behavior.hover_distances {
			return distances(&self.context(document, mouse_position, None));
		}

		self.handle_positions(document, value)
			.into_iter()
			.map(|local| viewport.transform_point2(local))
			.map(|handle| {
				// Hide the gizmo when the shape is too small on screen to interact with reliably.
				let reachable = handle.distance(center) >= GIZMO_HIDE_THRESHOLD;
				let distance = mouse_position.distance(handle);

				(reachable && distance <= SLIDER_HANDLE_HOVER_THRESHOLD).then_some(distance)
			})
			.collect()
	}

	/// Index of the grab point nearest the cursor, so a drag knows which ray it runs along.
	fn nearest_handle_index(&self, document: &DocumentMessageHandler, value: f64, mouse_position: DVec2) -> usize {
		let viewport = document.metadata().transform_to_viewport(self.layer);

		let center = viewport.transform_point2(DVec2::ZERO);

		self.hover_distances(document, value, mouse_position, viewport, center)
			.into_iter()
			.enumerate()
			.filter_map(|(index, distance)| distance.map(|distance| (index, distance)))
			.min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
			.map(|(index, _)| index)
			.unwrap_or(0)
	}

	/// Transition into the hovered state (no-op if already hovered or dragging). Capturing the
	/// reference value here is necessary because `handle_click` (which starts the drag) has no
	/// access to the document.
	pub fn enter_hover(&mut self, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		if self.state != GenericSliderState::Inactive {
			return;
		}
		let Some(value) = self.current_value(document) else { return };

		self.state = GenericSliderState::Hover;
		self.initial_value = value;
		self.handle_index = self.nearest_handle_index(document, value, mouse_position);
		self.initial_parameters = self.read_all_parameters(document);
		self.previous_mouse_position = mouse_position;
		self.total_angle = 0.;
		self.snap_targets = match self.info.behavior.snap_targets {
			Some(targets) => targets(&self.context(document, mouse_position, None)),
			None => Vec::new(),
		};
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
	}

	/// Transition out of the hovered state. Leaves an in-progress drag untouched.
	pub fn exit_hover(&mut self, responses: &mut VecDeque<Message>) {
		if self.state == GenericSliderState::Hover {
			self.state = GenericSliderState::Inactive;
			self.snap_targets.clear();
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}

	/// Update the parameter live while dragging. The new value is the mouse's position projected
	/// onto the local +X axis, clamped to the registry's min/max bounds.
	pub fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// The first frame of a drag fixes the reference every later frame is measured against. Take it from
		// the cursor itself rather than the tool's drag start: the two are in the same space, but the tool's
		// may have been snapped to nearby geometry, and that offset would otherwise be spent as movement the
		// instant the handle is grabbed -- a visible nudge on a click that never moved.
		if self.drag_origin.is_none() {
			self.drag_origin = Some(input.mouse.position);
			self.previous_mouse_position = input.mouse.position;
			if let Some(value) = self.current_value(document) {
				self.initial_value = value;
			}
		}
		let drag_start = self.drag_origin.unwrap_or(drag_start);

		self.accumulate_angle(document, input.mouse.position);

		if let Some(drag) = self.info.behavior.drag {
			let mut drag_input = DragInput {
				drag_start,
				mouse_position: input.mouse.position,
				initial_value: self.initial_value,
				initial_parameters: self.initial_parameters.clone(),
				total_angle: self.total_angle,
				angle_delta: self.angle_delta,
				handle_index: self.handle_index,
			};

			let writes = drag(&self.context(document, input.mouse.position, None), &mut drag_input);

			// The shape may have re-anchored the gesture; carry its baseline forward.
			self.total_angle = drag_input.total_angle;
			self.initial_parameters = drag_input.initial_parameters;
			self.handle_index = drag_input.handle_index;
			self.initial_value = drag_input.initial_value;
			self.drag_origin = Some(drag_input.drag_start);

			if writes.is_empty() {
				return;
			}
			for (parameter, value) in writes.inputs {
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(self.node_id, parameter),
					input: NodeInput::value(value, false),
				});
			}
			if let Some(transform) = writes.transform {
				responses.add(GraphOperationMessage::TransformChange {
					layer: self.layer,
					transform,
					transform_in: TransformIn::Viewport,
					skip_rerender: false,
				});
			}
			responses.add(NodeGraphMessage::RunDocumentGraph);
			return;
		}

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let local_mouse = viewport.inverse().transform_point2(input.mouse.position);

		// Project the cursor onto the ray through the grabbed handle. For the default single handle that ray
		// is the +X axis; for a handle sitting on the shape's own geometry it is the ray the user is visibly
		// pulling along.
		let Some(anchor) = self.active_handle_local(document, self.initial_value) else { return };
		let ray = anchor.try_normalize().unwrap_or(DVec2::X);

		// Measure how far the cursor has travelled along that ray rather than where it now sits, so the value
		// does not jump the instant the handle is grabbed a pixel off centre. This is what the hand-written
		// handlers did, all of which added a delta to the value they started from.
		let travelled = local_mouse.dot(ray) - viewport.inverse().transform_point2(drag_start).dot(ray);

		// Preserve the sign of the original value for parameters (like radius) that can be negative.
		let direction = if self.initial_value.is_sign_negative() { -1. } else { 1. };
		let mut value = self.initial_value + travelled * direction;

		value = self.snap(self.clamp(value));

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.parameter()),
			input: NodeInput::value(TaggedValue::F64(value), false),
		});

		// Parameters that are only meaningful in combination are written in the same batch, so the graph
		// never evaluates a half-updated shape.
		if let Some(coupled_writes) = self.info.behavior.coupled_writes {
			for (parameter, coupled_value) in coupled_writes(&self.context(document, input.mouse.position, None), value) {
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(self.node_id, parameter),
					input: NodeInput::value(coupled_value, false),
				});
			}
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Add this frame's rotation about the layer's origin to the running total, so a drag that winds several
	/// times around keeps counting instead of wrapping at half a turn.
	///
	/// Rotations inside the behavior's deadzone are dropped; see `GizmoBehavior::angle_deadzone`.
	fn accumulate_angle(&mut self, document: &DocumentMessageHandler, mouse_position: DVec2) {
		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);
		let inverse = viewport.inverse();

		let delta = inverse
			.transform_vector2(self.previous_mouse_position - center)
			.angle_to(inverse.transform_vector2(mouse_position - center))
			.to_degrees();

		self.previous_mouse_position = mouse_position;
		self.angle_delta = if delta.is_finite() && delta.abs() >= self.info.behavior.angle_deadzone { delta } else { 0. };
		self.total_angle += self.angle_delta;
	}

	/// Pull the value onto the nearest snap target within the threshold. Returns the value unchanged when
	/// nothing is in range.
	fn snap(&self, value: f64) -> f64 {
		nearest_snap_target(value, &self.snap_targets, POINT_RADIUS_HANDLE_SNAP_THRESHOLD).unwrap_or(value)
	}

	fn clamp(&self, value: f64) -> f64 {
		let mut value = value;
		if let Some(min) = self.info.min {
			value = value.max(min);
		}
		if let Some(max) = self.info.max {
			value = value.min(max);
		}
		value
	}

	/// Draw the handle dot, plus a guide line from the layer origin while hovered or dragging.
	pub fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&ShapeState>, overlay_context: &mut OverlayContext) {
		// The shape's own overlay runs in every state: a resting affordance is exactly the case it wants to
		// draw for, and the generic handle below only appears once the gizmo is engaged.
		if let Some(overlay) = self.info.behavior.overlay {
			overlay(&self.context(document, mouse_position, shape_editor), overlay_context);
		}

		if self.state == GenericSliderState::Inactive {
			// A shape that draws its own overlay has already put something on screen to aim at. One that
			// does not would otherwise be invisible until the cursor happened to land on it, so the generic
			// layer marks where it can be grabbed.
			if self.info.behavior.overlay.is_none() && !self.info.behavior.draws_own_handle {
				self.draw_resting_handles(document, overlay_context);
			}
			return;
		}

		// A shape that draws the thing being grabbed does not want a second handle on top of it.
		if self.info.behavior.draws_own_handle {
			return;
		}

		let Some(value) = self.current_value(document) else { return };
		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);
		let Some(local) = self.active_handle_local(document, value) else { return };
		let handle = viewport.transform_point2(local);

		if handle.distance(center) < GIZMO_HIDE_THRESHOLD {
			return;
		}

		overlay_context.line(center, handle, None, None);
		overlay_context.manipulator_handle(handle, self.state == GenericSliderState::Dragging, None);
	}

	/// Mark every grab point with an unengaged handle, so a control with no overlay of its own is still
	/// discoverable before the cursor finds it.
	fn draw_resting_handles(&self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		let Some(value) = self.current_value(document) else { return };
		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		for local in self.handle_positions(document, value) {
			let handle = viewport.transform_point2(local);
			// Too small on screen to aim at, and the handle would sit on top of the shape's own centre.
			if handle.distance(center) < GIZMO_HIDE_THRESHOLD {
				continue;
			}
			overlay_context.manipulator_handle(handle, false, None);
		}
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self.state {
			GenericSliderState::Hover | GenericSliderState::Dragging => Some(MouseCursorIcon::EWResize),
			GenericSliderState::Inactive => None,
		}
	}
}

/// The snap target closest to `value`, if any lies within `threshold`. Ties go to the earlier target, so a
/// shape can express priority through the order it returns them in.
fn nearest_snap_target(value: f64, targets: &[f64], threshold: f64) -> Option<f64> {
	targets
		.iter()
		.copied()
		.map(|target| (target, (target - value).abs()))
		.filter(|(_, distance)| *distance < threshold)
		.min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
		.map(|(target, _)| target)
}

#[cfg(test)]
mod tests {
	use super::nearest_snap_target;

	#[test]
	fn no_targets_never_snaps() {
		assert_eq!(nearest_snap_target(42., &[], 8.), None);
	}

	#[test]
	fn snaps_to_a_target_inside_the_threshold() {
		assert_eq!(nearest_snap_target(48., &[50., 100.], 8.), Some(50.));
	}

	#[test]
	fn ignores_targets_outside_the_threshold() {
		assert_eq!(nearest_snap_target(40., &[50., 100.], 8.), None);
	}

	#[test]
	fn picks_the_nearest_of_several_candidates() {
		assert_eq!(nearest_snap_target(52., &[50., 55., 100.], 8.), Some(50.));
		assert_eq!(nearest_snap_target(54., &[50., 55., 100.], 8.), Some(55.));
	}

	#[test]
	fn ties_go_to_the_earlier_target() {
		// A shape returns its most important snap targets first, so an exact tie must not reorder them.
		assert_eq!(nearest_snap_target(50., &[45., 55.], 8.), Some(45.));
	}

	#[test]
	fn handles_negative_values() {
		// Radii can be negative, and a negative radius snaps against negative targets.
		assert_eq!(nearest_snap_target(-48., &[-50., 50.], 8.), Some(-50.));
	}
}
