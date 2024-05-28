use super::transform_utils::{self, LayerBounds};
use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, NodeId, NodeInput, NodeNetwork};
use graphene_core::renderer::Quad;
use graphene_core::text::Font;
use graphene_core::vector::style::{Fill, Gradient, GradientType, LineCap, LineJoin, Stroke};
use graphene_core::Color;
use graphene_std::vector::convert_usvg_path;

use glam::{DAffine2, DVec2, IVec2};

pub struct GraphOperationMessageData<'a> {
	pub document_network: &'a mut NodeNetwork,
	pub document_metadata: &'a mut DocumentMetadata,
	pub selected_nodes: &'a mut SelectedNodes,
	pub collapsed: &'a mut CollapsedLayers,
	pub node_graph: &'a mut NodeGraphMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct GraphOperationMessageHandler {}

// GraphOperationMessageHandler always modified the document network. This is so changes to the layers panel will only affect the document network.
// For changes to the selected network, use NodeGraphMessageHandler. No NodeGraphMessage's should be added here, since they will affect the selected nested network.
impl MessageHandler<GraphOperationMessage, GraphOperationMessageData<'_>> for GraphOperationMessageHandler {
	fn process_message(&mut self, message: GraphOperationMessage, responses: &mut VecDeque<Message>, data: GraphOperationMessageData) {
		let GraphOperationMessageData {
			document_network,
			document_metadata,
			selected_nodes,
			collapsed,
			node_graph,
		} = data;

		match message {
			GraphOperationMessage::AddNodesAsChild { nodes, new_ids, parent, insert_index } => {
				let shift = document_network
					.root_node
					.and_then(|root_node| {
						nodes.get(&root_node.id).and_then(|node| {
							if parent == LayerNodeIdentifier::ROOT_PARENT {
								return None;
							};
							let parent_node_id = parent.to_node();
							document_network
								.nodes
								.get(&parent_node_id)
								.map(|layer| layer.metadata.position - node.metadata.position + IVec2::new(-8, 0))
						})
					})
					.unwrap_or_default();

				for (old_id, mut document_node) in nodes {
					// Shift copied node
					document_node.metadata.position += shift;

					// Get the new, non-conflicting id
					let node_id = *new_ids.get(&old_id).unwrap();
					document_node = document_node.map_ids(NodeGraphMessageHandler::default_node_input, &new_ids);

					// Insert node into network
					document_network.nodes.insert(node_id, document_node);
				}

				let Some(new_layer_id) = new_ids.get(&NodeId(0)) else {
					error!("Could not get layer node when adding as child");
					return;
				};

				let insert_index = if insert_index < 0 { 0 } else { insert_index as usize };
				let (downstream_node, upstream_node, input_index) = ModifyInputsContext::get_post_node_with_index(document_network, parent, insert_index);

				responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![*new_layer_id] });

				if let (Some(downstream_node), Some(upstream_node)) = (downstream_node, upstream_node) {
					responses.add(GraphOperationMessage::InsertNodeBetween {
						post_node_id: downstream_node,
						post_node_input_index: input_index,
						insert_node_output_index: 0,
						insert_node_id: *new_layer_id,
						insert_node_input_index: 0,
						pre_node_output_index: 0,
						pre_node_id: upstream_node,
					})
				} else if let Some(downstream_node) = downstream_node {
					responses.add(GraphOperationMessage::SetNodeInput {
						node_id: downstream_node,
						input_index: input_index,
						input: NodeInput::node(*new_layer_id, 0),
					})
				} else {
					document_network.root_node = Some(graph_craft::document::RootNode { id: *new_layer_id, output_index: 0 });
					if let Some(primary_export) = document_network.exports.get_mut(0) {
						*primary_export = NodeInput::node(*new_layer_id, 0)
					}
				}

				responses.add(GraphOperationMessage::ShiftUpstream {
					node_id: *new_layer_id,
					shift: IVec2::new(0, 3),
					shift_self: true,
				});

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::CreateBooleanOperationNode { node_id, operation } => {
				let new_boolean_operation_node = resolve_document_node_type("Boolean Operation")
					.expect("Failed to create a Boolean Operation node")
					.to_document_node_default_inputs(
						[
							Some(NodeInput::value(TaggedValue::VectorData(graphene_std::vector::VectorData::empty()), true)),
							Some(NodeInput::value(TaggedValue::VectorData(graphene_std::vector::VectorData::empty()), true)),
							Some(NodeInput::value(TaggedValue::BooleanOperation(operation), false)),
						],
						Default::default(),
					);
				document_network.nodes.insert(node_id, new_boolean_operation_node);
			}
			GraphOperationMessage::DeleteLayer { layer, reconnect } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot delete ROOT_PARENT");
					return;
				}
				ModifyInputsContext::delete_nodes(document_network, selected_nodes, vec![layer.to_node()], reconnect, responses, Vec::new(), &node_graph.resolved_types);

				load_network_structure(document_network, document_metadata, collapsed);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			// Make sure to also update NodeGraphMessage::DisconnectInput when changing this
			GraphOperationMessage::DisconnectInput { node_id, input_index } => {
				let Some(existing_input) = document_network.nodes.get(&node_id).map_or_else(
					|| {
						if input_index == 0 {
							responses.add(NodeGraphMessage::SetRootNode { root_node: None })
						};
						document_network.exports.get(input_index)
					},
					|node| node.inputs.get(input_index),
				) else {
					warn!("Could not find input for {node_id} at index {input_index} when disconnecting");
					return;
				};

				let tagged_value = ModifyInputsContext::get_input_tagged_value(document_network, &Vec::new(), node_id, &node_graph.resolved_types, input_index);

				let mut input = NodeInput::value(tagged_value, true);
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = existing_input.is_exposed();
				}
				responses.add(GraphOperationMessage::SetNodeInput { node_id, input_index, input });
				if document_network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			GraphOperationMessage::DisconnectNodeFromStack { node_id, reconnect_to_sibling } => {
				//TODO: downstream node can be none if it is the root node
				//TODO: .collect_outwards_links() is very inefficient. Replace node_id with a layer, and disconnect most upstream non layer node, before the next layer node
				let outwards_links = document_network.collect_outwards_links();

				let Some(downstream_node_id) = outwards_links.get(&node_id).and_then(|outward_links| outward_links.get(0)) else {
					log::error!("Downstream node should always exist when moving layer");
					return;
				};
				let Some(downstream_node) = document_network.nodes.get(downstream_node_id) else { return };
				let mut downstream_input_index = None;
				for input_index in 0..2 {
					if let Some(NodeInput::Node { node_id: input_id, .. }) = downstream_node.inputs.get(input_index) {
						if *input_id == node_id {
							downstream_input_index = Some(input_index)
						}
					}
				}
				let Some(downstream_input_index) = downstream_input_index else {
					log::error!("Downstream input_index should always exist when moving layer");
					return;
				};

				let layer_to_move_sibling_input = document_network.nodes.get(&node_id).and_then(|node| node.inputs.get(0));
				if let Some(NodeInput::Node { node_id, .. }) = layer_to_move_sibling_input.and_then(|node_input| if reconnect_to_sibling { Some(node_input) } else { None }) {
					let upstream_sibling_id = *node_id;

					let Some(downstream_node) = document_network.nodes.get_mut(downstream_node_id) else { return };
					if let Some(NodeInput::Node { node_id, .. }) = downstream_node.inputs.get_mut(downstream_input_index) {
						*node_id = upstream_sibling_id;
					}

					let upstream_shift = IVec2::new(0, -3);
					responses.add(GraphOperationMessage::ShiftUpstream {
						node_id: upstream_sibling_id,
						shift: upstream_shift,
						shift_self: true,
					});
				} else {
					// Disconnect node directly downstream if upstream sibling doesn't exist
					responses.add(GraphOperationMessage::DisconnectInput {
						node_id: *downstream_node_id,
						input_index: downstream_input_index,
					});
				}

				responses.add(GraphOperationMessage::DisconnectInput { node_id, input_index: 0 });
			}
			GraphOperationMessage::FillSet { layer, fill } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run FillSet on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.fill_set(fill);
				}
			}
			GraphOperationMessage::InsertNodeAtStackIndex { node_id, parent, insert_index } => {
				let (post_node_id, pre_node_id, post_node_input_index) = ModifyInputsContext::get_post_node_with_index(document_network, parent, insert_index);
				let Some(post_node_id) = post_node_id else {
					log::error!("Post node should exist in InsertLayerAtStackIndex");
					return;
				};
				// `layer_to_move` should always correspond to a node.
				let Some(layer_to_move_node) = document_network.nodes.get(&node_id) else {
					log::error!("Layer node not found when inserting node {} at index {}", node_id, insert_index);
					return;
				};

				// Move current layer to post node.
				let post_node = document_network.nodes.get(&post_node_id).expect("Post node id should always refer to a node");
				let current_position = layer_to_move_node.metadata.position;
				let new_position = post_node.metadata.position;

				// If moved to top of a layer stack, move to the left of the post node. If moved within a stack, move directly on the post node. The stack will be shifted down later.
				let offset_to_post_node = if insert_index == 0 {
					new_position - current_position - IVec2::new(8, 0)
				} else {
					new_position - current_position
				};

				responses.add(GraphOperationMessage::ShiftUpstream {
					node_id: node_id,
					shift: offset_to_post_node,
					shift_self: true,
				});

				// Update post_node input to layer_to_move.
				if let Some(upstream_node) = pre_node_id {
					responses.add(GraphOperationMessage::InsertNodeBetween {
						post_node_id: post_node_id,
						post_node_input_index: post_node_input_index,
						insert_node_output_index: 0,
						insert_node_id: node_id,
						insert_node_input_index: 0,
						pre_node_output_index: 0,
						pre_node_id: upstream_node,
					})
				} else {
					responses.add(GraphOperationMessage::SetNodeInput {
						node_id: post_node_id,
						input_index: post_node_input_index,
						input: NodeInput::node(node_id, 0),
					})
				}

				// Shift stack down, starting at the moved node.
				responses.add(GraphOperationMessage::ShiftUpstream {
					node_id: node_id,
					shift: IVec2::new(0, 3),
					shift_self: true,
				});
			}
			GraphOperationMessage::InsertBooleanOperation { operation } => {
				let mut selected_layers = selected_nodes.selected_layers(&document_metadata);

				let first_selected_layer = selected_layers.next();
				let second_selected_layer = selected_layers.next();
				let other_selected_layer = selected_layers.next();

				let (Some(upper_layer), Some(lower_layer), None) = (first_selected_layer, second_selected_layer, other_selected_layer) else {
					return;
				};

				let Some(upper_layer_node) = document_network.nodes.get(&upper_layer.to_node()) else { return };
				let Some(lower_layer_node) = document_network.nodes.get(&lower_layer.to_node()) else { return };

				let Some(NodeInput::Node {
					node_id: upper_node_id,
					output_index: upper_output_index,
					..
				}) = upper_layer_node.inputs.get(1).cloned()
				else {
					return;
				};
				let Some(NodeInput::Node {
					node_id: lower_node_id,
					output_index: lower_output_index,
					..
				}) = lower_layer_node.inputs.get(1).cloned()
				else {
					return;
				};

				let boolean_operation_node_id = NodeId::new();

				// Store a history step before doing anything
				responses.add(DocumentMessage::StartTransaction);

				// Create the new Boolean Operation node
				responses.add(GraphOperationMessage::CreateBooleanOperationNode {
					node_id: boolean_operation_node_id,
					operation,
				});

				// Insert it in the upper layer's chain, right before it enters the upper layer
				responses.add(GraphOperationMessage::InsertNodeBetween {
					post_node_id: upper_layer.to_node(),
					post_node_input_index: 1,
					insert_node_id: boolean_operation_node_id,
					insert_node_output_index: 0,
					insert_node_input_index: 0,
					pre_node_id: upper_node_id,
					pre_node_output_index: upper_output_index,
				});

				// Connect the lower chain to the Boolean Operation node's lower input
				responses.add(GraphOperationMessage::SetNodeInput {
					node_id: boolean_operation_node_id,
					input_index: 1,
					input: NodeInput::node(lower_node_id, lower_output_index),
				});

				// Delete the lower layer (but its chain is kept since it's still used by the Boolean Operation node)
				responses.add(GraphOperationMessage::DeleteLayer { layer: lower_layer, reconnect: true });

				// Put the Boolean Operation where the output layer is located, since this is the correct shift relative to its left input chain
				responses.add(GraphOperationMessage::SetNodePosition {
					node_id: boolean_operation_node_id,
					position: upper_layer_node.metadata.position,
				});

				// After the previous step, the Boolean Operation node is overlapping the upper layer, so we need to shift and its entire chain to the left by its width plus some padding
				responses.add(GraphOperationMessage::ShiftUpstream {
					node_id: boolean_operation_node_id,
					shift: (-8, 0).into(),
					shift_self: true,
				})
			}
			GraphOperationMessage::InsertNodeBetween {
				post_node_id,
				post_node_input_index,
				insert_node_output_index,
				insert_node_id,
				insert_node_input_index,
				pre_node_output_index,
				pre_node_id,
			} => {
				let post_node = document_network.nodes.get(&post_node_id);
				let Some((post_node_input_index, _)) = post_node
					.map_or(&document_network.exports, |post_node| &post_node.inputs)
					.iter()
					.enumerate()
					.filter(|input| input.1.is_exposed())
					.nth(post_node_input_index)
				else {
					error!("Failed to find input index {post_node_input_index} on node {post_node_id:#?}");
					return;
				};
				let Some(insert_node) = document_network.nodes.get(&insert_node_id) else {
					error!("Insert node not found");
					return;
				};
				let Some((insert_node_input_index, _)) = insert_node.inputs.iter().enumerate().filter(|input| input.1.is_exposed()).nth(insert_node_input_index) else {
					error!("Failed to find input index {insert_node_input_index} on node {insert_node_id:#?}");
					return;
				};

				let post_input = NodeInput::node(insert_node_id, insert_node_output_index);
				responses.add(GraphOperationMessage::SetNodeInput {
					node_id: post_node_id,
					input_index: post_node_input_index,
					input: post_input,
				});

				let insert_input = NodeInput::node(pre_node_id, pre_node_output_index);
				responses.add(GraphOperationMessage::SetNodeInput {
					node_id: insert_node_id,
					input_index: insert_node_input_index,
					input: insert_input,
				});
			}
			GraphOperationMessage::OpacitySet { layer, opacity } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run OpacitySet on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.opacity_set(opacity);
				}
			}
			GraphOperationMessage::BlendModeSet { layer, blend_mode } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run BlendModeSet on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.blend_mode_set(blend_mode);
				}
			}
			GraphOperationMessage::UpdateBounds { layer, old_bounds, new_bounds } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run UpdateBounds on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.update_bounds(old_bounds, new_bounds);
				}
			}
			GraphOperationMessage::StrokeSet { layer, stroke } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run StrokeSet on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.stroke_set(stroke);
				}
			}
			GraphOperationMessage::TransformChange {
				layer,
				transform,
				transform_in,
				skip_rerender,
			} => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run TransformChange on ROOT_PARENT");
					return;
				}
				let parent_transform = document_metadata.downstream_transform_to_viewport(layer);
				let bounds = LayerBounds::new(document_metadata, layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.transform_change(transform, transform_in, parent_transform, bounds, skip_rerender);
				}
			}
			GraphOperationMessage::TransformSet {
				layer,
				transform,
				transform_in,
				skip_rerender,
			} => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run TransformSet on ROOT_PARENT");
					return;
				}
				let parent_transform = document_metadata.downstream_transform_to_viewport(layer);

				let current_transform = Some(document_metadata.transform_to_viewport(layer));
				let bounds = LayerBounds::new(document_metadata, layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, current_transform, bounds, skip_rerender);
				}
			}
			GraphOperationMessage::TransformSetPivot { layer, pivot } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run TransformSetPivot on ROOT_PARENT");
					return;
				}
				let bounds = LayerBounds::new(document_metadata, layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.pivot_set(pivot, bounds);
				}
			}
			GraphOperationMessage::Vector { layer, modification } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run Vector on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					let previous_layer = modify_inputs.vector_modify(modification);
					if let Some(layer) = previous_layer {
						responses.add(GraphOperationMessage::DeleteLayer { layer, reconnect: true })
					}
				}
			}
			GraphOperationMessage::Brush { layer, strokes } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run Brush on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.brush_modify(strokes);
				}
			}
			GraphOperationMessage::MoveSelectedSiblingsToChild { new_parent } => {
				let Some(group_parent) = new_parent.parent(&document_metadata) else {
					log::error!("Could not find parent for layer {:?}", new_parent);
					return;
				};

				// Create a vec of nodes to move with all selected layers in the parent layer child stack, as well as each non layer sibling directly upstream of the selected layer
				let mut selected_siblings = Vec::new();

				// Skip over horizontal non layer node chain that feeds into parent
				let Some(mut current_stack_node_id) = group_parent.first_child(&document_metadata).and_then(|current_stack_node| Some(current_stack_node.to_node())) else {
					log::error!("Folder should always have child");
					return;
				};
				let current_stack_node_id = &mut current_stack_node_id;

				loop {
					let mut current_stack_node = document_network.nodes.get(current_stack_node_id).expect("Current stack node id should always be a node");

					// Check if the current stack node is a selected layer
					if selected_nodes
						.selected_layers(&document_metadata)
						.any(|selected_node_id| selected_node_id.to_node() == *current_stack_node_id)
					{
						selected_siblings.push(*current_stack_node_id);

						// Push all non layer sibling nodes directly upstream of the selected layer
						loop {
							let Some(NodeInput::Node { node_id, .. }) = current_stack_node.inputs.get(0) else { break };

							let next_node = document_network.nodes.get(node_id).expect("Stack node id should always be a node");

							// If the next node is a layer, immediately break and leave current stack node as the non layer node
							if next_node.is_layer {
								break;
							}

							*current_stack_node_id = *node_id;
							current_stack_node = next_node;

							selected_siblings.push(*current_stack_node_id);
						}
					}

					// Get next node
					let Some(NodeInput::Node { node_id, .. }) = current_stack_node.inputs.get(0) else { break };
					*current_stack_node_id = *node_id;
				}

				// Start with the furthest upstream node, move it as a child of the new folder, and continue downstream for each layer in vec
				for node_to_move in selected_siblings.iter().rev() {
					// Disconnect node, then reconnect as new child
					responses.add(GraphOperationMessage::DisconnectNodeFromStack {
						node_id: *node_to_move,
						reconnect_to_sibling: true,
					});

					responses.add(GraphOperationMessage::InsertNodeAtStackIndex {
						node_id: *node_to_move,
						parent: new_parent,
						insert_index: 0,
					});
				}

				let Some(most_upstream_sibling) = selected_siblings.last() else {
					return;
				};
				responses.add(GraphOperationMessage::DisconnectInput {
					node_id: *most_upstream_sibling,
					input_index: 0,
				});
			}
			GraphOperationMessage::NewArtboard { id, artboard } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(artboard_id) = modify_inputs.create_artboard(id, artboard) {
					responses.add_front(NodeGraphMessage::SelectedNodesSet { nodes: vec![artboard_id] });
				}
				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::NewBitmapLayer {
				id,
				image_frame,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer_with_insert_index(id, insert_index, parent) {
					modify_inputs.insert_image_data(image_frame, layer);
				}
			}
			GraphOperationMessage::NewCustomLayer {
				id,
				nodes,
				parent,
				insert_index,
				alias,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);

				if let Some(layer) = modify_inputs.create_layer_with_insert_index(id, insert_index, parent) {
					let new_ids: HashMap<_, _> = nodes.iter().map(|(&id, _)| (id, NodeId(generate_uuid()))).collect();

					if let Some(node) = modify_inputs.document_network.nodes.get_mut(&id) {
						node.alias = alias.clone();
					}

					let shift = nodes
						.get(&NodeId(0))
						.and_then(|node| {
							modify_inputs
								.document_network
								.nodes
								.get(&layer)
								.map(|layer| layer.metadata.position - node.metadata.position + IVec2::new(-8, 0))
						})
						.unwrap_or_default();

					for (old_id, mut document_node) in nodes {
						// Shift copied node
						document_node.metadata.position += shift;

						// Get the new, non-conflicting id
						let node_id = *new_ids.get(&old_id).unwrap();
						document_node = document_node.map_ids(NodeGraphMessageHandler::default_node_input, &new_ids);

						// Insert node into network
						modify_inputs.document_network.nodes.insert(node_id, document_node);
					}

					if let Some(layer_node) = modify_inputs.document_network.nodes.get_mut(&layer) {
						if let Some(&input) = new_ids.get(&NodeId(0)) {
							layer_node.inputs[1] = NodeInput::node(input, 0);
						}
					}

					modify_inputs.responses.add(NodeGraphMessage::RunDocumentGraph);
				} else {
					error!("Creating new custom layer failed");
				}

				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer_with_insert_index(id, insert_index, parent) {
					modify_inputs.insert_vector_data(subpaths, layer);
				}
				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::NewTextLayer {
				id,
				text,
				font,
				size,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer_with_insert_index(id, insert_index, parent) {
					modify_inputs.insert_text(text, font, size, layer);
				}
				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::ResizeArtboard { id, location, dimensions } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(id, document_network, document_metadata, node_graph, responses) {
					modify_inputs.resize_artboard(location, dimensions);
				}
			}
			GraphOperationMessage::ClearArtboards => {
				for &artboard in document_metadata.all_artboards() {
					responses.add(GraphOperationMessage::DeleteLayer { layer: artboard, reconnect: true });
				}
				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::NewSvg {
				id,
				svg,
				transform,
				parent,
				insert_index,
			} => {
				let tree = match usvg::Tree::from_str(&svg, &usvg::Options::default()) {
					Ok(t) => t,
					Err(e) => {
						responses.add(DocumentMessage::DocumentHistoryBackward);
						responses.add(DialogMessage::DisplayDialogError {
							title: "SVG parsing failed".to_string(),
							description: e.to_string(),
						});
						return;
					}
				};
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);

				import_usvg_node(&mut modify_inputs, &usvg::Node::Group(Box::new(tree.root)), transform, id, parent, insert_index);
				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::SetNodePosition { node_id, position } => {
				let Some(node) = document_network.nodes.get_mut(&node_id) else {
					log::error!("Failed to find node {node_id} when setting position");
					return;
				};
				node.metadata.position = position;
			}
			GraphOperationMessage::SetName { layer, name } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::SetNameImpl { layer, name });
			}
			GraphOperationMessage::SetNameImpl { layer, name } => {
				let Some(node) = document_network.nodes.get_mut(&layer.to_node()) else { return };
				node.alias = name;
				responses.add(NodeGraphMessage::SendGraph);
			}
			GraphOperationMessage::SetNodeInput { node_id, input_index, input } => {
				if ModifyInputsContext::set_input(document_network, node_id, input_index, input, true) {
					load_network_structure(document_network, document_metadata, collapsed);
				}
			}
			GraphOperationMessage::ShiftUpstream { node_id, shift, shift_self } => {
				ModifyInputsContext::shift_upstream(document_network, node_id, shift, shift_self);
			}
			GraphOperationMessage::ToggleSelectedVisibility => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes.selected_layers(&document_metadata).all(|layer| document_metadata.node_is_visible(layer.to_node()));

				for layer in selected_nodes.selected_layers(&document_metadata) {
					responses.add(GraphOperationMessage::SetVisibility { node_id: layer.to_node(), visible });
				}
			}
			GraphOperationMessage::ToggleVisibility { node_id } => {
				let visible = !document_metadata.node_is_visible(node_id);
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::SetVisibility { node_id, visible });
			}
			GraphOperationMessage::SetVisibility { node_id, visible } => {
				// Set what we determined shall be the visibility of the node
				let Some(node) = document_network.nodes.get_mut(&node_id) else {
					log::error!("Could not get node {:?} in GraphOperationMessage::SetVisibility", node_id);
					return;
				};
				node.visible = visible;

				// Only generate node graph if one of the selected nodes is connected to the output
				if document_network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				document_metadata.load_structure(document_network);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			GraphOperationMessage::ToggleSelectedLocked => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes.selected_layers(&document_metadata).all(|layer| document_metadata.node_is_locked(layer.to_node()));

				for layer in selected_nodes.selected_layers(&document_metadata) {
					responses.add(GraphOperationMessage::SetVisibility { node_id: layer.to_node(), visible });
				}
			}
			GraphOperationMessage::ToggleLocked { node_id } => {
				let Some(node) = document_network.nodes.get(&node_id) else {
					log::error!("Cannot get node {:?} in GraphOperationMessage::ToggleLocked", node_id);
					return;
				};

				let locked = !node.locked;
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::SetLocked { node_id, locked });
			}
			GraphOperationMessage::SetLocked { node_id, locked } => {
				let Some(node) = document_network.nodes.get_mut(&node_id) else { return };
				node.locked = locked;

				if document_network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				document_metadata.load_structure(document_network);
				responses.add(NodeGraphMessage::SelectedNodesUpdated)
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GraphOperationMessage;)
	}
}

