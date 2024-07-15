use super::transform_utils;
use super::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, NodeTypePersistentMetadata};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::messages::prelude::*;

use graph_craft::document::{generate_uuid, NodeId, NodeInput};
use graphene_core::renderer::Quad;
use graphene_core::text::Font;
use graphene_core::vector::style::{Fill, Gradient, GradientStops, GradientType, LineCap, LineJoin, Stroke};
use graphene_core::Color;
use graphene_std::vector::convert_usvg_path;

use glam::{DAffine2, DVec2};

pub struct GraphOperationMessageData<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
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
			network_interface,
			selected_nodes,
			collapsed: _,
			node_graph: _,
		} = data;

		match message {
			GraphOperationMessage::MoveLayerToStack {
				layer,
				parent,
				insert_index,
				skip_rerender,
			} => {
				network_interface.move_layer_to_stack(layer, parent, insert_index);
				if !skip_rerender {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
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
				let parent_transform = network_interface.document_metadata().downstream_transform_to_viewport(layer);
				let current_transform = Some(network_interface.document_metadata().transform_to_viewport(layer));
				if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, current_transform, skip_rerender);
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
			GraphOperationMessage::NewArtboard { id, artboard } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);

				modify_inputs.create_artboard(id, artboard);
				responses.add_front(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
			}
			GraphOperationMessage::NewBitmapLayer {
				id,
				image_frame,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id, parent);
				modify_inputs.insert_image_data(image_frame, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index);
			}
			GraphOperationMessage::NewCustomLayer { id, nodes, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id, parent);

				if nodes.len() > 0 {
					// Add the nodes to the network
					let new_ids: HashMap<_, _> = nodes.iter().map(|(&id, _)| (id, NodeId(generate_uuid()))).collect();
					// Since all the new nodes are already connected, just connect the input of the layer to first new node
					let first_new_node_id = new_ids[&NodeId(0)];
					responses.add(NodeGraphMessage::AddNodes {
						nodes,
						new_ids: new_ids,
						use_document_network: true,
					});

					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(layer.to_node(), 1),
						input: NodeInput::node(first_new_node_id, 0),
						use_document_network: true,
					});
				}
				// Move the layer and all nodes to the correct position in the network
				responses.add(GraphOperationMessage::MoveLayerToStack {
					layer,
					parent,
					insert_index,
					skip_rerender: false,
				})
			}
			GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index } => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id, parent);
				modify_inputs.insert_vector_data(subpaths, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index);
			}
			GraphOperationMessage::NewTextLayer {
				id,
				text,
				font,
				size,
				parent,
				insert_index,
			} => {
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);
				let layer = modify_inputs.create_layer(id, parent);
				modify_inputs.insert_text(text, font, size, layer);
				network_interface.move_layer_to_stack(layer, parent, insert_index);
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
						reconnect: false,
						use_document_network: true,
					});
				}
				//TODO: Replace deleted artboards with merge nodes
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
				let mut modify_inputs = ModifyInputsContext::new(network_interface, responses);

				import_usvg_node(&mut modify_inputs, &usvg::Node::Group(Box::new(tree.root)), transform, id, parent, insert_index);
			}
			GraphOperationMessage::ShiftUpstream { node_id, shift, shift_self } => {
				network_interface.shift_upstream(node_id, shift, shift_self);
			}
			GraphOperationMessage::ToggleSelectedVisibility => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes
					.selected_layers(&network_interface.document_metadata())
					.all(|layer| network_interface.is_visible(&layer.to_node()));

				for layer in selected_nodes.selected_layers(&network_interface.document_metadata()) {
					responses.add(GraphOperationMessage::SetVisibility { node_id: layer.to_node(), visible });
				}
			}
			GraphOperationMessage::ToggleVisibility { node_id } => {
				let visible = !network_interface.is_visible(&node_id);
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::SetVisibility { node_id, visible });
			}
			GraphOperationMessage::SetVisibility { node_id, visible } => {
				network_interface.set_visibility(node_id, visible);

				// Only execute node graph if one of the selected nodes is connected to the output
				if network_interface.connected_to_output(&node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			GraphOperationMessage::StartPreviewingWithoutRestore => {
				network_interface.start_previewing_without_restore();
			}
			GraphOperationMessage::ToggleSelectedLocked => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are locked, show them all. Otherwise, hide them all.
				let locked = !selected_nodes
					.selected_layers(&network_interface.document_metadata())
					.all(|layer| network_interface.is_locked(&layer.to_node()));

				for layer in selected_nodes.selected_layers(&network_interface.document_metadata()) {
					responses.add(GraphOperationMessage::SetLocked { layer, locked });
				}
			}
			GraphOperationMessage::ToggleLocked { layer } => {
				let Some(node_metadata) = network_interface.document_network_metadata().persistent_metadata.node_metadata.get(&layer.to_node()) else {
					log::error!("Cannot get node {:?} in GraphOperationMessage::ToggleLocked", layer.to_node());
					return;
				};

				let locked = if let NodeTypePersistentMetadata::Layer(layer_metadata) = &node_metadata.persistent_metadata.node_type_metadata {
					!layer_metadata.locked
				} else {
					log::error!("Layer should always store LayerPersistentMetadata");
					false
				};
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::SetLocked { layer, locked });
			}
			GraphOperationMessage::SetLocked { layer, locked } => {
				network_interface.set_locked(layer.to_node(), locked);

				if network_interface.connected_to_output(&layer.to_node()) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				responses.add(NodeGraphMessage::SelectedNodesUpdated)
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
	let layer = modify_inputs.create_layer(id, parent);
	modify_inputs.layer_node = Some(layer);
	match node {
		usvg::Node::Group(group) => {
			for child in &group.children {
				import_usvg_node(modify_inputs, child, transform, NodeId(generate_uuid()), layer, 0);
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

			modify_inputs.network_interface.move_layer_to_stack(layer, parent, insert_index);

			if let Some(transform_node_id) = modify_inputs.get_existing_node_id("Transform") {
				transform_utils::update_transform(&mut modify_inputs.network_interface, &transform_node_id, transform * usvg_transform(node.abs_transform()));
			}

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
				let stops = linear.stops.iter().map(|stop| (stop.offset.get() as f64, usvg_color(stop.color, stop.opacity.get()))).collect();
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
				let stops = radial.stops.iter().map(|stop| (stop.offset.get() as f64, usvg_color(stop.color, stop.opacity.get()))).collect();
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
