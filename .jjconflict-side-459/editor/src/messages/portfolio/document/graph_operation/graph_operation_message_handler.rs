use super::transform_utils;
use super::utility_types::{ModifyInputsContext, set_stroke_paint_order};
use crate::consts::{LAYER_INDENT_OFFSET, STACK_VERTICAL_GAP};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{BLEND_PATH_INPUT_INDEX, DefinitionIdentifier};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_clip_mode;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::document::{NodeId, NodeInput};
use graph_craft::list;
use graphene_std::renderer::convert_usvg_path::convert_usvg_path;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::style::{Gradient, GradientForm, GradientSettings, GradientSpace, GradientSpread, GradientStop, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_std::{Artboard, Color};

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
			GraphOperationMessage::StrokeOrderSet { layer, paint_order } => {
				let stroke_reference = DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER);
				let Some(stroke_node_id) = ModifyInputsContext::locate_node_in_layer_chain(&stroke_reference, layer, network_interface) else {
					return;
				};
				if set_stroke_paint_order(network_interface, &[], stroke_node_id, paint_order) {
					responses.add(PropertiesPanelMessage::Refresh);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(NodeGraphMessage::SendGraph);
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

fn usvg_color(c: usvg::Color, a: f32) -> Color {
	// `usvg::Color` channels are u8 sRGB display values (gamma-encoded); lift to linear-light for the internal `Color`
	Color::from_gamma_srgb_channels(c.red as f32 / 255., c.green as f32 / 255., c.blue as f32 / 255., a)
}

fn usvg_transform(c: usvg::Transform) -> DAffine2 {
	DAffine2::from_cols_array(&[c.sx as f64, c.ky as f64, c.kx as f64, c.sy as f64, c.tx as f64, c.ty as f64])
}

const GRAPHITE_NAMESPACE: &str = "https://graphite.art";

/// Gradient information pre-parsed from the raw SVG XML, carrying what usvg's simplified tree drops.
struct SvgGradientInfo {
	/// Real stops, keyed by gradient element `id`, for gradients Graphite exported with midpoint curve data.
	graphite_stops: HashMap<String, Gradient>,
	/// Gradient spaces, keyed by gradient element `id`, resolved from the `color-interpolation` property.
	spaces: HashMap<String, GradientSpace>,
}

/// Pre-parses the raw SVG XML to resolve each gradient's inherited `color-interpolation` property, which usvg's
/// tree does not carry. Only `linearRGB` selects the linear space; `auto` and `sRGB` (browsers treat the
/// user-agent-defined `auto` as `sRGB`) mean gamma, as does any unrecognized value.
fn extract_gradient_spaces(svg: &str) -> HashMap<String, GradientSpace> {
	let mut result = HashMap::new();

	// Quick check: gradients in an SVG that never mentions `color-interpolation` all take the sRGB default
	if !svg.contains("color-interpolation") {
		return result;
	}

	let doc = match usvg::roxmltree::Document::parse(svg) {
		Ok(doc) => doc,
		Err(_) => return result,
	};

	// The document's `<style>` blocks apply to every element, so parse them once up front
	let mut stylesheet = simplecss::StyleSheet::new();
	for style_element in doc.descendants().filter(|node| node.tag_name().name() == "style") {
		if !matches!(style_element.attribute("type"), None | Some("") | Some("text/css")) {
			continue;
		}
		for text in style_element.children().filter(|child| child.is_text()).filter_map(|child| child.text()) {
			stylesheet.parse_more(text);
		}
	}

	for node in doc.descendants() {
		match node.tag_name().name() {
			"linearGradient" | "radialGradient" => {}
			_ => continue,
		}

		if let Some(gradient_id) = node.attribute("id")
			&& let Some(gradient_space) = resolve_color_interpolation(node, &stylesheet)
		{
			result.insert(gradient_id.to_string(), gradient_space);
		}
	}

	result
}

/// The `color-interpolation` in effect for an element: the nearest self-or-ancestor declaration, taking each
/// element's own winning declaration per [`declared_color_interpolation`]'s cascade order.
fn resolve_color_interpolation(element: usvg::roxmltree::Node, stylesheet: &simplecss::StyleSheet) -> Option<GradientSpace> {
	let mut next = Some(element);

	while let Some(element) = next {
		match declared_color_interpolation(element, stylesheet) {
			Some("linearRGB") => return Some(GradientSpace::RgbLinear),
			// `inherit` defers to the ancestors like an undeclared element
			Some("inherit") | None => {}
			Some(_) => return Some(GradientSpace::RgbGamma),
		}

		next = element.parent_element();
	}

	None
}

/// The winning `color-interpolation` declaration on a single element per the CSS cascade: `!important` declarations
/// beat normal ones, the inline `style` beats the `<style>` rules (already specificity-sorted, so their last match
/// wins), and the presentation attribute yields to them all. Later declarations win priority ties.
fn declared_color_interpolation<'a>(element: usvg::roxmltree::Node<'a, '_>, stylesheet: &simplecss::StyleSheet<'a>) -> Option<&'a str> {
	let mut winner: Option<(u8, &'a str)> = None;
	let mut consider = |priority: u8, value: &'a str| {
		if winner.is_none_or(|(existing, _)| priority >= existing) {
			winner = Some((priority, value));
		}
	};

	if let Some(value) = element.attribute("color-interpolation") {
		consider(0, value.trim());
	}

	for rule in stylesheet.rules.iter().filter(|rule| rule.selector.matches(&CssElement(element))) {
		for declaration in rule.declarations.iter().filter(|declaration| declaration.name == "color-interpolation") {
			consider(if declaration.important { 3 } else { 1 }, declaration.value);
		}
	}

	if let Some(style) = element.attribute("style") {
		for declaration in simplecss::DeclarationTokenizer::from(style).filter(|declaration| declaration.name == "color-interpolation") {
			consider(if declaration.important { 4 } else { 2 }, declaration.value);
		}
	}

	winner.map(|(_, value)| value)
}

