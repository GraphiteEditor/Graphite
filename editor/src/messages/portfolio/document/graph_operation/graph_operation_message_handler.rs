use super::transform_utils;
use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_clip_mode;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::renderer::convert_usvg_path::convert_usvg_path;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::style::{Fill, Gradient, GradientStops, GradientType, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};

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

				let artboard_layer = modify_inputs.create_artboard(id, artboard);
				network_interface.move_layer_to_stack(artboard_layer, LayerNodeIdentifier::ROOT_PARENT, 0, &[]);

				// If there is a non artboard feeding into the primary input of the artboard, move it to the secondary input
				let Some(artboard) = network_interface.document_network().nodes.get(&id) else {
					log::error!("Artboard not created");
					return;
				};
				let primary_input = artboard.inputs.first().expect("Artboard should have a primary input").clone();
				if let NodeInput::Node { node_id, .. } = &primary_input {
					if network_interface.is_layer(node_id, &[]) && !network_interface.is_artboard(node_id, &[]) {
						network_interface.move_layer_to_stack(LayerNodeIdentifier::new(*node_id, network_interface), artboard_layer, 0, &[]);
					} else {
						network_interface.disconnect_input(&InputConnector::node(artboard_layer.to_node(), 0), &[]);
						network_interface.set_input(&InputConnector::node(id, 0), primary_input, &[]);
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

				let size = tree.size();
				let offset_to_center = DVec2::new(size.width() as f64, size.height() as f64) / -2.;
				let transform = transform * DAffine2::from_translation(offset_to_center);

				import_usvg_node(&mut modify_inputs, &usvg::Node::Group(Box::new(tree.root().clone())), transform, id, parent, insert_index);
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

fn import_usvg_node(modify_inputs: &mut ModifyInputsContext, node: &usvg::Node, transform: DAffine2, id: NodeId, parent: LayerNodeIdentifier, insert_index: usize) {
	let layer = modify_inputs.create_layer(id);
	modify_inputs.network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
	modify_inputs.layer_node = Some(layer);
	if let Some(upstream_layer) = layer.next_sibling(modify_inputs.network_interface.document_metadata()) {
		modify_inputs.network_interface.shift_node(&upstream_layer.to_node(), IVec2::new(0, 3), &[]);
	}
	match node {
		usvg::Node::Group(group) => {
			for child in group.children() {
				import_usvg_node(modify_inputs, child, transform, NodeId::new(), layer, 0);
			}
			modify_inputs.layer_node = Some(layer);
		}
		usvg::Node::Path(path) => {
			let subpaths = convert_usvg_path(path);
			let bounds = subpaths.iter().filter_map(|subpath| subpath.bounding_box()).reduce(Quad::combine_bounds).unwrap_or_default();

			modify_inputs.insert_vector(subpaths, layer, true, path.fill().is_some(), path.stroke().is_some());

			if let Some(transform_node_id) = modify_inputs.existing_node_id("Transform", true) {
				transform_utils::update_transform(modify_inputs.network_interface, &transform_node_id, transform * usvg_transform(node.abs_transform()));
			}

			if let Some(fill) = path.fill() {
				let bounds_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
				apply_usvg_fill(fill, modify_inputs, transform * usvg_transform(node.abs_transform()), bounds_transform);
			}
			if let Some(stroke) = path.stroke() {
				apply_usvg_stroke(stroke, modify_inputs, transform * usvg_transform(node.abs_transform()));
			}
		}
		usvg::Node::Image(_image) => {
			warn!("Skip image")
		}
		usvg::Node::Text(text) => {
			let font = Font::new(graphene_std::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_std::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks().iter().map(|chunk| chunk.text()).collect(), font, TypesettingConfig::default(), layer);
			modify_inputs.fill_set(Fill::Solid(Color::BLACK));
		}
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
			non_scaling: false,
		})
	}
}

fn apply_usvg_fill(fill: &usvg::Fill, modify_inputs: &mut ModifyInputsContext, transform: DAffine2, bounds_transform: DAffine2) {
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
			let stops = GradientStops::new(stops);

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
			let stops = GradientStops::new(stops);

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
