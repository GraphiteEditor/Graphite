use super::transform_utils::{self, LayerBounds};
use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::messages::prelude::*;

use bezier_rs::{ManipulatorGroup, Subpath};
use graph_craft::document::{generate_uuid, NodeId, NodeInput, NodeNetwork};
use graphene_core::renderer::Quad;
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::style::{Fill, Gradient, GradientType, LineCap, LineJoin, Stroke};
use graphene_core::Color;

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
			GraphOperationMessage::FillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.fill_set(fill);
				}
			}
			GraphOperationMessage::OpacitySet { layer, opacity } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.opacity_set(opacity);
				}
			}
			GraphOperationMessage::BlendModeSet { layer, blend_mode } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.blend_mode_set(blend_mode);
				}
			}
			GraphOperationMessage::UpdateBounds { layer, old_bounds, new_bounds } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.update_bounds(old_bounds, new_bounds);
				}
			}
			GraphOperationMessage::StrokeSet { layer, stroke } => {
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
				let parent_transform = document_metadata.downstream_transform_to_viewport(layer);

				let current_transform = Some(document_metadata.transform_to_viewport(layer));
				let bounds = LayerBounds::new(document_metadata, layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, current_transform, bounds, skip_rerender);
				}
			}
			GraphOperationMessage::TransformSetPivot { layer, pivot } => {
				let bounds = LayerBounds::new(document_metadata, layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.pivot_set(pivot, bounds);
				}
			}
			GraphOperationMessage::Vector { layer, modification } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.vector_modify(modification);
				}
			}
			GraphOperationMessage::Brush { layer, strokes } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer.to_node(), document_network, document_metadata, node_graph, responses) {
					modify_inputs.brush_modify(strokes);
				}
			}
			GraphOperationMessage::NewArtboard { id, artboard } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				//currently creates a layer with output node connected to output, and previous connection to output feeds into Layer.
				/*
				if let Some(layer) = modify_inputs.create_layer(id, modify_inputs.document_network.original_outputs()[0].node_id, 0) {
					modify_inputs.insert_artboard(artboard, layer);
				}
				*/
				//Instead, create_artboard should be called which should create the artboard node and adds it to the graph in a similar way as create_layer
				//modify_inputs.create_artboard(id, artboard, modify_inputs.document_network.original_outputs()[0].node_id);
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
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
				info!("Inserting new layer {id} as a child of {parent:?} at index {insert_index}");

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
						info!("Inserting nodes");
					}

					if let Some(layer_node) = modify_inputs.document_network.nodes.get_mut(&layer) {
						if let Some(&input) = new_ids.get(&NodeId(0)) {
							layer_node.inputs[1] = NodeInput::node(input, 0);
							info!("Linking node");
						}
					}

					modify_inputs.responses.add(NodeGraphMessage::RunDocumentGraph);
				} else {
					error!("Create failed");
				}

				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
			}
			GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer_with_insert_index(id, insert_index, parent) {
					modify_inputs.insert_vector_data(subpaths, layer);
				}
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
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
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
			}
			GraphOperationMessage::ResizeArtboard { id, location, dimensions } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(id, document_network, document_metadata, node_graph, responses) {
					modify_inputs.resize_artboard(location, dimensions);
				}
			}
			GraphOperationMessage::DeleteLayer { id } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				modify_inputs.delete_layer(id, selected_nodes, false);
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
			}
			GraphOperationMessage::DeleteArtboard { id } => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				if let Some(artboard_id) = modify_inputs.document_network.nodes.get(&id).and_then(|node| node.inputs[0].as_node()) {
					modify_inputs.delete_artboard(artboard_id, selected_nodes);
				} else {
					warn!("Artboard does not exist");
				}
				modify_inputs.delete_layer(id, selected_nodes, true);
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
			}
			GraphOperationMessage::ClearArtboards => {
				let mut modify_inputs = ModifyInputsContext::new(document_network, document_metadata, node_graph, responses);
				let layer_nodes = modify_inputs.document_network.nodes.iter().filter(|(_, node)| node.is_layer()).map(|(id, _)| *id).collect::<Vec<_>>();
				for layer in layer_nodes {
					let artboards = modify_inputs
						.document_network
						.upstream_flow_back_from_nodes(vec![layer], true)
						.filter_map(|(node, _id)| if node.is_artboard() { Some(_id) } else { None })
						.collect::<Vec<_>>();
					if artboards.is_empty() {
						continue;
					}
					for artboard in artboards {
						modify_inputs.delete_artboard(artboard, selected_nodes);
					}
					modify_inputs.delete_layer(layer, selected_nodes, true);
				}
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
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
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GraphOperationMessage;)
	}
}

pub fn load_network_structure(document_network: &NodeNetwork, document_metadata: &mut DocumentMetadata, selected_nodes: &mut SelectedNodes, collapsed: &mut CollapsedLayers) {
	document_metadata.load_structure(document_network, selected_nodes);
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
			let font = Font::new(crate::consts::DEFAULT_FONT_FAMILY.to_string(), crate::consts::DEFAULT_FONT_STYLE.to_string());
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

fn convert_usvg_path(path: &usvg::Path) -> Vec<Subpath<ManipulatorGroupId>> {
	let mut subpaths = Vec::new();
	let mut groups = Vec::new();

	let mut points = path.data.points().iter();
	let to_vec = |p: &usvg::tiny_skia_path::Point| DVec2::new(p.x as f64, p.y as f64);

	for verb in path.data.verbs() {
		match verb {
			usvg::tiny_skia_path::PathVerb::Move => {
				subpaths.push(Subpath::new(std::mem::take(&mut groups), false));
				let Some(start) = points.next().map(to_vec) else { continue };
				groups.push(ManipulatorGroup::new(start, Some(start), Some(start)));
			}
			usvg::tiny_skia_path::PathVerb::Line => {
				let Some(end) = points.next().map(to_vec) else { continue };
				groups.push(ManipulatorGroup::new(end, Some(end), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Quad => {
				let Some(handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = groups.last_mut() {
					last.out_handle = Some(last.anchor + (2. / 3.) * (handle - last.anchor));
				}
				groups.push(ManipulatorGroup::new(end, Some(end + (2. / 3.) * (handle - end)), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Cubic => {
				let Some(first_handle) = points.next().map(to_vec) else { continue };
				let Some(second_handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = groups.last_mut() {
					last.out_handle = Some(first_handle);
				}
				groups.push(ManipulatorGroup::new(end, Some(second_handle), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Close => {
				subpaths.push(Subpath::new(std::mem::take(&mut groups), true));
			}
		}
	}
	subpaths.push(Subpath::new(groups, false));
	subpaths
}