/// Adapts a roxmltree element to simplecss's selector-matching interface.
struct CssElement<'a, 'input>(usvg::roxmltree::Node<'a, 'input>);

impl simplecss::Element for CssElement<'_, '_> {
	fn parent_element(&self) -> Option<Self> {
		self.0.parent_element().map(CssElement)
	}

	fn prev_sibling_element(&self) -> Option<Self> {
		self.0.prev_sibling_element().map(CssElement)
	}

	fn has_local_name(&self, local_name: &str) -> bool {
		self.0.tag_name().name() == local_name
	}

	fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
		self.0.attribute(local_name).is_some_and(|value| operator.matches(value))
	}

	fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
		matches!(class, simplecss::PseudoClass::FirstChild) && self.0.prev_sibling_element().is_none()
	}
}

/// Pre-parses the raw SVG XML to extract gradient stops that have `graphite:midpoint` attributes.
/// Graphite exports gradients with midpoint curve data by writing interpolated approximation stops
/// alongside the real stops. Real stops are tagged with `graphite:midpoint` attributes.
/// Returns a map from gradient element `id` to `Gradient` containing only the real stops.
fn extract_graphite_gradient_stops(svg: &str) -> HashMap<String, Gradient> {
	let mut result = HashMap::new();

	// Quick check: if the SVG doesn't reference `graphite:midpoint` at all, skip parsing
	if !svg.contains("graphite:midpoint") {
		return result;
	}

	let doc = match usvg::roxmltree::Document::parse(svg) {
		Ok(doc) => doc,
		Err(_) => return result,
	};

	for node in doc.descendants() {
		match node.tag_name().name() {
			"linearGradient" | "radialGradient" => {}
			_ => continue,
		}

		let gradient_id = match node.attribute("id") {
			Some(id) => id.to_string(),
			None => continue,
		};

		let mut real_stops = Vec::new();
		let mut has_any_midpoint = false;

		for child in node.children() {
			if child.tag_name().name() != "stop" {
				continue;
			}

			let midpoint = child.attribute((GRAPHITE_NAMESPACE, "midpoint")).and_then(|v| v.parse::<f64>().ok());

			if let Some(midpoint) = midpoint {
				has_any_midpoint = true;

				let offset = child.attribute("offset").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
				let opacity = child.attribute("stop-opacity").and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.);
				let color = child.attribute("stop-color").and_then(|hex| parse_hex_stop_color(hex, opacity)).unwrap_or(Color::BLACK);

				real_stops.push(GradientStop { position: offset, midpoint, color });
			}
		}

		if has_any_midpoint && !real_stops.is_empty() {
			result.insert(gradient_id, Gradient::new(real_stops));
		}
	}

	result
}

