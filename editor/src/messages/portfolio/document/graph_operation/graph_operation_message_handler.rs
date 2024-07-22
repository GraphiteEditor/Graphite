use super::transform_utils;
use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, NodeId, NodeInput, NodeNetwork, Previewing};
use graphene_core::renderer::Quad;
use graphene_core::text::Font;
use graphene_core::vector::style::{Fill, Gradient, GradientStops, GradientType, LineCap, LineJoin, Stroke};
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
					.get_root_node()
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
					let default_inputs = NodeGraphMessageHandler::get_default_inputs(document_network, &Vec::new(), node_id, &node_graph.resolved_types, &document_node);
					document_node = document_node.map_ids(default_inputs, &new_ids);

					// Insert node into network
					node_graph.insert_node(node_id, document_node, document_network, &Vec::new());
				}

				let Some(new_layer_id) = new_ids.get(&NodeId(0)) else {
					error!("Could not get layer node when adding as child");
					return;
				};

				let insert_index = if insert_index < 0 { 0 } else { insert_index as usize };
				let (downstream_node, upstream_node, input_index) = ModifyInputsContext::get_post_node_with_index(document_network, parent, insert_index);

				responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![*new_layer_id] });

				match (downstream_node, upstream_node) {
					(Some(downstream_node), Some(upstream_node)) => responses.add(GraphOperationMessage::InsertNodeBetween {
						post_node_id: downstream_node,
						post_node_input_index: input_index,
						insert_node_output_index: 0,
						insert_node_id: *new_layer_id,
						insert_node_input_index: 0,
						pre_node_output_index: 0,
						pre_node_id: upstream_node,
					}),
					(Some(downstream_node), None) => responses.add(GraphOperationMessage::SetNodeInput {
						node_id: downstream_node,
						input_index,
						input: NodeInput::node(*new_layer_id, 0),
					}),
					(None, Some(upstream_node)) => responses.add(GraphOperationMessage::InsertNodeBetween {
						post_node_id: document_network.exports_metadata.0,
						post_node_input_index: 0,
						insert_node_output_index: 0,
						insert_node_id: *new_layer_id,
						insert_node_input_index: 0,
						pre_node_output_index: 0,
						pre_node_id: upstream_node,
					}),
					(None, None) => {
						if let Some(primary_export) = document_network.exports.get_mut(0) {
							*primary_export = NodeInput::node(*new_layer_id, 0)
						}
					}
				};
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

				node_graph.insert_node(node_id, new_boolean_operation_node, document_network, &Vec::new());
			}
			GraphOperationMessage::DeleteLayer { layer, reconnect } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot delete ROOT_PARENT");
					return;
				}
				ModifyInputsContext::delete_nodes(node_graph, document_network, selected_nodes, vec![layer.to_node()], reconnect, responses, Vec::new());

				load_network_structure(document_network, document_metadata, collapsed);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			// Make sure to also update NodeGraphMessage::DisconnectInput when changing this
			GraphOperationMessage::DisconnectInput { node_id, input_index } => {
				let Some(existing_input) = document_network
					.nodes
					.get(&node_id)
					.map_or_else(|| document_network.exports.get(input_index), |node| node.inputs.get(input_index))
				else {
					warn!("Could not find input for {node_id} at index {input_index} when disconnecting");
					return;
				};

				let tagged_value = TaggedValue::from_type(&ModifyInputsContext::get_input_type(document_network, &Vec::new(), node_id, &node_graph.resolved_types, input_index));

				let mut input = NodeInput::value(tagged_value, true);
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = existing_input.is_exposed();
				}
				if node_id == document_network.exports_metadata.0 {
					// Since it is only possible to drag the solid line, there must be a root_node_to_restore
					if let Previewing::Yes { .. } = document_network.previewing {
						responses.add(GraphOperationMessage::StartPreviewingWithoutRestore);
					}
					// If there is no preview, then disconnect
					else {
						responses.add(GraphOperationMessage::SetNodeInput { node_id, input_index, input });
					}
				} else {
					responses.add(GraphOperationMessage::SetNodeInput { node_id, input_index, input });
				}
				if document_network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			GraphOperationMessage::DisconnectNodeFromStack { node_id, reconnect_to_sibling } => {
				ModifyInputsContext::remove_references_from_network(node_graph, document_network, node_id, reconnect_to_sibling, &Vec::new());
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

				// `layer_to_move` should always correspond to a node.
				let Some(layer_to_move_node) = document_network.nodes.get(&node_id) else {
					log::error!("Layer node not found when inserting node {} at index {}", node_id, insert_index);
					return;
				};

				// Move current layer to post node.
				let current_position = layer_to_move_node.metadata.position;
				let new_position = if let Some(post_node_id) = post_node_id {
					document_network.nodes.get(&post_node_id).expect("Post node id should always refer to a node").metadata.position
				} else if let Some(root_node) = document_network.get_root_node() {
					document_network.nodes.get(&root_node.id).expect("Root node id should always refer to a node").metadata.position + IVec2::new(8, -3)
				} else {
					document_network.exports_metadata.1
				};

				// If moved to top of a layer stack, move to the left of the post node. If moved within a stack, move directly on the post node. The stack will be shifted down later.
				let offset_to_post_node = if insert_index == 0 {
					new_position - current_position - IVec2::new(8, 0)
				} else {
					new_position - current_position
				};

				responses.add(GraphOperationMessage::ShiftUpstream {
					node_id,
					shift: offset_to_post_node,
					shift_self: true,
				});

				match (post_node_id, pre_node_id) {
					(Some(post_node_id), Some(pre_node_id)) => responses.add(GraphOperationMessage::InsertNodeBetween {
						post_node_id,
						post_node_input_index,
						insert_node_output_index: 0,
						insert_node_id: node_id,
						insert_node_input_index: 0,
						pre_node_output_index: 0,
						pre_node_id,
					}),
					(None, Some(pre_node_id)) => responses.add(GraphOperationMessage::InsertNodeBetween {
						post_node_id: document_network.exports_metadata.0,
						post_node_input_index: 0,
						insert_node_output_index: 0,
						insert_node_id: node_id,
						insert_node_input_index: 0,
						pre_node_output_index: 0,
						pre_node_id,
					}),
					(Some(post_node_id), None) => responses.add(GraphOperationMessage::SetNodeInput {
						node_id: post_node_id,
						input_index: post_node_input_index,
						input: NodeInput::node(node_id, 0),
					}),
					(None, None) => {
						if let Some(primary_export) = document_network.exports.get_mut(0) {
							*primary_export = NodeInput::node(node_id, 0)
						}
					}
				}

				// Shift stack down, starting at the moved node.
				responses.add(GraphOperationMessage::ShiftUpstream {
					node_id,
					shift: IVec2::new(0, 3),
					shift_self: true,
				});
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
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.transform_change(transform, transform_in, parent_transform, skip_rerender);
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
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, current_transform, skip_rerender);
				}
			}
			GraphOperationMessage::TransformSetPivot { layer, pivot } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run TransformSetPivot on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.pivot_set(pivot);
				}
			}
			GraphOperationMessage::Vector { layer, modification_type } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run Vector on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.vector_modify(modification_type);
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
				let Some(group_parent) = new_parent.parent(document_metadata) else {
					log::error!("Could not find parent for layer {:?}", new_parent);
					return;
				};

				// Create a vec of nodes to move with all selected layers in the parent layer child stack, as well as each non layer sibling directly upstream of the selected layer
				let mut selected_siblings = Vec::new();

				// Skip over horizontal non layer node chain that feeds into parent
				let Some(mut current_stack_node_id) = group_parent.first_child(document_metadata).map(|current_stack_node| current_stack_node.to_node()) else {
					log::error!("Folder should always have child");
					return;
				};
				let current_stack_node_id = &mut current_stack_node_id;

				loop {
					let mut current_stack_node = document_network.nodes.get(current_stack_node_id).expect("Current stack node id should always be a node");

					// Check if the current stack node is a selected layer
					if selected_nodes
						.selected_layers(document_metadata)
						.any(|selected_node_id| selected_node_id.to_node() == *current_stack_node_id)
					{
						selected_siblings.push(*current_stack_node_id);

						// Push all non layer sibling nodes directly upstream of the selected layer
						loop {
							let Some(NodeInput::Node { node_id, .. }) = current_stack_node.inputs.first() else { break };

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
					let Some(NodeInput::Node { node_id, .. }) = current_stack_node.inputs.first() else { break };
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
				if let Some(artboard_id) = ModifyInputsContext::create_artboard(node_graph, document_network, id, artboard) {
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
				if let Some(layer) = modify_inputs.create_layer(id, parent, insert_index) {
					ModifyInputsContext::insert_image_data(node_graph, document_network, image_frame, layer, responses);
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

				if let Some(layer) = modify_inputs.create_layer(id, parent, insert_index) {
					let new_ids: HashMap<_, _> = nodes.iter().map(|(&id, _)| (id, NodeId(generate_uuid()))).collect();

					if let Some(node) = modify_inputs.document_network.nodes.get_mut(&id) {
						node.alias.clone_from(&alias);
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
						let default_inputs = NodeGraphMessageHandler::get_default_inputs(document_network, &Vec::new(), node_id, &node_graph.resolved_types, &document_node);
						document_node = document_node.map_ids(default_inputs, &new_ids);

						// Insert node into network
						node_graph.insert_node(node_id, document_node, document_network, &Vec::new());
						node_graph.update_click_target(node_id, document_network, Vec::new());
					}

					if let Some(layer_node) = document_network.nodes.get_mut(&layer) {
						if let Some(&input) = new_ids.get(&NodeId(0)) {
							layer_node.inputs[1] = NodeInput::node(input, 0);
						}
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
				} else {
					error!("Creating new custom layer failed");
				}

				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer(id, parent, insert_index) {
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
				if let Some(layer) = modify_inputs.create_layer(id, parent, insert_index) {
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

				import_usvg_node(&mut modify_inputs, &usvg::Node::Group(Box::new(tree.root().clone())), transform, id, parent, insert_index);
				load_network_structure(document_network, document_metadata, collapsed);
			}
			GraphOperationMessage::SetNodePosition { node_id, position } => {
				let Some(node) = document_network.nodes.get_mut(&node_id) else {
					log::error!("Failed to find node {node_id} when setting position");
					return;
				};
				node.metadata.position = position;
				node_graph.update_click_target(node_id, document_network, Vec::new());
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
			}
			GraphOperationMessage::SetName { layer, name } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::SetNameImpl { layer, name });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::SetNameImpl { layer, name } => {
				if let Some(node) = document_network.nodes.get_mut(&layer.to_node()) {
					node.alias = name;
					if let Some(node_metadata) = node_graph.node_metadata.get_mut(&layer.to_node()) {
						node_metadata.layer_width = Some(NodeGraphMessageHandler::layer_width_cells(node));
					};
					node_graph.update_click_target(layer.to_node(), document_network, Vec::new());
					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
					responses.add(NodeGraphMessage::SendGraph);
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			GraphOperationMessage::SetNodeInput { node_id, input_index, input } => {
				if ModifyInputsContext::set_input(node_graph, document_network, &Vec::new(), node_id, input_index, input, true) {
					load_network_structure(document_network, document_metadata, collapsed);
				}
			}
			GraphOperationMessage::ShiftUpstream { node_id, shift, shift_self } => {
				ModifyInputsContext::shift_upstream(node_graph, document_network, &Vec::new(), node_id, shift, shift_self);
			}
			GraphOperationMessage::ToggleSelectedVisibility => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes.selected_layers(document_metadata).all(|layer| document_metadata.node_is_visible(layer.to_node()));

				for layer in selected_nodes.selected_layers(document_metadata) {
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
			GraphOperationMessage::StartPreviewingWithoutRestore => {
				document_network.start_previewing_without_restore();
			}
			GraphOperationMessage::ToggleSelectedLocked => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are locked, show them all. Otherwise, hide them all.
				let locked = !selected_nodes.selected_layers(document_metadata).all(|layer| document_metadata.node_is_locked(layer.to_node()));

				for layer in selected_nodes.selected_layers(document_metadata) {
					responses.add(GraphOperationMessage::SetLocked { node_id: layer.to_node(), locked });
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
	let Some(layer) = modify_inputs.create_layer(id, parent, insert_index) else {
		return;
	};
	modify_inputs.layer_node = Some(layer);
	match node {
		usvg::Node::Group(group) => {
			for child in group.children() {
				import_usvg_node(modify_inputs, child, transform, NodeId(generate_uuid()), LayerNodeIdentifier::new_unchecked(layer), -1);
			}
			modify_inputs.layer_node = Some(layer);
		}
		usvg::Node::Path(path) => {
			let subpaths = convert_usvg_path(path);
			let bounds = subpaths.iter().filter_map(|subpath| subpath.bounding_box()).reduce(Quad::combine_bounds).unwrap_or_default();
			modify_inputs.insert_vector_data(subpaths, layer);

			modify_inputs.modify_inputs("Transform", true, |inputs, _node_id, _metadata| {
				transform_utils::update_transform(inputs, transform * usvg_transform(node.abs_transform()));
			});
			let bounds_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
			apply_usvg_fill(path.fill(), modify_inputs, transform * usvg_transform(node.abs_transform()), bounds_transform);
			apply_usvg_stroke(path.stroke(), modify_inputs);
		}
		usvg::Node::Image(_image) => {
			warn!("Skip image")
		}
		usvg::Node::Text(text) => {
			let font = Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_core::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks().iter().map(|chunk| chunk.text()).collect(), font, 24., layer);
			modify_inputs.fill_set(Fill::Solid(Color::BLACK));
		}
	}
}

fn apply_usvg_stroke(stroke: Option<&usvg::Stroke>, modify_inputs: &mut ModifyInputsContext) {
	if let Some(stroke) = stroke {
		if let usvg::Paint::Color(color) = &stroke.paint() {
			modify_inputs.stroke_set(Stroke {
				color: Some(usvg_color(*color, stroke.opacity().get())),
				weight: stroke.width().get() as f64,
				dash_lengths: stroke.dasharray().as_ref().map(|lengths| lengths.iter().map(|&length| length as f64).collect()).unwrap_or_default(),
				dash_offset: stroke.dashoffset() as f64,
				line_cap: match stroke.linecap() {
					usvg::LineCap::Butt => LineCap::Butt,
					usvg::LineCap::Round => LineCap::Round,
					usvg::LineCap::Square => LineCap::Square,
				},
				line_join: match stroke.linejoin() {
					usvg::LineJoin::Miter => LineJoin::Miter,
					usvg::LineJoin::MiterClip => LineJoin::Miter,
					usvg::LineJoin::Round => LineJoin::Round,
					usvg::LineJoin::Bevel => LineJoin::Bevel,
				},
				line_join_miter_limit: stroke.miterlimit().get() as f64,
			})
		} else {
			warn!("Skip non-solid stroke")
		}
	}
}

fn apply_usvg_fill(fill: Option<&usvg::Fill>, modify_inputs: &mut ModifyInputsContext, transform: DAffine2, bounds_transform: DAffine2) {
	if let Some(fill) = &fill {
		modify_inputs.fill_set(match &fill.paint() {
			usvg::Paint::Color(color) => Fill::solid(usvg_color(*color, fill.opacity().get())),
			usvg::Paint::LinearGradient(linear) => {
				let local = [DVec2::new(linear.x1() as f64, linear.y1() as f64), DVec2::new(linear.x2() as f64, linear.y2() as f64)];

				// TODO: fix this
				// let to_doc_transform = if linear.base.units() == usvg::Units::UserSpaceOnUse {
				// 	transform
				// } else {
				// 	transformed_bound_transform
				// };
				let to_doc_transform = transform;
				let to_doc = to_doc_transform * usvg_transform(linear.transform());

				let document = [to_doc.transform_point2(local[0]), to_doc.transform_point2(local[1])];
				let layer = [transform.inverse().transform_point2(document[0]), transform.inverse().transform_point2(document[1])];

				let [start, end] = [bounds_transform.inverse().transform_point2(layer[0]), bounds_transform.inverse().transform_point2(layer[1])];
				let stops = linear.stops().iter().map(|stop| (stop.offset().get() as f64, usvg_color(stop.color(), stop.opacity().get()))).collect();
				let stops = GradientStops(stops);

				Fill::Gradient(Gradient {
					start,
					end,
					transform: DAffine2::IDENTITY,
					gradient_type: GradientType::Linear,
					stops,
				})
			}
			usvg::Paint::RadialGradient(radial) => {
				let local = [DVec2::new(radial.cx() as f64, radial.cy() as f64), DVec2::new(radial.fx() as f64, radial.fy() as f64)];

				// TODO: fix this
				// let to_doc_transform = if radial.base.units == usvg::Units::UserSpaceOnUse {
				// 	transform
				// } else {
				// 	transformed_bound_transform
				// };
				let to_doc_transform = transform;
				let to_doc = to_doc_transform * usvg_transform(radial.transform());

				let document = [to_doc.transform_point2(local[0]), to_doc.transform_point2(local[1])];
				let layer = [transform.inverse().transform_point2(document[0]), transform.inverse().transform_point2(document[1])];

				let [start, end] = [bounds_transform.inverse().transform_point2(layer[0]), bounds_transform.inverse().transform_point2(layer[1])];
				let stops = radial.stops().iter().map(|stop| (stop.offset().get() as f64, usvg_color(stop.color(), stop.opacity().get()))).collect();
				let stops = GradientStops(stops);

				Fill::Gradient(Gradient {
					start,
					end,
					transform: DAffine2::IDENTITY,
					gradient_type: GradientType::Radial,
					stops,
				})
			}
			usvg::Paint::Pattern(_) => {
				warn!("Skip pattern");
				return;
			}
		});
	}
}
