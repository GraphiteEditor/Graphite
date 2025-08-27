use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_circular_repeat_parameters;
use crate::messages::tool::tool_messages::operation_tool::{OperationToolData, OperationToolFsmState};
use crate::messages::tool::tool_messages::tool_prelude::*;
use std::collections::VecDeque;

#[derive(Default)]
pub struct CircularRepeatOperation;

#[derive(Clone, Debug, Default)]
pub struct CircularRepeatOperationData {
	clicked_layer_radius: (LayerNodeIdentifier, f64),
	layers_dragging: Vec<(LayerNodeIdentifier, f64)>,
	initial_center: DVec2,
}

impl CircularRepeatOperation {
	pub fn create_node(tool_data: &mut OperationToolData, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, input: &InputPreprocessorMessageHandler) {
		let selected_layers = document
			.network_interface
			.selected_nodes()
			.selected_layers(document.metadata())
			.collect::<HashSet<LayerNodeIdentifier>>();

		let Some(clicked_layer) = document.click(&input) else { return };
		responses.add(DocumentMessage::StartTransaction);
		let viewport = document.metadata().transform_to_viewport(clicked_layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		// Only activate the operation if the click is close enough to the repeat center
		if center.distance(input.mouse.position) > 5. {
			return;
		};

		// If the clicked layer is part of the current selection, apply the operation to all selected layers
		if selected_layers.contains(&clicked_layer) {
			tool_data.circular_operation_data.layers_dragging = selected_layers
				.iter()
				.map(|layer| {
					let (_angle_offset, radius, _count) = extract_circular_repeat_parameters(Some(*layer), document).unwrap_or((0.0, 0.0, 6));
					if *layer == clicked_layer {
						tool_data.circular_operation_data.clicked_layer_radius = (*layer, radius)
					}
					(*layer, radius)
				})
				.collect::<Vec<(LayerNodeIdentifier, f64)>>();
		} else {
			// If the clicked layer is not in the selection, deselect all and only apply the operation to the clicked layer
			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![clicked_layer.to_node()] });

			let (_angle_offset, radius, _count) = extract_circular_repeat_parameters(Some(clicked_layer), document).unwrap_or((0.0, 0.0, 6));

			tool_data.circular_operation_data.clicked_layer_radius = (clicked_layer, radius);
			tool_data.circular_operation_data.layers_dragging = vec![(clicked_layer, radius)];
		}
		tool_data.drag_start = input.mouse.position;
		tool_data.circular_operation_data.initial_center = viewport.transform_point2(DVec2::ZERO);
	}

	pub fn update_shape(tool_data: &mut OperationToolData, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, input: &InputPreprocessorMessageHandler) {
		let (_clicked_layer, clicked_radius) = tool_data.circular_operation_data.clicked_layer_radius;

		let viewport = document.metadata().transform_to_viewport(tool_data.circular_operation_data.clicked_layer_radius.0);
		let sign = (input.mouse.position - tool_data.circular_operation_data.initial_center)
			.dot(viewport.transform_vector2(DVec2::Y))
			.signum();

		// Compute mouse movement in local space, ignoring the layer’s own transform
		let delta = document
			.metadata()
			.downstream_transform_to_viewport(tool_data.circular_operation_data.clicked_layer_radius.0)
			.inverse()
			.transform_vector2(input.mouse.position - tool_data.circular_operation_data.initial_center)
			.length() * sign;

		for (layer, initial_radius) in &tool_data.circular_operation_data.layers_dragging {
			// If the layer’s sign differs from the clicked layer, invert delta to preserve consistent in/out dragging behavior
			let new_radius = if initial_radius.signum() == clicked_radius.signum() {
				*initial_radius + delta
			} else {
				*initial_radius + delta.signum() * -1. * delta.abs()
			};

			responses.add(GraphOperationMessage::CircularRepeatSet {
				layer: *layer,
				angle: 0.,
				radius: new_radius,
				count: 6,
			});
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(
		tool_state: &OperationToolFsmState,
		tool_data: &mut OperationToolData,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		overlay_context: &mut OverlayContext,
	) {
		match tool_state {
			OperationToolFsmState::Ready => {
				// Draw overlays for all selected layers
				for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
					Self::draw_layer_overlay(layer, document, input, overlay_context)
				}

				// Also highlight the hovered layer if it’s not selected
				if let Some(layer) = document.click(&input) {
					Self::draw_layer_overlay(layer, document, input, overlay_context);
				}
			}
			_ => {
				// While dragging, only draw overlays for the layers being modified
				for layer in tool_data.circular_operation_data.layers_dragging.iter().map(|(l, _)| l) {
					let Some(vector) = document.network_interface.compute_modified_vector(*layer) else { continue };
					let viewport = document.metadata().transform_to_viewport(*layer);

					overlay_context.outline_vector(&vector, viewport);
				}
			}
		}
	}

	fn draw_layer_overlay(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, overlay_context: &mut OverlayContext) {
		if let Some(vector) = document.network_interface.compute_modified_vector(layer) {
			let viewport = document.metadata().transform_to_viewport(layer);
			let center = viewport.transform_point2(DVec2::ZERO);
			// Show a small circle if the mouse is near the repeat center
			if center.distance(input.mouse.position) < 5. {
				overlay_context.circle(center, 3., None, None);
			}
			overlay_context.outline_vector(&vector, viewport);
		}
	}

	pub fn cleanup(tool_data: &mut OperationToolData) {
		// Clear stored drag state at the end of the operation
		tool_data.circular_operation_data.layers_dragging.clear();
	}
}
