use super::super::tool_prelude::*;
use super::drag_state::DragState;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, NodeTemplate};
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::tool::tool_messages::select_tool::LayerNodeIdentifier;
use crate::messages::tool::tool_messages::select_tool::TransformIn;
use graphene_std::uuid::NodeId;

#[derive(Clone, Debug, Default)]
pub struct DuplcateState {
	pub non_duplicated_layers: Option<Vec<LayerNodeIdentifier>>,
}

impl DuplcateState {
	pub fn reset(&mut self) {
		self.non_duplicated_layers = None;
	}

	pub fn set_duplicating(&mut self, state: bool, layers_dragging: &mut Vec<LayerNodeIdentifier>, document: &mut DocumentMessageHandler, dragging: &DragState, responses: &mut VecDeque<Message>) {
		if !state && self.non_duplicated_layers.is_some() {
			self.stop_duplicates(layers_dragging, document, dragging, responses);
		} else if state && self.non_duplicated_layers.is_none() {
			self.start_duplicates(layers_dragging, document, dragging, responses);
		}
	}

	/// Duplicates the currently dragging layers. Called when Alt is pressed and the layers have not yet been duplicated.
	fn start_duplicates(&mut self, layers_dragging: &mut Vec<LayerNodeIdentifier>, document: &mut DocumentMessageHandler, dragging: &DragState, responses: &mut VecDeque<Message>) {
		let mut new_dragging = Vec::new();

		// Get the shallowest unique layers and sort by their index relative to parent for ordered processing
		let layers = document.network_interface.shallowest_unique_layers(&[]).collect::<Vec<_>>();

		for layer in layers.into_iter().rev() {
			let Some(parent) = layer.parent(document.metadata()) else { continue };

			// Moves the layer back to its starting position.
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(dragging.inverse_drag_delta_viewport(document.metadata())),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			// Copy the layer
			let mut copy_ids = HashMap::new();
			let node_id = layer.to_node();
			copy_ids.insert(node_id, NodeId(0));

			document
				.network_interface
				.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::LayerChildrenUpstreamFlow)
				.enumerate()
				.for_each(|(index, node_id)| {
					copy_ids.insert(node_id, NodeId((index + 1) as u64));
				});

			let nodes = document.network_interface.copy_nodes(&copy_ids, &[]).collect::<Vec<(NodeId, NodeTemplate)>>();

			let insert_index = DocumentMessageHandler::get_calculated_insert_index(document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent);

			let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();

			let layer_id = *new_ids.get(&NodeId(0)).expect("Node Id 0 should be a layer");
			let layer = LayerNodeIdentifier::new_unchecked(layer_id);
			new_dragging.push(layer);
			responses.add(NodeGraphMessage::AddNodes { nodes, new_ids });
			responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index });
		}
		let nodes = new_dragging.iter().filter(|&&layer| layer != LayerNodeIdentifier::ROOT_PARENT).map(|layer| layer.to_node()).collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		responses.add(NodeGraphMessage::RunDocumentGraph);
		self.non_duplicated_layers = Some(core::mem::replace(layers_dragging, new_dragging));
	}

	/// Removes the duplicated layers. Called when Alt is released and the layers have previously been duplicated.
	fn stop_duplicates(&mut self, layers_dragging: &mut Vec<LayerNodeIdentifier>, document: &DocumentMessageHandler, dragging: &DragState, responses: &mut VecDeque<Message>) {
		let Some(original) = self.non_duplicated_layers.take() else {
			return;
		};

		// Delete the duplicated layers
		for layer in document.network_interface.shallowest_unique_layers(&[]) {
			responses.add(NodeGraphMessage::DeleteNodes {
				node_ids: vec![layer.to_node()],
				delete_children: true,
			});
		}

		for &layer in &original {
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(dragging.total_drag_delta_viewport(document.metadata())),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});
		}
		let nodes = original.iter().filter(|&&layer| layer != LayerNodeIdentifier::ROOT_PARENT).map(|layer| layer.to_node()).collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		responses.add(NodeGraphMessage::RunDocumentGraph);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::SendGraph);
		*layers_dragging = original;
	}
}