pub fn load_network_structure(document_network: &NodeNetwork, document_metadata: &mut DocumentMetadata, collapsed: &mut CollapsedLayers) {
	document_metadata.load_structure(document_network);
	collapsed.0.retain(|&layer| document_metadata.layer_exists(layer));
}

fn usvg_color(c: usvg::Color, a: f32) -> Color {
	Color::from_rgbaf32_unchecked(c.red as f32 / 255., c.green as f32 / 255., c.blue as f32 / 255., a)
}

fn usvg_transform(c: usvg::Transform) -> DAffine2 {
	DAffine2::from_cols_array(&[c.sx as f64, c.ky as f64, c.kx as f64, c.sy as f64, c.tx as f64, c.ty as f64])
}

fn import_usvg_node(modify_inputs: &mut ModifyInputsContext, node: &usvg::Node, transform: DAffine2, id: NodeId, parent: LayerNodeIdentifier, insert_index: isize) {
	let Some(layer) = modify_inputs.create_layer_with_insert_index(id, insert_index, parent) else {
		return;
	};
	modify_inputs.layer_node = Some(layer);
	match node {
		usvg::Node::Group(group) => {
			for child in &group.children {
				import_usvg_node(modify_inputs, child, transform, NodeId(generate_uuid()), LayerNodeIdentifier::new_unchecked(layer), -1);
			}
			modify_inputs.layer_node = Some(layer);
		}
		usvg::Node::Path(path) => {
			let subpaths = convert_usvg_path(path);
			let bounds = subpaths.iter().filter_map(|subpath| subpath.bounding_box()).reduce(Quad::combine_bounds).unwrap_or_default();
			let transformed_bounds = subpaths
				.iter()
				.filter_map(|subpath| subpath.bounding_box_with_transform(transform * usvg_transform(node.abs_transform())))
				.reduce(Quad::combine_bounds)
				.unwrap_or_default();
			modify_inputs.insert_vector_data(subpaths, layer);

			let center = DAffine2::from_translation((bounds[0] + bounds[1]) / 2.);

			modify_inputs.modify_inputs("Transform", true, |inputs, _node_id, _metadata| {
				transform_utils::update_transform(inputs, center.inverse() * transform * usvg_transform(node.abs_transform()) * center);
			});
			let bounds_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
			let transformed_bound_transform = DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);
			apply_usvg_fill(
				&path.fill,
				modify_inputs,
				transform * usvg_transform(node.abs_transform()),
				bounds_transform,
				transformed_bound_transform,
			);
			apply_usvg_stroke(&path.stroke, modify_inputs);
		}
		usvg::Node::Image(_image) => {
			warn!("Skip image")
		}
		usvg::Node::Text(text) => {
			let font = Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_core::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks.iter().map(|chunk| chunk.text.clone()).collect(), font, 24., layer);
			modify_inputs.fill_set(Fill::Solid(Color::BLACK));
		}
	}
}