fn parse_hex_stop_color(hex: &str, opacity: f32) -> Option<Color> {
	let hex = hex.strip_prefix('#')?;
	if hex.len() != 6 {
		return None;
	}
	let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.;
	let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.;
	let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.;
	Some(Color::from_gamma_srgb_channels(r, g, b, opacity))
}

/// Import a usvg node as the root of an SVG import operation.
///
/// The root layer uses the full `move_layer_to_stack` (with push/collision logic) to correctly
/// interact with any existing layers in the parent stack. All descendant layers use a lightweight
/// O(n) import path that skips collision detection and instead calculates positions directly from
/// the known tree structure.
fn import_usvg_node(modify_inputs: &mut ModifyInputsContext, node: &usvg::Node, id: NodeId, parent: LayerNodeIdentifier, insert_index: usize, gradient_info: &SvgGradientInfo) {
	let layer = modify_inputs.create_layer(id);

	modify_inputs.network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);
	if let Some(upstream_layer) = layer.next_sibling(modify_inputs.network_interface.document_metadata()) {
		modify_inputs.network_interface.shift_node(&upstream_layer.to_node(), IVec2::new(0, STACK_VERTICAL_GAP), &[]);
	}

	match node {
		usvg::Node::Group(group) => {
			// Collect child extents for O(n) position calculation
			let mut child_extents_svg_order: Vec<u32> = Vec::new();
			let mut group_extents_map: HashMap<LayerNodeIdentifier, Vec<u32>> = HashMap::new();

			// Enable import mode: skips expensive is_acyclic checks and per-node cache invalidation
			// during wiring since we're building a known tree structure where cycles are impossible
			modify_inputs.import = true;

			for child in group.children() {
				let extent = import_usvg_node_inner(modify_inputs, child, NodeId::new(), layer, 0, gradient_info, &mut group_extents_map);
				child_extents_svg_order.push(extent);
			}

			modify_inputs.import = false;
			modify_inputs.layer_node = Some(layer);

			// Rebuild the layer tree once now that all wiring is complete
			modify_inputs.network_interface.load_structure();

			// Set positions for all imported descendants in a single O(n) pass
			let parent_pos = modify_inputs.network_interface.position(&layer.to_node(), &[]).unwrap_or(IVec2::ZERO);
			set_import_child_positions(modify_inputs.network_interface, layer, parent_pos, &child_extents_svg_order, &group_extents_map);

			// Invalidate caches once after all positions are set
			modify_inputs.network_interface.unload_all_nodes_click_targets(&[]);
			modify_inputs.network_interface.unload_all_nodes_bounding_box(&[]);
		}
		usvg::Node::Path(path) => {
			import_usvg_path(modify_inputs, node, path, layer, gradient_info);
		}
		usvg::Node::Image(_image) => {
			warn!("Skip image");
		}
		usvg::Node::Text(text) => {
			let font = Font::new(graphene_std::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_std::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks().iter().map(|chunk| chunk.text()).collect(), font, TypesettingConfig::default(), layer);
			modify_inputs.fill_color_set(Some(Color::BLACK));
		}
	}
}

