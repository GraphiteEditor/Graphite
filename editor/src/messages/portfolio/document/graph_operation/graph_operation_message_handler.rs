use super::transform_utils;
use super::utility_types::ModifyInputsContext;
use crate::consts::{LAYER_INDENT_OFFSET, STACK_VERTICAL_GAP};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_clip_mode;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::descriptor;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::list::List;
use graphene_std::renderer::convert_usvg_path::convert_usvg_path;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::style::{GradientSpreadMethod, GradientStop, GradientStops, GradientType, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_std::{Artboard, Color, MaskMode};

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

fn usvg_color(c: usvg::Color, a: f32) -> Color {
	// `usvg::Color` channels are u8 sRGB display values (gamma-encoded); lift to linear-light for the internal `Color`
	Color::from_gamma_srgb_channels(c.red as f32 / 255., c.green as f32 / 255., c.blue as f32 / 255., a)
}

fn usvg_transform(c: usvg::Transform) -> DAffine2 {
	DAffine2::from_cols_array(&[c.sx as f64, c.ky as f64, c.kx as f64, c.sy as f64, c.tx as f64, c.ty as f64])
}

const GRAPHITE_NAMESPACE: &str = "https://graphite.art";

/// Pre-parses the raw SVG XML to extract gradient stops that have `graphite:midpoint` attributes.
/// Graphite exports gradients with midpoint curve data by writing interpolated approximation stops
/// alongside the real stops. Real stops are tagged with `graphite:midpoint` attributes.
/// Returns a map from gradient element `id` to `GradientStops` containing only the real stops.
fn extract_graphite_gradient_stops(svg: &str) -> HashMap<String, GradientStops> {
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
			result.insert(gradient_id, GradientStops::new(real_stops));
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
	Some(Color::from_rgbaf32_unchecked(r, g, b, opacity))
}

#[derive(Clone, Copy, Debug)]
struct ImportedNode {
	layer: LayerNodeIdentifier,
	extent: u32,
}

fn imported_group_extent(child_extents: &[u32]) -> u32 {
	if child_extents.is_empty() {
		0
	} else {
		(2 * STACK_VERTICAL_GAP as u32) * child_extents.len() as u32 - STACK_VERTICAL_GAP as u32 + child_extents.iter().sum::<u32>()
	}
}

/// Import a usvg node as the root of an SVG import operation.
///
/// The root layer uses the full `move_layer_to_stack` (with push/collision logic) to correctly
/// interact with any existing layers in the parent stack. All descendant layers use a lightweight
/// O(n) import path that skips collision detection and instead calculates positions directly from
/// the known tree structure.
fn import_usvg_node(
	modify_inputs: &mut ModifyInputsContext,
	node: &usvg::Node,
	id: NodeId,
	parent: LayerNodeIdentifier,
	insert_index: usize,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
) {
	let layer = modify_inputs.create_layer(id);

	modify_inputs.network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);
	if let Some(upstream_layer) = layer.next_sibling(modify_inputs.network_interface.document_metadata()) {
		modify_inputs.network_interface.shift_node(&upstream_layer.to_node(), IVec2::new(0, STACK_VERTICAL_GAP), &[]);
	}

	match node {
		usvg::Node::Group(group) => {
			let mut group_extents_map: HashMap<LayerNodeIdentifier, Vec<u32>> = HashMap::new();

			// Enable import mode: skips expensive is_acyclic checks and per-node cache invalidation
			// during wiring since we're building a known tree structure where cycles are impossible
			modify_inputs.import = true;

			let child_extents_svg_order = import_usvg_group_children(modify_inputs, group, layer, graphite_gradient_stops, &mut group_extents_map);

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
			import_usvg_path(modify_inputs, node, path, layer, graphite_gradient_stops);
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

/// Imports a group's children in SVG paint order, adding a hidden mask base for a resolved SVG mask.
fn import_usvg_group_children(
	modify_inputs: &mut ModifyInputsContext,
	group: &usvg::Group,
	parent: LayerNodeIdentifier,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> Vec<u32> {
	import_usvg_children_with_mask(
		modify_inputs,
		group.children(),
		group.mask(),
		parent,
		usvg_transform(group.abs_transform()),
		graphite_gradient_stops,
		group_extents_map,
	)
}

/// Imports one sibling list and represents its optional SVG mask with Graphite's existing clipping-mask nodes.
fn import_usvg_children_with_mask(
	modify_inputs: &mut ModifyInputsContext,
	children: &[usvg::Node],
	mask: Option<&usvg::Mask>,
	parent: LayerNodeIdentifier,
	referencing_transform: DAffine2,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> Vec<u32> {
	let Some(mask) = mask.filter(|_| !children.is_empty()) else {
		return import_usvg_children(modify_inputs, children, parent, graphite_gradient_stops, group_extents_map);
	};

	let imported_mask = import_usvg_mask(modify_inputs, mask, parent, 0, referencing_transform, graphite_gradient_stops, group_extents_map);
	let imported_content = import_usvg_children_as_group(modify_inputs, children, parent, 0, graphite_gradient_stops, group_extents_map);

	modify_inputs.layer_node = Some(imported_content.layer);
	modify_inputs.clip_mode_set(true);

	vec![imported_mask.extent, imported_content.extent]
}

/// Imports a sibling list directly into an existing Graphite group.
fn import_usvg_children(
	modify_inputs: &mut ModifyInputsContext,
	children: &[usvg::Node],
	parent: LayerNodeIdentifier,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> Vec<u32> {
	children
		.iter()
		.map(|child| import_usvg_node_inner(modify_inputs, child, NodeId::new(), parent, 0, graphite_gradient_stops, group_extents_map).extent)
		.collect()
}

/// Imports a sibling list as one composited Graphite group so a parent mask can be applied exactly once.
fn import_usvg_children_as_group(
	modify_inputs: &mut ModifyInputsContext,
	children: &[usvg::Node],
	parent: LayerNodeIdentifier,
	insert_index: usize,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> ImportedNode {
	let layer = modify_inputs.create_layer(NodeId::new());
	modify_inputs.network_interface.move_layer_to_stack_for_import(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);

	let child_extents = import_usvg_children(modify_inputs, children, layer, graphite_gradient_stops, group_extents_map);
	let extent = imported_group_extent(&child_extents);
	group_extents_map.insert(layer, child_extents);

	ImportedNode { layer, extent }
}

/// Imports a resolved SVG mask as one hidden Graphite mask-base layer.
fn import_usvg_mask(
	modify_inputs: &mut ModifyInputsContext,
	mask: &usvg::Mask,
	parent: LayerNodeIdentifier,
	insert_index: usize,
	referencing_transform: DAffine2,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> ImportedNode {
	let layer = modify_inputs.create_layer(NodeId::new());
	modify_inputs.network_interface.move_layer_to_stack_for_import(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);

	let child_extents = import_usvg_children_with_mask(
		modify_inputs,
		mask.root().children(),
		mask.mask(),
		layer,
		DAffine2::IDENTITY,
		graphite_gradient_stops,
		group_extents_map,
	);

	let extent = imported_group_extent(&child_extents);
	group_extents_map.insert(layer, child_extents);

	modify_inputs.layer_node = Some(layer);
	modify_inputs.transform_set(referencing_transform, TransformIn::Local, false);
	let mask_mode = match mask.kind() {
		usvg::MaskType::Alpha => MaskMode::Alpha,
		usvg::MaskType::Luminance => MaskMode::Luminance,
	};
	modify_inputs.mask_mode_set(mask_mode);
	// Independent fill opacity hides the mask artwork during normal rendering but is
	// intentionally ignored by the renderer's mask pass, preserving its source paint and alpha.
	modify_inputs.opacity_fill_set(0.);

	ImportedNode { layer, extent }
}

/// Recursively import a usvg node as a descendant of the root import layer.
/// Uses lightweight wiring (no push/collision) and returns the layer plus its subtree extent for position calculation.
///
/// The subtree extent represents the additional vertical grid units that this node's descendants
/// occupy below the node's position. This is used to calculate correct y_offsets between siblings.
fn import_usvg_node_inner(
	modify_inputs: &mut ModifyInputsContext,
	node: &usvg::Node,
	id: NodeId,
	parent: LayerNodeIdentifier,
	insert_index: usize,
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> ImportedNode {
	let layer = modify_inputs.create_layer(id);
	modify_inputs.network_interface.move_layer_to_stack_for_import(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);

	let extent = match node {
		usvg::Node::Group(group) => {
			let child_extents = import_usvg_group_children(modify_inputs, group, layer, graphite_gradient_stops, group_extents_map);
			modify_inputs.layer_node = Some(layer);

			let total_extent = imported_group_extent(&child_extents);
			group_extents_map.insert(layer, child_extents);
			total_extent
		}
		usvg::Node::Path(path) => {
			import_usvg_path(modify_inputs, node, path, layer, graphite_gradient_stops);
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
	};

	ImportedNode { layer, extent }
}

/// Helper to apply path data (vector geometry, fill, stroke, transform) to a layer.
fn import_usvg_path(modify_inputs: &mut ModifyInputsContext, node: &usvg::Node, path: &usvg::Path, layer: LayerNodeIdentifier, graphite_gradient_stops: &HashMap<String, GradientStops>) {
	let subpaths = convert_usvg_path(path);

	// Skip creating a Transform node entirely when the SVG-native transform is identity.
	let node_transform = usvg_transform(node.abs_transform());
	let has_transform = node_transform != DAffine2::IDENTITY;

	modify_inputs.insert_vector(subpaths, layer, has_transform, path.fill().is_some(), path.stroke().is_some());

	if has_transform && let Some(transform_node_id) = modify_inputs.existing_proto_node_id(graphene_std::transform_nodes::transform::IDENTIFIER, false) {
		transform_utils::update_transform(modify_inputs.network_interface, &transform_node_id, node_transform);
	}

	if let Some(fill) = path.fill() {
		apply_usvg_fill(fill, modify_inputs, graphite_gradient_stops);
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
				paint_order: PaintOrder::StrokeAbove,
				transform,
			},
		)
	}
}

fn convert_spread_method(spread_method: usvg::SpreadMethod) -> GradientSpreadMethod {
	match spread_method {
		usvg::SpreadMethod::Pad => GradientSpreadMethod::Pad,
		usvg::SpreadMethod::Reflect => GradientSpreadMethod::Reflect,
		usvg::SpreadMethod::Repeat => GradientSpreadMethod::Repeat,
	}
}

fn apply_usvg_fill(fill: &usvg::Fill, modify_inputs: &mut ModifyInputsContext, graphite_gradient_stops: &HashMap<String, GradientStops>) {
	match &fill.paint() {
		usvg::Paint::Color(color) => modify_inputs.fill_color_set(Some(usvg_color(*color, fill.opacity().get()))),
		usvg::Paint::LinearGradient(linear) => {
			let gradient_transform = usvg_transform(linear.transform());
			let (start, end) = (DVec2::new(linear.x1() as f64, linear.y1() as f64), DVec2::new(linear.x2() as f64, linear.y2() as f64));
			let (start, end) = (gradient_transform.transform_point2(start), gradient_transform.transform_point2(end));
			let direction = end - start;
			let transform = DAffine2::from_cols(direction, direction.perp(), start);

			let gradient_type = GradientType::Linear;

			let gradient = match graphite_gradient_stops.get(linear.id()) {
				Some(graphite_stops) => graphite_stops.clone(),
				None => {
					let stops = linear.stops().iter().map(|stop| GradientStop {
						position: stop.offset().get() as f64,
						midpoint: 0.5,
						color: usvg_color(stop.color(), stop.opacity().get()),
					});
					GradientStops::new(stops)
				}
			};
			let spread_method = convert_spread_method(linear.spread_method());
			modify_inputs.fill_gradient_set(gradient, gradient_type, spread_method, transform);
		}
		usvg::Paint::RadialGradient(radial) => {
			let gradient_transform = usvg_transform(radial.transform());
			let center = DVec2::new(radial.cx() as f64, radial.cy() as f64);
			let edge = center + DVec2::X * radial.r().get() as f64;
			let (start, end) = (gradient_transform.transform_point2(center), gradient_transform.transform_point2(edge));
			let direction = end - start;
			let transform = DAffine2::from_cols(direction, direction.perp(), start);

			let gradient_type = GradientType::Radial;

			let gradient = match graphite_gradient_stops.get(radial.id()) {
				Some(graphite_stops) => graphite_stops.clone(),
				None => {
					let stops = radial.stops().iter().map(|stop| GradientStop {
						position: stop.offset().get() as f64,
						midpoint: 0.5,
						color: usvg_color(stop.color(), stop.opacity().get()),
					});
					GradientStops::new(stops)
				}
			};
			let spread_method = convert_spread_method(radial.spread_method());

			modify_inputs.fill_gradient_set(gradient, gradient_type, spread_method, transform);
		}
		usvg::Paint::Pattern(_) => warn!("SVG patterns are not currently supported"),
	};
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::test_prelude::*;
	use graphene_std::renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender};

	#[tokio::test]
	async fn svg_group_alpha_mask_imports_as_hidden_clipping_mask() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
			<defs>
				<mask id="circle-mask" mask-type="alpha" maskUnits="userSpaceOnUse" x="0" y="0" width="128" height="128">
					<circle cx="64" cy="64" r="48" fill="white" fill-opacity="0.5" />
				</mask>
			</defs>
			<g transform="translate(10 20)" mask="url(#circle-mask)">
				<rect x="0" width="96" height="128" fill="#2563eb" fill-opacity="0.5" />
				<rect x="32" width="96" height="128" fill="#dc2626" fill-opacity="0.5" />
			</g>
		</svg>"##;

		editor
			.handle_message(GraphOperationMessage::NewSvg {
				id: NodeId::new(),
				svg: svg.to_string(),
				transform: DAffine2::IDENTITY,
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
				center: false,
			})
			.await;

		let instrumented = editor.eval_graph().await.expect("alpha-masked SVG import should evaluate");

		let clip_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::clipping_mask::ClipInput>(&editor.runtime).collect();
		assert_eq!(clip_values, vec![true], "the composited visible group should receive the SVG mask exactly once");
		let mask_mode_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::mask_mode::ModeInput>(&editor.runtime).collect();
		assert!(mask_mode_values.is_empty(), "alpha masks should retain the default behavior without an additional Mask Mode node");

		let has_fill_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::opacity::HasFillInput>(&editor.runtime).collect();
		assert_eq!(has_fill_values, vec![true], "the imported mask base should enable independent fill opacity");

		let fill_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::opacity::FillInput>(&editor.runtime).collect();
		assert_eq!(fill_values, vec![0.], "the imported mask artwork should be hidden during ordinary rendering");

		let paint_fills: Vec<_> = instrumented.grab_all_input::<graphene_std::vector::fill::FillInput<List<Color>>>(&editor.runtime).collect();
		assert!(
			paint_fills.iter().filter_map(|fill| fill.element(0)).any(|color| {
				let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
				(red - 1.).abs() < 1e-6 && (green - 1.).abs() < 1e-6 && (blue - 1.).abs() < 1e-6 && (alpha - 0.5).abs() < 1e-6
			}),
			"alpha mask paint should pass through the ordinary SVG fill importer; got {paint_fills:?}"
		);

		let translations: Vec<_> = instrumented.grab_all_input::<graphene_std::transform_nodes::transform::TranslationInput>(&editor.runtime).collect();
		let referencing_translation_count = translations.iter().filter(|translation| translation.abs_diff_eq(DVec2::new(10., 20.), 1e-10)).count();
		assert!(
			referencing_translation_count >= 2,
			"the visible artwork and independent mask subroot should both inherit the referencing transform; got {translations:?}"
		);

		let rendered_group_svgs: Vec<_> = instrumented
			.grab_all_input::<graphene_std::graphic::extend::NewInput<graphene_std::Graphic>>(&editor.runtime)
			.map(|graphics| {
				let mut render = SvgRender::new();
				graphics.render_svg(&mut render, &RenderParams::default());
				format!("{}{}", render.svg_defs, render.svg.to_svg_string())
			})
			.collect();
		let rendered_group_svg = rendered_group_svgs
			.iter()
			.find(|svg| svg.contains("<mask") && svg.contains("mask=\"url(#mask-"))
			.unwrap_or_else(|| panic!("the imported mask attributes should reach the existing SVG mask renderer; intermediate SVGs: {rendered_group_svgs:#?}"));
		assert!(rendered_group_svg.contains("opacity=\"0\""), "the mask base should remain hidden in the normal render pass");
		assert_eq!(
			rendered_group_svg.matches("mask=\"url(#mask-").count(),
			1,
			"the mask should wrap the composited visible group once, not each semi-transparent child independently"
		);
	}

	async fn assert_luminance_mask_import(mask_type_attribute: &str) {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let svg = format!(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
			<defs>
				<mask id="luminance-mask"{mask_type_attribute} maskUnits="userSpaceOnUse" x="0" y="0" width="128" height="128">
					<rect width="128" height="128" fill="white" />
					<circle cx="64" cy="64" r="32" fill="black" />
				</mask>
			</defs>
			<rect width="128" height="128" fill="#2563eb" mask="url(#luminance-mask)" />
		</svg>"##
		);

		editor
			.handle_message(GraphOperationMessage::NewSvg {
				id: NodeId::new(),
				svg,
				transform: DAffine2::IDENTITY,
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
				center: false,
			})
			.await;

		let instrumented = editor.eval_graph().await.expect("luminance-masked SVG import should evaluate");
		let clip_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::clipping_mask::ClipInput>(&editor.runtime).collect();
		assert_eq!(clip_values, vec![true], "the composited visible group should receive the luminance mask exactly once");

		let mask_mode_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::mask_mode::ModeInput>(&editor.runtime).collect();
		assert_eq!(mask_mode_values, vec![MaskMode::Luminance], "the imported hidden mask source should preserve usvg's resolved mask kind");

		let fill_values: Vec<_> = instrumented.grab_all_input::<graphene_std::blending_nodes::opacity::FillInput>(&editor.runtime).collect();
		assert_eq!(fill_values, vec![0.], "the luminance mask artwork should be hidden during ordinary rendering");

		let rendered_group_svgs: Vec<_> = instrumented
			.grab_all_input::<graphene_std::graphic::extend::NewInput<graphene_std::Graphic>>(&editor.runtime)
			.map(|graphics| {
				let mut render = SvgRender::new();
				graphics.render_svg(&mut render, &RenderParams::default());
				format!("{}{}", render.svg_defs, render.svg.to_svg_string())
			})
			.collect();
		assert!(
			rendered_group_svgs.iter().any(|svg| svg.contains("mask-type=\"luminance\"") && svg.contains("mask=\"url(#mask-")),
			"the imported mode should reach the SVG renderer; intermediate SVGs: {rendered_group_svgs:#?}"
		);
	}

	#[tokio::test]
	async fn svg_explicit_luminance_mask_preserves_its_mode() {
		assert_luminance_mask_import(r#" mask-type="luminance""#).await;
	}

	#[tokio::test]
	async fn svg_mask_defaults_to_luminance() {
		assert_luminance_mask_import("").await;
	}
}
