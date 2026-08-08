use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::graph_operation::utility_types::{TransformIn, import_usvg_node};
use crate::messages::portfolio::document::node_graph::document_node_definitions::BLEND_PATH_INPUT_INDEX;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_clip_mode;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::document::{NodeId, NodeInput};
use graph_craft::list;
use graphene_std::Artboard;
use graphene_std::renderer::usvg_utils::{SvgGradientInfo, extract_gradient_spaces, extract_graphite_gradient_stops};

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
			GraphOperationMessage::ColorValueSet { layer, color } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.color_value_set(color);
				}
			}
			GraphOperationMessage::FillGradientSet {
				layer,
				gradient,
				gradient_form,
				gradient_settings,
				transform,
			} => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.fill_gradient_set(gradient, gradient_form, gradient_settings, transform);
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
			GraphOperationMessage::GradientPositionsSet { layer, positions } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_positions_set(positions);
				}
			}
			GraphOperationMessage::GradientMidpointsSet { layer, midpoints } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_midpoints_set(midpoints);
				}
			}
			GraphOperationMessage::GradientTransformSet { layer, transform } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_transform_set(transform);
				}
			}
			GraphOperationMessage::GradientFormSet { layer, gradient_form } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_form_set(gradient_form);
				}
			}
			GraphOperationMessage::GradientSpreadSet { layer, gradient_spread } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_spread_set(gradient_spread);
				}
			}
			GraphOperationMessage::GradientSpaceSet { layer, gradient_space } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_space_set(gradient_space);
				}
			}
			GraphOperationMessage::GradientCyclicSet { layer, gradient_cyclic } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_cyclic_set(gradient_cyclic);
				}
			}
			GraphOperationMessage::GradientHueDirectionSet { layer, gradient_hue_direction } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_hue_direction_set(gradient_hue_direction);
				}
			}
			GraphOperationMessage::GradientInterpolationSet { layer, gradient_interpolation } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.gradient_interpolation_set(gradient_interpolation);
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
				let Some(OutputConnector::Node { node_id: first_chain_node, .. }) = network_interface.upstream_output_connector(&InputConnector::layer_secondary_input(layer.to_node()), &[]) else {
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
						let bottom_input = NodeInput::type_default(list!(Artboard), true);
						network_interface.set_input(&InputConnector::primary_input(artboard_layer.to_node()), bottom_input, &[]);
					} else {
						// We have some non layers (e.g. just a rectangle node). We disconnect the bottom input and connect it to the left input.
						network_interface.disconnect_input(&InputConnector::primary_input(artboard_layer.to_node()), &[]);
						network_interface.set_input(&InputConnector::layer_secondary_input(artboard_layer.to_node()), primary_input, &[]);

						// Set the bottom input of the artboard back to artboard
						let bottom_input = NodeInput::type_default(list!(Artboard), true);
						network_interface.set_input(&InputConnector::primary_input(artboard_layer.to_node()), bottom_input, &[]);
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

				// Insert the main chain node (Blend or Morph) depending on whether a blend count is provided, referencing
				// its control path input by the Blend template's named position or the Morph proto node's parameter symbol
				let (path_input_connector, layer_alias, path_alias) = if let Some(count) = blend_count {
					let blend_node_id = modify_inputs.insert_blend_data(layer, count as f64);
					(InputConnector::node_at_index(blend_node_id, BLEND_PATH_INPUT_INDEX), "Blend", "Blend Path")
				} else {
					let morph_node_id = modify_inputs.insert_morph_data(layer);
					(InputConnector::node(morph_node_id, graphene_std::vector::morph::PathInput), "Morph", "Morph Path")
				};

				// Create the control path layer (Path → Auto-Tangents → Origins to Polyline)
				let control_path_layer = modify_inputs.create_layer(control_path_id);
				let path_node_id = modify_inputs.insert_control_path_data(control_path_layer);

				network_interface.move_layer_to_stack(control_path_layer, parent, insert_index, &[]);
				network_interface.move_layer_to_stack(layer, parent, insert_index + 1, &[]);

				// Connect the Path node's output to the chain node's control path input.
				// Done after move_layer_to_stack so chain nodes have correct positions when converted to absolute.
				network_interface.set_input(&path_input_connector, NodeInput::node(path_node_id, 0), &[]);

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
				let Some(OutputConnector::Node { node_id: chain_node, .. }) = network_interface.upstream_output_connector(&InputConnector::layer_secondary_input(interpolation_layer_id), &[]) else {
					log::error!("Could not find chain node for layer {interpolation_layer_id}");
					return;
				};

				// Get what feeds into the chain node's primary input (the children stack)
				let Some(OutputConnector::Node { node_id: children_id, output_index }) = network_interface.upstream_output_connector(&InputConnector::primary_input(chain_node), &[]) else {
					log::error!("Could not find children stack feeding chain node {chain_node}");
					return;
				};

				// Find the deepest node in the control path layer's chain (Origins to Polyline)
				let mut deepest_chain_node = None;
				let mut current_connector = InputConnector::layer_secondary_input(control_path_id);
				while let Some(OutputConnector::Node { node_id, .. }) = network_interface.upstream_output_connector(&current_connector, &[]) {
					deepest_chain_node = Some(node_id);
					current_connector = InputConnector::primary_input(node_id);
				}

				// Connect children to the deepest chain node's input 0 (or the layer's input 1 if no chain)
				let target_connector = match deepest_chain_node {
					Some(node_id) => InputConnector::primary_input(node_id),
					None => InputConnector::layer_secondary_input(control_path_id),
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
						input_connector: InputConnector::layer_secondary_input(layer.to_node()),
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
				modify_inputs.insert_color_value(color, layer, InputConnector::layer_secondary_input(layer.to_node()));
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
								.and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(artboard.to_node())))
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
						input_connector: InputConnector::layer_secondary_input(artboard.1.merge_node),
						input: NodeInput::node(artboard.1.input_node.as_node().unwrap_or_default(), 0),
					});

					// Modify upstream connections
					for outward_wire in &artboard.1.output_nodes {
						let input = NodeInput::node(artboard_data[artboard.0].merge_node, 0);
						let input_connector = match artboard_data.get(&outward_wire.node_id().unwrap_or_default()) {
							Some(artboard_info) => InputConnector::node_at_index(artboard_info.merge_node, outward_wire.input_index()),
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

				let gradient_info = SvgGradientInfo {
					graphite_stops: extract_graphite_gradient_stops(&svg),
					spaces: extract_gradient_spaces(&svg),
				};

				// Pass identity so each leaf layer receives only its SVG-native transform from `abs_transform`.
				// The placement offset is then applied once to the root group layer below.
				import_usvg_node(&mut modify_inputs, &usvg::Node::Group(Box::new(tree.root().clone())), id, parent, insert_index, &gradient_info);

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn color_interpolation_resolves_per_gradient_with_inheritance_and_style_priority() {
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" color-interpolation="linearRGB">
			<defs>
				<linearGradient id="inherited"/>
				<linearGradient id="attribute" color-interpolation="sRGB"/>
				<linearGradient id="styled" color-interpolation="sRGB" style="fill: red; color-interpolation: linearRGB"/>
				<radialGradient id="auto" color-interpolation="auto"/>
			</defs>
		</svg>"##;

		let spaces = extract_gradient_spaces(svg);
		assert_eq!(spaces.get("inherited"), Some(&GradientSpace::RgbLinear), "an undeclared gradient should inherit from its ancestors");
		assert_eq!(spaces.get("attribute"), Some(&GradientSpace::RgbGamma), "an sRGB declaration should beat the inherited linearRGB");
		assert_eq!(spaces.get("styled"), Some(&GradientSpace::RgbLinear), "the inline style should beat the presentation attribute");
		assert_eq!(
			spaces.get("auto"),
			Some(&GradientSpace::RgbGamma),
			"auto should mean gamma like browsers treat it, not defer to ancestors"
		);
	}

	#[test]
	fn graphite_stop_extraction_keeps_real_stops_and_linearizes_their_colors() {
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art">
			<defs>
				<linearGradient id="ramp">
					<stop stop-color="#000000" graphite:midpoint="0.3" />
					<stop offset="0.25" stop-color="#404040" />
					<stop offset="0.5" stop-color="#808080" stop-opacity="0.5" graphite:midpoint="0.5" />
					<stop offset="1" stop-color="#ffffff" graphite:midpoint="0.5" />
				</linearGradient>
			</defs>
		</svg>"##;

		let stops = extract_graphite_gradient_stops(svg);
		let gradient = stops.get("ramp").expect("the tagged gradient should be recovered");

		// The untagged stop is baked approximation residue, not authored data
		assert_eq!(gradient.len(), 3, "only stops tagged with a midpoint should survive");
		assert_eq!(gradient.positions(false), vec![0., 0.5, 1.]);
		assert_eq!(gradient.midpoints(), vec![0.3, 0.5, 0.5]);

		// Hex stop bytes are gamma-encoded, so the recovered color must lift them to linear light
		assert_eq!(gradient.color(1), Some(Color::from_gamma_srgb_channels(128. / 255., 128. / 255., 128. / 255., 0.5)));
	}

	#[test]
	fn color_interpolation_reads_style_blocks_with_selector_specificity() {
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
			<style>
				linearGradient { color-interpolation: linearRGB }
				.classy { color-interpolation: sRGB }
				#exact { color-interpolation: linearRGB }
			</style>
			<defs>
				<linearGradient id="from-type-rule"/>
				<linearGradient id="from-class-rule" class="classy"/>
				<linearGradient id="exact" class="classy"/>
				<linearGradient id="inline-beats-rules" class="classy" style="color-interpolation: linearRGB"/>
				<linearGradient id="rule-beats-attribute" class="classy" color-interpolation="linearRGB"/>
			</defs>
		</svg>"##;

		let spaces = extract_gradient_spaces(svg);
		assert_eq!(spaces.get("from-type-rule"), Some(&GradientSpace::RgbLinear), "a type rule in a style block should reach the gradient");
		assert_eq!(
			spaces.get("from-class-rule"),
			Some(&GradientSpace::RgbGamma),
			"the class rule should outrank the type rule by specificity"
		);
		assert_eq!(spaces.get("exact"), Some(&GradientSpace::RgbLinear), "the ID rule should outrank the class rule");
		assert_eq!(spaces.get("inline-beats-rules"), Some(&GradientSpace::RgbLinear), "the inline style should beat every style block rule");
		assert_eq!(
			spaces.get("rule-beats-attribute"),
			Some(&GradientSpace::RgbGamma),
			"a style block rule should beat the presentation attribute"
		);
	}

	#[test]
	fn repeated_and_important_declarations_resolve_by_cascade_order() {
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
			<style>.forced { color-interpolation: linearRGB !important }</style>
			<linearGradient id="last-declaration-wins" style="color-interpolation: sRGB; color-interpolation: linearRGB"/>
			<linearGradient id="important-beats-later" style="color-interpolation: linearRGB !important; color-interpolation: sRGB"/>
			<linearGradient id="important-rule-beats-inline" class="forced" style="color-interpolation: sRGB"/>
		</svg>"##;

		let spaces = extract_gradient_spaces(svg);
		assert_eq!(
			spaces.get("last-declaration-wins"),
			Some(&GradientSpace::RgbLinear),
			"the last of repeated inline declarations should win"
		);
		assert_eq!(
			spaces.get("important-beats-later"),
			Some(&GradientSpace::RgbLinear),
			"an `!important` declaration should beat a later normal one"
		);
		assert_eq!(
			spaces.get("important-rule-beats-inline"),
			Some(&GradientSpace::RgbLinear),
			"an `!important` style block rule should beat the inline style"
		);
	}

	#[test]
	fn color_interpolation_yields_nothing_when_never_declared() {
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><linearGradient id="plain"/></svg>"##;

		assert!(
			extract_gradient_spaces(svg).is_empty(),
			"gradients without any declaration should fall back to the caller's gamma default"
		);
	}
}
