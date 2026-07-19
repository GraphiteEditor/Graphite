use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::graph_operation::utility_types::{TransformIn, import_usvg_node};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_clip_mode;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::descriptor;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Artboard;
use graphene_std::list::List;
use graphene_std::renderer::usvg_utils::extract_graphite_gradient_stops;

#[derive(ExtractField)]
pub struct GraphOperationMessageContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub collapsed: &'a mut CollapsedLayers,
	pub node_graph: &'a mut NodeGraphMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize, ExtractField)]
pub struct GraphOperationMessageHandler {}

// GraphOperationMessageHandler always modified the document network. This is so changes to the Layers panel will only affect the document network.
// For changes to the selected network, use NodeGraphMessageHandler. No NodeGraphMessage's should be added here, since they will affect the selected nested network.
#[message_handler_data]
impl MessageHandler<GraphOperationMessage, GraphOperationMessageContext<'_>> for GraphOperationMessageHandler {
	fn process_message(&mut self, message: GraphOperationMessage, responses: &mut VecDeque<Message>, context: GraphOperationMessageContext) {
		let GraphOperationMessageContext { network_interface, .. } = context;

		match message {
			GraphOperationMessage::FillColorSet { layer, color } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.fill_color_set(color);
				}
			}
			GraphOperationMessage::FillGradientSet {
				layer,
				gradient,
				gradient_type,
				spread_method,
				transform,
			} => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.fill_gradient_set(gradient, gradient_type, spread_method, transform);
				}
			}
			GraphOperationMessage::BlendingFillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.opacity_fill_set(fill);
				}
			}
			GraphOperationMessage::GradientStopsSet { layer, stops } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_stops_set(stops);
				}
			}
			GraphOperationMessage::GradientTransformSet { layer, transform } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_transform_set(transform);
				}
			}
			GraphOperationMessage::GradientTypeSet { layer, gradient_type } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_type_set(gradient_type);
				}
			}
			GraphOperationMessage::GradientSpreadMethodSet { layer, spread_method } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_spread_method_set(spread_method);
				}
			}
			GraphOperationMessage::OpacitySet { layer, opacity } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.opacity_set(opacity);
				}
			}
			GraphOperationMessage::BlendModeSet { layer, blend_mode } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.blend_mode_set(blend_mode);
				}
			}
			GraphOperationMessage::ClipModeToggle { layer } => {
				let clip_mode = get_clip_mode(layer, network_interface);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.clip_mode_toggle(clip_mode);
				}
			}
			GraphOperationMessage::StrokeSet { layer, color, stroke } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.stroke_set(color, stroke);
				}
			}
			GraphOperationMessage::TransformChange {
				layer,
				transform,
				transform_in,
				skip_rerender,
			} => {
				let parent_transform = network_interface.document_metadata().downstream_transform_to_viewport(layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.transform_change_with_parent(transform, transform_in, parent_transform, skip_rerender);
				}
			}
			GraphOperationMessage::TransformSet {
				layer,
				transform,
				transform_in,
				skip_rerender,
			} => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.transform_set(transform, transform_in, skip_rerender);
				}
			}
			GraphOperationMessage::Vector { layer, modification_type } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run Vector on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.vector_modify(modification_type);
				}
			}
			GraphOperationMessage::Brush { layer, strokes } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.brush_modify(strokes);
				}
			}
			GraphOperationMessage::SetUpstreamToChain { layer } => {
				let Some(OutputConnector::Node { node_id: first_chain_node, .. }) = network_interface.upstream_output_connector(&InputConnector::node(layer.to_node(), 1), &[]) else {
					return;
				};

				network_interface.force_set_upstream_to_chain(&first_chain_node, &[]);
			}
			GraphOperationMessage::NewArtboard {
				id,
				location,
				dimensions,
				background,
				clip,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);

				let artboard_layer = modify_inputs.create_artboard(id, location, dimensions, background, clip);
				network_interface.move_layer_to_stack(artboard_layer, LayerNodeIdentifier::ROOT_PARENT, 0, &[]);

				// If there is a non artboard feeding into the primary input of the artboard, move it to the secondary input
				let Some(artboard) = network_interface.document_network().nodes.get(&id) else {
					log::error!("Artboard not created");
					return;
				};
				let document_metadata = network_interface.document_metadata();

				let primary_input = artboard.inputs.first().expect("Artboard should have a primary input").clone();
				if let NodeInput::Node { node_id, .. } = &primary_input {
					if network_interface.is_artboard(node_id, &[]) {
						// Nothing to do here: we have a stack full of artboards!
					} else if network_interface.is_layer(node_id, &[]) {
						// We have a stack of non-layer artboards.
						for (insert_index, layer) in LayerNodeIdentifier::ROOT_PARENT.children(document_metadata).filter(|&layer| layer != artboard_layer).enumerate() {
							// Parent the layer to our new artboard (retaining ordering)
							responses.add(NodeGraphMessage::MoveLayerToStack {
								layer,
								parent: artboard_layer,
								insert_index,
							});
							// Apply a translation to prevent the content from shifting
							responses.add(GraphOperationMessage::TransformChange {
								layer,
								transform: DAffine2::from_translation(-location),
								transform_in: TransformIn::Local,
								skip_rerender: true,
							});
						}

						// Set the bottom input of the artboard back to artboard
						let bottom_input = NodeInput::type_default(descriptor!(List<Artboard>), true);
						network_interface.set_input(&InputConnector::node(artboard_layer.to_node(), 0), bottom_input, &[]);
					} else {
						// We have some non layers (e.g. just a rectangle node). We disconnect the bottom input and connect it to the left input.
						network_interface.disconnect_input(&InputConnector::node(artboard_layer.to_node(), 0), &[]);
						network_interface.set_input(&InputConnector::node(artboard_layer.to_node(), 1), primary_input, &[]);

						// Set the bottom input of the artboard back to artboard
						let bottom_input = NodeInput::type_default(descriptor!(List<Artboard>), true);
						network_interface.set_input(&InputConnector::node(artboard_layer.to_node(), 0), bottom_input, &[]);
					}
				}
				responses.add_front(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewBitmapLayer { id, image, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_image_data(image, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewInterpolationLayer {
				id,
				control_path_id,
				parent,
				insert_index,
				blend_count,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);

				// Insert the main chain node (Blend or Morph) depending on whether a blend count is provided
				let (chain_node_id, layer_alias, path_alias) = if let Some(count) = blend_count {
					(modify_inputs.insert_blend_data(layer, count as f64), "Blend", "Blend Path")
				} else {
					(modify_inputs.insert_morph_data(layer), "Morph", "Morph Path")
				};

				// Create the control path layer (Path → Auto-Tangents → Origins to Polyline)
				let control_path_layer = modify_inputs.create_layer(control_path_id);
				let path_node_id = modify_inputs.insert_control_path_data(control_path_layer);

				network_interface.move_layer_to_stack(control_path_layer, parent, insert_index, &[]);
				network_interface.move_layer_to_stack(layer, parent, insert_index + 1, &[]);

				// Connect the Path node's output to the chain node's path parameter input (input 4 for both Morph and Blend).
				// Done after move_layer_to_stack so chain nodes have correct positions when converted to absolute.
				network_interface.set_input(&InputConnector::node(chain_node_id, 4), NodeInput::node(path_node_id, 0), &[]);

				responses.add(NodeGraphMessage::SetDisplayNameImpl {
					node_id: id,
					network_path: Vec::new(),
					alias: layer_alias.to_string(),
				});
				responses.add(NodeGraphMessage::SetDisplayNameImpl {
					node_id: control_path_id,
					network_path: Vec::new(),
					alias: path_alias.to_string(),
				});
			}
			GraphOperationMessage::ConnectInterpolationControlPathToChildren {
				interpolation_layer_id,
				control_path_id,
			} => {
				// Find the chain node (Blend or Morph, first in chain of the layer)
				let Some(OutputConnector::Node { node_id: chain_node, .. }) = network_interface.upstream_output_connector(&InputConnector::node(interpolation_layer_id, 1), &[]) else {
					log::error!("Could not find chain node for layer {interpolation_layer_id}");
					return;
				};

				// Get what feeds into the chain node's primary input (the children stack)
				let Some(OutputConnector::Node { node_id: children_id, output_index }) = network_interface.upstream_output_connector(&InputConnector::node(chain_node, 0), &[]) else {
					log::error!("Could not find children stack feeding chain node {chain_node}");
					return;
				};

				// Find the deepest node in the control path layer's chain (Origins to Polyline)
				let mut deepest_chain_node = None;
				let mut current_connector = InputConnector::node(control_path_id, 1);
				while let Some(OutputConnector::Node { node_id, .. }) = network_interface.upstream_output_connector(&current_connector, &[]) {
					deepest_chain_node = Some(node_id);
					current_connector = InputConnector::node(node_id, 0);
				}

				// Connect children to the deepest chain node's input 0 (or the layer's input 1 if no chain)
				let target_connector = match deepest_chain_node {
					Some(node_id) => InputConnector::node(node_id, 0),
					None => InputConnector::node(control_path_id, 1),
				};
				network_interface.set_input(&target_connector, NodeInput::node(children_id, output_index), &[]);

				// Shift the child stack (topmost child only, the rest follow) down 3 and left 10
				network_interface.shift_node(&children_id, IVec2::new(-10, 3), &[]);
			}
			GraphOperationMessage::NewBooleanOperationLayer { id, operation, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_boolean_data(operation, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::SetDisplayNameImpl {
					node_id: id,
					network_path: Vec::new(),
					alias: "Boolean Operation".to_string(),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewCustomLayer { id, nodes, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);

				if !nodes.is_empty() {
					// Add the nodes to the network
					let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();
					// Since all the new nodes are already connected, just connect the input of the layer to first new node
					let first_new_node_id = new_ids[&NodeId(0)];
					responses.add(NodeGraphMessage::AddNodes { nodes, new_ids });

					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(layer.to_node(), 1),
						input: NodeInput::node(first_new_node_id, 0),
					});
				}
				// Move the layer and all nodes to the correct position in the network
				responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewColorFillLayer { node_id, color, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(node_id);
				modify_inputs.insert_color_value(color, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_vector(subpaths, layer, true, true, true);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewTextLayer {
				id,
				text,
				font,
				typesetting,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_text(text, font, typesetting, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::ResizeArtboard { layer, location, dimensions } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.resize_artboard(location, dimensions);
				}
			}
			GraphOperationMessage::RemoveArtboards => {
				if network_interface.all_artboards().is_empty() {
					return;
				}

				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: network_interface.all_artboards().iter().map(|layer_node| layer_node.to_node()).collect(),
					delete_children: false,
				});

				let mut artboard_data: HashMap<NodeId, ArtboardInfo> = HashMap::new();

				// Go through all artboards and create merge nodes
				for artboard in network_interface.all_artboards() {
					let node_id = NodeId::new();
					let Some(document_node) = network_interface.document_network().nodes.get(&artboard.to_node()) else {
						log::error!("Artboard not created");
						responses.add(DocumentMessage::AbortTransaction);
						return;
					};

					artboard_data.insert(
						artboard.to_node(),
						ArtboardInfo {
							input_node: NodeInput::node(document_node.inputs[1].as_node().unwrap_or_default(), 0),
							output_nodes: network_interface
								.outward_wires(&[])
								.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(artboard.to_node(), 0)))
								.cloned()
								.unwrap_or_default(),
							merge_node: node_id,
						},
					);

					let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
					modify_inputs.create_layer(node_id);

					responses.add(NodeGraphMessage::SetDisplayName {
						node_id,
						network_path: Vec::new(),
						alias: network_interface.display_name(&artboard.to_node(), &[]),
						skip_adding_history_step: true,
					});

					// Shift node positions in the graph
					let (x, y) = network_interface.position(&artboard.to_node(), &[]).unwrap_or_default().into();
					responses.add(NodeGraphMessage::ShiftNodePosition { node_id, x, y });
				}

				// Go through all artboards and connect them to the merge nodes
				for artboard in &artboard_data {
					// Modify downstream connections
					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(artboard.1.merge_node, 1),
						input: NodeInput::node(artboard.1.input_node.as_node().unwrap_or_default(), 0),
					});

					// Modify upstream connections
					for outward_wire in &artboard.1.output_nodes {
						let input = NodeInput::node(artboard_data[artboard.0].merge_node, 0);
						let input_connector = match artboard_data.get(&outward_wire.node_id().unwrap_or_default()) {
							Some(artboard_info) => InputConnector::node(artboard_info.merge_node, outward_wire.input_index()),
							_ => *outward_wire,
						};
						responses.add(NodeGraphMessage::SetInput { input_connector, input });
					}

					// Apply a transformation to the newly created layers to match the original artboard position
					let offset = network_interface
						.document_metadata()
						.bounding_box_document(LayerNodeIdentifier::new_unchecked(*artboard.0))
						.map(|p| p[0])
						.unwrap_or_default();
					responses.add(GraphOperationMessage::TransformChange {
						layer: LayerNodeIdentifier::new_unchecked(artboard.1.merge_node),
						transform: DAffine2::from_translation(offset),
						transform_in: TransformIn::Local,
						skip_rerender: false,
					});
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
			}

			GraphOperationMessage::NewSvg {
				id,
				svg,
				transform,
				parent,
				insert_index,
				center,
			} => {
				let tree = match usvg::Tree::from_str(&svg, &usvg::Options::default()) {
					Ok(t) => t,
					Err(e) => {
						responses.add(DialogMessage::DisplayDialogError {
							title: "SVG parsing failed".to_string(),
							description: e.to_string(),
						});
						return;
					}
				};
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);

				// The placement transform positions the root group in document space.
				// When centering (paste at cursor/viewport), shift so the SVG is centered at the transform origin.
				// When not centering (file-open flow), content stays at viewport coordinates (usvg's viewBox mapping
				// already places it in [0, width] × [0, height]); the artboard's X/Y handles the viewBox origin offset.
				let mut placement_transform = if center {
					// Center on the actual rendered content bounds rather than the viewbox size.
					// An SVG may have a viewbox larger than its content, so using viewport_size/2 would place the cursor
					// in that empty region instead of on the content.
					let bounds = tree.root().abs_bounding_box();
					let visual_center = DVec2::new((bounds.left() + bounds.right()) as f64 / 2., (bounds.top() + bounds.bottom()) as f64 / 2.);
					transform * DAffine2::from_translation(-visual_center)
				} else {
					transform
				};
				placement_transform.translation = placement_transform.translation.round();

				let graphite_gradient_stops = extract_graphite_gradient_stops(&svg);

				// Pass identity so each leaf layer receives only its SVG-native transform from `abs_transform`.
				// The placement offset is then applied once to the root group layer below.
				import_usvg_node(
					&mut modify_inputs,
					&usvg::Node::Group(Box::new(tree.root().clone())),
					id,
					parent,
					insert_index,
					&graphite_gradient_stops,
				);

				// After import, `layer_node` is set to the root group. Apply the placement transform to it
				// (skipped automatically when identity, so file-open with content at origin creates no Transform node).
				modify_inputs.transform_set(placement_transform, TransformIn::Local, false);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GraphOperationMessage;)
	}
}

#[derive(Debug, Clone)]
struct ArtboardInfo {
	input_node: NodeInput,
	output_nodes: Vec<InputConnector>,
	merge_node: NodeId,
}