fn apply_usvg_stroke(stroke: &Option<usvg::Stroke>, modify_inputs: &mut ModifyInputsContext) {
	if let Some(stroke) = stroke {
		if let usvg::Paint::Color(color) = &stroke.paint {
			modify_inputs.stroke_set(Stroke {
				color: Some(usvg_color(*color, stroke.opacity.get())),
				weight: stroke.width.get() as f64,
				dash_lengths: stroke.dasharray.as_ref().map(|lengths| lengths.iter().map(|&length| length as f64).collect()).unwrap_or_default(),
				dash_offset: stroke.dashoffset as f64,
				line_cap: match stroke.linecap {
					usvg::LineCap::Butt => LineCap::Butt,
					usvg::LineCap::Round => LineCap::Round,
					usvg::LineCap::Square => LineCap::Square,
				},
				line_join: match stroke.linejoin {
					usvg::LineJoin::Miter => LineJoin::Miter,
					usvg::LineJoin::MiterClip => LineJoin::Miter,
					usvg::LineJoin::Round => LineJoin::Round,
					usvg::LineJoin::Bevel => LineJoin::Bevel,
				},
				line_join_miter_limit: stroke.miterlimit.get() as f64,
			})
		} else {
			warn!("Skip non-solid stroke")
		}
	}
}

