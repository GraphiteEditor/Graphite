use super::transform_utils;
use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use crate::messages::prelude::*;

use graph_craft::document::{NodeId, NodeInput};
use graphene_core::renderer::Quad;
use graphene_core::text::Font;
use graphene_core::vector::style::{Fill, Gradient, GradientStops, GradientType, LineCap, LineJoin, Stroke};
use graphene_core::Color;
use graphene_std::vector::convert_usvg_path;

use glam::{DAffine2, DVec2};

pub struct GraphOperationMessageData<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub collapsed: &'a mut CollapsedLayers,
	pub node_graph: &'a mut NodeGraphMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct GraphOperationMessageHandler {}

// GraphOperationMessageHandler always modified the document network. This is so changes to the layers panel will only affect the document network.
// For changes to the selected network, use NodeGraphMessageHandler. No NodeGraphMessage's should be added here, since they will affect the selected nested network.
impl MessageHandler<GraphOperationMessage, GraphOperationMessageData<'_>> for GraphOperationMessageHandler {
	fn process_message(&mut self, message: GraphOperationMessage, responses: &mut VecDeque<Message>, data: GraphOperationMessageData) {
		let network_interface = data.network_interface;

		match message {
			GraphOperationMessage::FillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.fill_set(fill);
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
					modify_inputs.transform_change(transform, transform_in, parent_transform, skip_rerender);
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
			GraphOperationMessage::TransformSetPivot { layer, pivot } => {
				if layer == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Cannot run TransformSetPivot on ROOT_PARENT");
					return;
				}
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.pivot_set(pivot);
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
				let Some(artboard) = network_interface.network(&[]).unwrap().nodes.get(&id) else {
					log::error!("Artboard not created");
					return;
				};
				let primary_input = artboard.inputs.first().expect("Artboard should have a primary input").clone();
				if let NodeInput::Node { node_id, .. } = &primary_input {
					if network_interface.is_layer(node_id, &[]) && !network_interface.is_artboard(node_id, &[]) {
						network_interface.move_layer_to_stack(LayerNodeIdentifier::new(*node_id, network_interface, &[]), artboard_layer, 0, &[]);
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
				modify_inputs.insert_vector_data(subpaths, layer, true, true, true);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::NewTextLayer {
				id,
				text,
				font,
				size,
				line_height_ratio,
				character_spacing,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id);
				modify_inputs.insert_text(text, font, size, line_height_ratio, character_spacing, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index, &[]);
				responses.add(GraphOperationMessage::StrokeSet { layer, stroke: Stroke::default() });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			GraphOperationMessage::ResizeArtboard { layer, location, dimensions } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.resize_artboard(location, dimensions);
				}
			}
			GraphOperationMessage::ClearArtboards => {
				for artboard in network_interface.all_artboards() {
					responses.add(NodeGraphMessage::DeleteNodes {
						node_ids: vec![artboard.to_node()],
						delete_children: false,
					});
				}
				// TODO: Replace deleted artboards with merge nodes
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

			modify_inputs.insert_vector_data(subpaths, layer, true, path.fill().is_some(), path.stroke().is_some());

			if let Some(transform_node_id) = modify_inputs.existing_node_id("Transform") {
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
			let font = Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.to_string(), graphene_core::consts::DEFAULT_FONT_STYLE.to_string());
			modify_inputs.insert_text(text.chunks().iter().map(|chunk| chunk.text()).collect(), font, 24., 1.2, 1., layer);
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
			transform,
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