/// Recursively import a usvg node as a descendant of the root import layer.
/// Uses lightweight wiring (no push/collision) and returns the subtree extent for position calculation.
///
/// The subtree extent represents the additional vertical grid units that this node's descendants
/// occupy below the node's position. This is used to calculate correct y_offsets between siblings.
fn import_usvg_node_inner(
	modify_inputs: &mut ModifyInputsContext,
	node: &usvg::Node,
	id: NodeId,
	parent: LayerNodeIdentifier,
	insert_index: usize,
	gradient_info: &SvgGradientInfo,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> u32 {
	let layer = modify_inputs.create_layer(id);
	modify_inputs.network_interface.move_layer_to_stack_for_import(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);

	match node {
		usvg::Node::Group(group) => {
			let mut child_extents: Vec<u32> = Vec::new();
			for child in group.children() {
				let extent = import_usvg_node_inner(modify_inputs, child, NodeId::new(), layer, 0, gradient_info, group_extents_map);
				child_extents.push(extent);
			}
			modify_inputs.layer_node = Some(layer);

			let n = child_extents.len();
			let total_extent = if n == 0 {
				0
			} else {
				(2 * STACK_VERTICAL_GAP as u32) * n as u32 - STACK_VERTICAL_GAP as u32 + child_extents.iter().sum::<u32>()
			};
			group_extents_map.insert(layer, child_extents);
			total_extent
		}
		usvg::Node::Path(path) => {
			import_usvg_path(modify_inputs, node, path, layer, gradient_info);
			0
		}
		usvg::Node::Image(_image) => {
			warn!("Skip image");
			0
		}
		usvg::Node::Text(text) => {
			let font = Font::new(graphene_std::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_std::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks().iter().map(|chunk| chunk.text()).collect(), font, TypesettingConfig::default(), layer);
			modify_inputs.fill_color_set(Some(Color::BLACK));
			0
		}
	}
}

/// Helper to apply path data (vector geometry, fill, stroke, transform) to a layer.
fn import_usvg_path(modify_inputs: &mut ModifyInputsContext, node: &usvg::Node, path: &usvg::Path, layer: LayerNodeIdentifier, gradient_info: &SvgGradientInfo) {
	let subpaths = convert_usvg_path(path);

	// Skip creating a Transform node entirely when the SVG-native transform is identity.
	let node_transform = usvg_transform(node.abs_transform());
	let has_transform = node_transform != DAffine2::IDENTITY;

	modify_inputs.insert_vector(subpaths, layer, has_transform, path.fill().is_some(), path.stroke().is_some());

	if has_transform && let Some(transform_node_id) = modify_inputs.existing_proto_node_id(graphene_std::transform_nodes::transform::IDENTIFIER, false) {
		transform_utils::update_transform(modify_inputs.network_interface, &transform_node_id, node_transform);
	}

	if let Some(fill) = path.fill() {
		apply_usvg_fill(fill, modify_inputs, gradient_info);
	}
	if let Some(stroke) = path.stroke() {
		apply_usvg_stroke(stroke, modify_inputs, node_transform);
	}
}

/// Set correct positions for all imported layers in a single top-down O(n) pass.
///
/// For each group's child stack:
/// - The top-of-stack child (last SVG child) gets an `Absolute` position at `(parent_x - LAYER_INDENT_OFFSET, parent_y + STACK_VERTICAL_GAP)`
/// - All other children get `Stack(y_offset)` where `y_offset` accounts for the subtree extent of the sibling above them in the stack, ensuring no overlap.
fn set_import_child_positions(
	network_interface: &mut NodeNetworkInterface,
	group: LayerNodeIdentifier,
	group_pos: IVec2,
	child_extents_svg_order: &[u32],
	group_extents_map: &HashMap<LayerNodeIdentifier, Vec<u32>>,
) {
	use crate::messages::portfolio::document::utility_types::network_interface::LayerPosition;

	let layer_children: Vec<_> = group.children(network_interface.document_metadata()).collect();
	let n = child_extents_svg_order.len();

	if n == 0 || layer_children.is_empty() {
		return;
	}

	// Children in the layer tree are in stack order (top to bottom), which is the REVERSE of SVG order.
	// SVG order:   [s_0,     s_1,     ..., s_{n-1}] with extents [e_0, e_1, ..., e_{n-1}]
	// Stack order: [s_{n-1}, s_{n-2}, ..., s_0    ] (top to bottom)
	//
	// For stack child at index i:
	//   - SVG index = n - 1 - i
	//   - Previous stack sibling's SVG index = n - i
	//   - y_offset = extent_of_previous_sibling + STACK_VERTICAL_GAP

	let child_x = group_pos.x - LAYER_INDENT_OFFSET;
	let mut current_y = group_pos.y + STACK_VERTICAL_GAP;

	for (i, child_layer) in layer_children.iter().enumerate() {
		let child_pos = IVec2::new(child_x, current_y);

		if i == 0 {
			// Top of stack: set to `Absolute` position
			network_interface.set_layer_position_for_import(&child_layer.to_node(), LayerPosition::Absolute(child_pos), &[]);
		} else {
			// Below top: set `Stack` with `y_offset` based on previous sibling's subtree extent
			let prev_sibling_svg_index = n - i;
			let y_offset = child_extents_svg_order[prev_sibling_svg_index] + STACK_VERTICAL_GAP as u32;
			network_interface.set_layer_position_for_import(&child_layer.to_node(), LayerPosition::Stack(y_offset), &[]);
		}

		// Recurse into group children to set their descendants' positions
		if let Some(grandchild_extents) = group_extents_map.get(child_layer) {
			set_import_child_positions(network_interface, *child_layer, child_pos, grandchild_extents, group_extents_map);
		}

		// Advance `current_y` for the next child: node height (STACK_VERTICAL_GAP) + gap (STACK_VERTICAL_GAP) + subtree extent
		let child_svg_index = n - 1 - i;
		let child_extent = child_extents_svg_order[child_svg_index];
		current_y += 2 * STACK_VERTICAL_GAP + child_extent as i32;
	}
}

fn apply_usvg_stroke(stroke: &usvg::Stroke, modify_inputs: &mut ModifyInputsContext, transform: DAffine2) {
	if let usvg::Paint::Color(color) = &stroke.paint() {
		modify_inputs.stroke_set(
			Some(usvg_color(*color, stroke.opacity().get())),
			Stroke {
				weight: stroke.width().get() as f64,
				dash_lengths: stroke.dasharray().as_ref().map(|lengths| lengths.iter().map(|&length| length as f64).collect()).unwrap_or_default(),
				dash_offset: stroke.dashoffset() as f64,
				cap: match stroke.linecap() {
					usvg::LineCap::Butt => StrokeCap::Butt,
					usvg::LineCap::Round => StrokeCap::Round,
					usvg::LineCap::Square => StrokeCap::Square,
				},
				join: match stroke.linejoin() {
					usvg::LineJoin::Miter => StrokeJoin::Miter,
					usvg::LineJoin::MiterClip => StrokeJoin::Miter,
					usvg::LineJoin::Round => StrokeJoin::Round,
					usvg::LineJoin::Bevel => StrokeJoin::Bevel,
				},
				join_miter_limit: stroke.miterlimit().get() as f64,
				align: StrokeAlign::Center,
				transform,
			},
		)
	}
}

fn convert_gradient_spread(spread_method: usvg::SpreadMethod) -> GradientSpread {
	match spread_method {
		usvg::SpreadMethod::Pad => GradientSpread::Pad,
		usvg::SpreadMethod::Reflect => GradientSpread::Reflect,
		usvg::SpreadMethod::Repeat => GradientSpread::Repeat,
	}
}

fn apply_usvg_fill(fill: &usvg::Fill, modify_inputs: &mut ModifyInputsContext, gradient_info: &SvgGradientInfo) {
	match &fill.paint() {
		usvg::Paint::Color(color) => modify_inputs.fill_color_set(Some(usvg_color(*color, fill.opacity().get()))),
		usvg::Paint::LinearGradient(linear) => {
			let gradient_transform = usvg_transform(linear.transform());
			let (start, end) = (DVec2::new(linear.x1() as f64, linear.y1() as f64), DVec2::new(linear.x2() as f64, linear.y2() as f64));
			let (start, end) = (gradient_transform.transform_point2(start), gradient_transform.transform_point2(end));
			let direction = end - start;
			let transform = DAffine2::from_cols(direction, direction.perp(), start);

			let gradient_form = GradientForm::Linear;

			let gradient = match gradient_info.graphite_stops.get(linear.id()) {
				Some(graphite_stops) => graphite_stops.clone(),
				None => {
					let stops = linear.stops().iter().map(|stop| GradientStop {
						position: stop.offset().get() as f64,
						midpoint: 0.5,
						color: usvg_color(stop.color(), stop.opacity().get()),
					});
					Gradient::new(stops)
				}
			};
			// SVG interpolates between stops in gamma sRGB unless `color-interpolation` opts into linearRGB, carried explicitly rather than as the linear default
			let settings = GradientSettings {
				spread: convert_gradient_spread(linear.spread_method()),
				space: gradient_info.spaces.get(linear.id()).copied().unwrap_or(GradientSpace::RgbGamma),
				..Default::default()
			};
			modify_inputs.fill_gradient_set(gradient, gradient_form, settings, transform);
		}
		usvg::Paint::RadialGradient(radial) => {
			let gradient_transform = usvg_transform(radial.transform());
			let center = DVec2::new(radial.cx() as f64, radial.cy() as f64);
			let edge = center + DVec2::X * radial.r().get() as f64;
			let (start, end) = (gradient_transform.transform_point2(center), gradient_transform.transform_point2(edge));
			let direction = end - start;
			let transform = DAffine2::from_cols(direction, direction.perp(), start);

			let gradient_form = GradientForm::Radial;

			let gradient = match gradient_info.graphite_stops.get(radial.id()) {
				Some(graphite_stops) => graphite_stops.clone(),
				None => {
					let stops = radial.stops().iter().map(|stop| GradientStop {
						position: stop.offset().get() as f64,
						midpoint: 0.5,
						color: usvg_color(stop.color(), stop.opacity().get()),
					});
					Gradient::new(stops)
				}
			};
			let settings = GradientSettings {
				spread: convert_gradient_spread(radial.spread_method()),
				space: gradient_info.spaces.get(radial.id()).copied().unwrap_or(GradientSpace::RgbGamma),
				..Default::default()
			};
			modify_inputs.fill_gradient_set(gradient, gradient_form, settings, transform);
		}
		usvg::Paint::Pattern(_) => warn!("SVG patterns are not currently supported"),
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn stroke_order_set_reorders_the_fill_and_stroke_nodes() {
		use crate::messages::tool::common_functionality::graph_modification_utils::get_stroke_paint_order;
		use crate::test_utils::test_prelude::*;
		use graphene_std::vector::style::PaintOrder;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();
		let paint_order = get_stroke_paint_order(layer, &document.network_interface);
		assert_eq!(paint_order, PaintOrder::StrokeAbove, "a fresh shape should stroke above its fill");

		editor
			.handle_message(GraphOperationMessage::StrokeOrderSet {
				layer,
				paint_order: PaintOrder::StrokeBelow,
			})
			.await;
		let paint_order = get_stroke_paint_order(layer, &editor.active_document().network_interface);
		assert_eq!(paint_order, PaintOrder::StrokeBelow, "the rewrite should move the stroke downstream of the fill");

		editor
			.handle_message(GraphOperationMessage::StrokeOrderSet {
				layer,
				paint_order: PaintOrder::StrokeAbove,
			})
			.await;
		let paint_order = get_stroke_paint_order(layer, &editor.active_document().network_interface);
		assert_eq!(paint_order, PaintOrder::StrokeAbove, "the rewrite should move the stroke back upstream of the fill");
	}

	#[tokio::test]
	async fn a_node_inserted_between_the_fill_and_stroke_makes_the_order_swap_inapplicable() {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
		use crate::messages::tool::common_functionality::graph_modification_utils::stroke_paint_order_applicable;
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		assert!(
			stroke_paint_order_applicable(layer, &editor.active_document().network_interface),
			"a fresh shape's adjacent Fill and Stroke pair should be swappable"
		);

		let stroke_reference = DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER);
		let stroke_node_id = ModifyInputsContext::locate_node_in_layer_chain(&stroke_reference, layer, &editor.active_document().network_interface).unwrap();

		let node_template = Box::new(resolve_proto_node_type(graphene_std::ops::passthrough::IDENTIFIER).unwrap().default_node_template());
		let passthrough_node_id = NodeId::new();
		editor
			.handle_message(NodeGraphMessage::InsertNode {
				node_id: passthrough_node_id,
				node_template,
			})
			.await;
		editor
			.handle_message(NodeGraphMessage::InsertNodeBetween {
				node_id: passthrough_node_id,
				input_connector: InputConnector::node_at_index(stroke_node_id, 0),
				insert_node_input_index: 0,
			})
			.await;

		assert!(
			!stroke_paint_order_applicable(layer, &editor.active_document().network_interface),
			"a node between the pair should gray the order radio out"
		);
	}

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
