use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::spiral_turns_handle::{SpiralTurns, SpiralTurnsState};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeGizmoHandler, extract_spiral_parameters};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::tool_messages::shape_tool::ShapeOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeInputDecleration;
use graphene_std::subpath::{calculate_b, spiral_point};
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Spiral;

#[derive(Clone, Debug, Default)]
pub struct SpiralGizmoHandler {
	turns_handle: SpiralTurns,
}

impl ShapeGizmoHandler for SpiralGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.turns_handle.hovered()
	}

	fn handle_state(&mut self, selected_spiral_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.turns_handle.handle_actions(selected_spiral_layer, mouse_position, document, responses);
	}

	fn handle_click(&mut self) {
		if self.turns_handle.hovered() {
			self.turns_handle.update_state(SpiralTurnsState::Dragging);
			return;
		}
	}

	fn handle_update(&mut self, _drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if self.turns_handle.is_dragging() {
			self.turns_handle.update_number_of_turns(document, input, responses);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_spiral_layer: Option<LayerNodeIdentifier>,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.turns_handle.hovered() {
			self.turns_handle.overlays(document, selected_spiral_layer, shape_editor, mouse_position, overlay_context);
			return;
		}

		self.turns_handle.overlays(document, selected_spiral_layer, shape_editor, mouse_position, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.turns_handle.is_dragging() {
			self.turns_handle.overlays(document, None, shape_editor, mouse_position, overlay_context);
		}
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		if self.turns_handle.hovered() || self.turns_handle.is_dragging() {
			return Some(MouseCursorIcon::Default);
		}
		None
	}

	fn cleanup(&mut self) {
		self.turns_handle.cleanup();
	}
}

impl Spiral {
	pub fn create_node(spiral_type: SpiralType, turns: f64) -> NodeTemplate {
		let inner_radius = match spiral_type {
			SpiralType::Archimedean => 0.,
			SpiralType::Logarithmic => 0.1,
		};

		let node_type = resolve_document_node_type("Spiral").expect("Spiral node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::SpiralType(spiral_type), false)),
			Some(NodeInput::value(TaggedValue::F64(turns), false)),
			Some(NodeInput::value(TaggedValue::F64(0.), false)),
			Some(NodeInput::value(TaggedValue::F64(inner_radius), false)),
			Some(NodeInput::value(TaggedValue::F64(0.1), false)),
			Some(NodeInput::value(TaggedValue::F64(90.), false)),
		])
	}

	pub fn update_shape(document: &DocumentMessageHandler, ipp: &InputPreprocessorMessageHandler, layer: LayerNodeIdentifier, shape_tool_data: &mut ShapeToolData, responses: &mut VecDeque<Message>) {
		use graphene_std::vector::generator_nodes::spiral::*;

		let viewport_drag_start = shape_tool_data.data.viewport_drag_start(document);

		let ignore = vec![layer];
		let snap_data = SnapData::ignore(document, ipp, &ignore);
		let config = SnapTypeConfiguration::default();
		let document_mouse = document.metadata().document_to_viewport.inverse().transform_point2(ipp.mouse.position);
		let snapped = shape_tool_data.data.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), config);
		let snapped_viewport_point = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);
		shape_tool_data.data.snap_manager.update_indicator(snapped);

		let dragged_distance = (viewport_drag_start - snapped_viewport_point).length();

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Spiral") else {
			return;
		};

		let Some(&TaggedValue::SpiralType(spiral_type)) = node_inputs.get(SpiralTypeInput::INDEX).unwrap().as_value() else {
			return;
		};

		let new_radius = match spiral_type {
			SpiralType::Archimedean => dragged_distance,
			SpiralType::Logarithmic => (dragged_distance).max(0.1),
		};

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., viewport_drag_start),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, OuterRadiusInput::INDEX),
			input: NodeInput::value(TaggedValue::F64(new_radius), false),
		});
	}

	/// Updates the number of turns of a Spiral node and recalculates its radius based on drag distance.
	/// Also updates the Shape tool's turns UI widget to reflect the change.
	pub fn update_turns(decrease: bool, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		use graphene_std::vector::generator_nodes::spiral::*;

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Spiral") else {
			return;
		};

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let Some(&TaggedValue::F64(mut turns)) = node_inputs.get(TurnsInput::INDEX).unwrap().as_value() else {
			return;
		};

		if decrease {
			turns = (turns - 1.).max(1.);
		} else {
			turns += 1.;
		}

		responses.add(ShapeToolMessage::UpdateOptions {
			options: ShapeOptionsUpdate::Turns(turns),
		});

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, TurnsInput::INDEX),
			input: NodeInput::value(TaggedValue::F64(turns), false),
		});
	}
}

pub fn calculate_spiral_endpoints(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, viewport: DAffine2, theta: f64) -> Option<DVec2> {
	let Some((spiral_type, start_angle, a, outer_radius, turns, _)) = extract_spiral_parameters(layer, document) else {
		return None;
	};

	let theta = turns * theta + start_angle.to_radians();
	let b = calculate_b(a, turns, outer_radius, spiral_type);

	Some(viewport.transform_point2(spiral_point(theta, a, b, spiral_type)))
}
