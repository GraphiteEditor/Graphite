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
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::renderer::convert_usvg_path::convert_usvg_path;
use graphene_std::table::Table;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::style::{Fill, Gradient, GradientStop, GradientStops, GradientType, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};

#[derive(ExtractField)]
pub struct GraphOperationMessageContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub collapsed: &'a mut CollapsedLayers,
	pub node_graph: &'a mut NodeGraphMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize, ExtractField)]
pub struct GraphOperationMessageHandler {}

// GraphOperationMessageHandler always modified the document network. This is so changes to the layers panel will only affect the document network.
// For changes to the selected network, use NodeGraphMessageHandler. No NodeGraphMessage's should be added here, since they will affect the selected nested network.
#[message_handler_data]
impl MessageHandler<GraphOperationMessage, GraphOperationMessageContext<'_>> for GraphOperationMessageHandler {
	fn process_message(&mut self, message: GraphOperationMessage, responses: &mut VecDeque<Message>, context: GraphOperationMessageContext) {
		let network_interface = context.network_interface;

		match message {
			GraphOperationMessage::FillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.fill_set(fill);
				}
			}
			GraphOperationMessage::BlendingFillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.blending_fill_set(fill);
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
			GraphOperationMessage::StrokeSet { layer, stroke } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.stroke_set(stroke);
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
			GraphOperationMessage::NewArtboard { id, artboard } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);

				let artboard_location = artboard.location;
				let artboard_layer = modify_inputs.create_artboard(id, artboard);
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
								transform: DAffine2::from_translation(-artboard_location.as_dvec2()),
								transform_in: TransformIn::Local,
								skip_rerender: true,
							});
						}

						// Set the bottom input of the artboard back to artboard
						let bottom_input = NodeInput::value(TaggedValue::Artboard(Table::new()), true);
						network_interface.set_input(&InputConnector::node(artboard_layer.to_node(), 0), bottom_input, &[]);
					} else {
						// We have some non layers (e.g. just a rectangle node). We disconnect the bottom input and connect it to the left input.
						network_interface.disconnect_input(&InputConnector::node(artboard_layer.to_node(), 0), &[]);
						network_interface.set_input(&InputConnector::node(artboard_layer.to_node(), 1), primary_input, &[]);

						// Set the bottom input of the artboard back to artboard
						let bottom_input = NodeInput::value(TaggedValue::Artboard(Table::new()), true);
						network_interface.set_input(&InputConnector::node(artboard_layer.to_node(), 0), bottom_input, &[]);
					}
				}
				responses.add_front(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewBitmapLayer {
				id,
				image_frame,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_image_data(image_frame, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewBooleanOperationLayer { id, operation, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_boolean_data(operation, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::SetDisplayNameImpl {
					node_id: id,
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
				responses.add(GraphOperationMessage::StrokeSet { layer, stroke: Stroke::default() });
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
	Color::from_rgbaf32_unchecked(c.red as f32 / 255., c.green as f32 / 255., c.blue as f32 / 255., a)
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
			// Collect child extents for O(n) position calculation
			let mut child_extents_svg_order: Vec<u32> = Vec::new();
			let mut group_extents_map: HashMap<LayerNodeIdentifier, Vec<u32>> = HashMap::new();

			// Enable import mode: skips expensive is_acyclic checks and per-node cache invalidation
			// during wiring since we're building a known tree structure where cycles are impossible
			modify_inputs.import = true;

			for child in group.children() {
				let extent = import_usvg_node_inner(modify_inputs, child, NodeId::new(), layer, 0, graphite_gradient_stops, &mut group_extents_map);
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
			import_usvg_path(modify_inputs, node, path, layer, graphite_gradient_stops);
		}
		usvg::Node::Image(_image) => {
			warn!("Skip image");
		}
		usvg::Node::Text(text) => {
			let font = Font::new(graphene_std::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_std::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks().iter().map(|chunk| chunk.text()).collect(), font, TypesettingConfig::default(), layer);
			modify_inputs.fill_set(Fill::Solid(Color::BLACK));
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
	graphite_gradient_stops: &HashMap<String, GradientStops>,
	group_extents_map: &mut HashMap<LayerNodeIdentifier, Vec<u32>>,
) -> u32 {
	let layer = modify_inputs.create_layer(id);
	modify_inputs.network_interface.move_layer_to_stack_for_import(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);

	match node {
		usvg::Node::Group(group) => {
			let mut child_extents: Vec<u32> = Vec::new();
			for child in group.children() {
				let extent = import_usvg_node_inner(modify_inputs, child, NodeId::new(), layer, 0, graphite_gradient_stops, group_extents_map);
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
			modify_inputs.fill_set(Fill::Solid(Color::BLACK));
			0
		}
	}
}

/// Helper to apply path data (vector geometry, fill, stroke, transform) to a layer.
fn import_usvg_path(modify_inputs: &mut ModifyInputsContext, node: &usvg::Node, path: &usvg::Path, layer: LayerNodeIdentifier, graphite_gradient_stops: &HashMap<String, GradientStops>) {
	let subpaths = convert_usvg_path(path);
	let bounds = subpaths.iter().filter_map(|subpath| subpath.bounding_box()).reduce(Quad::combine_bounds).unwrap_or_default();

	// Skip creating a Transform node entirely when the SVG-native transform is identity.
	let node_transform = usvg_transform(node.abs_transform());
	let has_transform = node_transform != DAffine2::IDENTITY;

	modify_inputs.insert_vector(subpaths, layer, has_transform, path.fill().is_some(), path.stroke().is_some());

	if has_transform && let Some(transform_node_id) = modify_inputs.existing_network_node_id("Transform", false) {
		transform_utils::update_transform(modify_inputs.network_interface, &transform_node_id, node_transform);
	}

	if let Some(fill) = path.fill() {
		let bounds_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		apply_usvg_fill(fill, modify_inputs, bounds_transform, graphite_gradient_stops);
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
			// Top of stack — set to `Absolute` position
			network_interface.set_layer_position_for_import(&child_layer.to_node(), LayerPosition::Absolute(child_pos), &[]);
		} else {
			// Below top — set `Stack` with `y_offset` based on previous sibling's subtree extent
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
		modify_inputs.stroke_set(Stroke {
			color: Some(usvg_color(*color, stroke.opacity().get())),
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
		})
	}
}

fn apply_usvg_fill(fill: &usvg::Fill, modify_inputs: &mut ModifyInputsContext, bounds_transform: DAffine2, graphite_gradient_stops: &HashMap<String, GradientStops>) {
	modify_inputs.fill_set(match &fill.paint() {
		usvg::Paint::Color(color) => Fill::solid(usvg_color(*color, fill.opacity().get())),
		usvg::Paint::LinearGradient(linear) => {
			let gradient_transform = usvg_transform(linear.transform());
			let (start, end) = (DVec2::new(linear.x1() as f64, linear.y1() as f64), DVec2::new(linear.x2() as f64, linear.y2() as f64));
			let (start, end) = (gradient_transform.transform_point2(start), gradient_transform.transform_point2(end));
			let (start, end) = (bounds_transform.inverse().transform_point2(start), bounds_transform.inverse().transform_point2(end));

			let gradient_type = GradientType::Linear;

			let stops = match graphite_gradient_stops.get(linear.id()) {
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

			Fill::Gradient(Gradient { start, end, gradient_type, stops })
		}
		usvg::Paint::RadialGradient(radial) => {
			let gradient_transform = usvg_transform(radial.transform());
			let center = DVec2::new(radial.cx() as f64, radial.cy() as f64);
			let edge = center + DVec2::X * radial.r().get() as f64;
			let (start, end) = (gradient_transform.transform_point2(center), gradient_transform.transform_point2(edge));
			let (start, end) = (bounds_transform.inverse().transform_point2(start), bounds_transform.inverse().transform_point2(end));

			let gradient_type = GradientType::Radial;

			let stops = match graphite_gradient_stops.get(radial.id()) {
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

			Fill::Gradient(Gradient { start, end, gradient_type, stops })
		}
		usvg::Paint::Pattern(_) => {
			warn!("SVG patterns are not currently supported");
			return;
		}
	});
}