fn apply_usvg_fill(fill: &Option<usvg::Fill>, modify_inputs: &mut ModifyInputsContext, transform: DAffine2, bounds_transform: DAffine2, transformed_bound_transform: DAffine2) {
	if let Some(fill) = &fill {
		modify_inputs.fill_set(match &fill.paint {
			usvg::Paint::Color(color) => Fill::solid(usvg_color(*color, fill.opacity.get())),
			usvg::Paint::LinearGradient(linear) => {
				let local = [DVec2::new(linear.x1 as f64, linear.y1 as f64), DVec2::new(linear.x2 as f64, linear.y2 as f64)];

				let to_doc_transform = if linear.base.units == usvg::Units::UserSpaceOnUse {
					transform
				} else {
					transformed_bound_transform
				};
				let to_doc = to_doc_transform * usvg_transform(linear.transform);

				let document = [to_doc.transform_point2(local[0]), to_doc.transform_point2(local[1])];
				let layer = [transform.inverse().transform_point2(document[0]), transform.inverse().transform_point2(document[1])];

				let [start, end] = [bounds_transform.inverse().transform_point2(layer[0]), bounds_transform.inverse().transform_point2(layer[1])];

				Fill::Gradient(Gradient {
					start,
					end,
					transform: DAffine2::IDENTITY,
					gradient_type: GradientType::Linear,
					positions: linear.stops.iter().map(|stop| (stop.offset.get() as f64, usvg_color(stop.color, stop.opacity.get()))).collect(),
				})
			}
			usvg::Paint::RadialGradient(radial) => {
				let local = [DVec2::new(radial.cx as f64, radial.cy as f64), DVec2::new(radial.fx as f64, radial.fy as f64)];

				let to_doc_transform = if radial.base.units == usvg::Units::UserSpaceOnUse {
					transform
				} else {
					transformed_bound_transform
				};
				let to_doc = to_doc_transform * usvg_transform(radial.transform);

				let document = [to_doc.transform_point2(local[0]), to_doc.transform_point2(local[1])];
				let layer = [transform.inverse().transform_point2(document[0]), transform.inverse().transform_point2(document[1])];

				let [start, end] = [bounds_transform.inverse().transform_point2(layer[0]), bounds_transform.inverse().transform_point2(layer[1])];

				Fill::Gradient(Gradient {
					start,
					end,
					transform: DAffine2::IDENTITY,
					gradient_type: GradientType::Radial,
					positions: radial.stops.iter().map(|stop| (stop.offset.get() as f64, usvg_color(stop.color, stop.opacity.get()))).collect(),
				})
			}
			usvg::Paint::Pattern(_) => {
				warn!("Skip pattern");
				return;
			}
		});
	}
}
