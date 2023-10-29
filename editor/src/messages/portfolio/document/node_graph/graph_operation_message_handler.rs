use super::{resolve_document_node_type, VectorDataModification};
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use document_legacy::document::Document;
use document_legacy::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use document_legacy::{LayerId, Operation};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, NodeId, NodeInput, NodeNetwork, NodeOutput};
use graphene_core::raster::ImageFrame;
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, FillType, Stroke};
use graphene_core::{Artboard, Color};
use transform_utils::LayerBounds;

use glam::{DAffine2, DVec2, IVec2};

pub mod transform_utils;

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct GraphOperationMessageHandler;

struct ModifyInputsContext<'a> {
	network: &'a mut NodeNetwork,
	node_graph: &'a mut NodeGraphMessageHandler,
	responses: &'a mut VecDeque<Message>,
	layer: &'a [LayerId],
	outwards_links: HashMap<NodeId, Vec<NodeId>>,
	layer_node: Option<NodeId>,
	document_metadata: &'a mut DocumentMetadata,
}
impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	fn new(document: &'a mut Document, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			outwards_links: document.document_network.collect_outwards_links(),
			network: &mut document.document_network,
			node_graph,
			responses,
			layer: &[],
			layer_node: None,
			document_metadata: &mut document.metadata,
		}
	}

	/// Get the node network from the document
	fn new_layer(layer: &'a [LayerId], document: &'a mut Document, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Option<Self> {
		let mut document = Self::new(document, node_graph, responses);
		let Some(mut id) = layer.last().copied() else {
			error!("Tried to modify root layer");
			return None;
		};
		while document.network.nodes.get(&id)?.name != "Layer" {
			id = document.outwards_links.get(&id)?.first().copied()?;
		}
		document.layer_node = Some(id);
		Some(document)
	}

	/// Updates the input of an existing node
	fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let document_node = self.network.nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs, node_id, &self.document_metadata);
	}

	pub fn insert_between(&mut self, id: NodeId, pre: NodeOutput, post: NodeOutput, mut node: DocumentNode, input: usize, output: usize, shift_upstream: IVec2) -> Option<NodeId> {
		assert!(!self.network.nodes.contains_key(&id), "Creating already existing node");
		let pre_node = self.network.nodes.get_mut(&pre.node_id)?;
		node.metadata.position = pre_node.metadata.position;

		let post_node = self.network.nodes.get_mut(&post.node_id)?;
		node.inputs[input] = NodeInput::node(pre.node_id, pre.node_output_index);
		post_node.inputs[post.node_output_index] = NodeInput::node(id, output);

		self.network.nodes.insert(id, node);

		self.shift_upstream(id, shift_upstream);

		Some(id)
	}

	pub fn insert_node_before(&mut self, new_id: NodeId, node_id: NodeId, input_index: usize, mut document_node: DocumentNode, offset: IVec2) -> Option<NodeId> {
		assert!(!self.network.nodes.contains_key(&new_id), "Creating already existing node");
		let post_node = self.network.nodes.get_mut(&node_id)?;

		post_node.inputs[input_index] = NodeInput::node(new_id, 0);
		document_node.metadata.position = post_node.metadata.position + offset;
		self.network.nodes.insert(new_id, document_node);

		Some(new_id)
	}

	pub fn create_layer(&mut self, new_id: NodeId, output_node_id: NodeId, input_index: usize) -> Option<NodeId> {
		assert!(!self.network.nodes.contains_key(&new_id), "Creating already existing layer");

		let output = NodeOutput::new(output_node_id, input_index);
		// Locate the node output of the first sibling layer to the new layer
		let new_id = if let NodeInput::Node { node_id, output_index, .. } = &self.network.nodes.get(&output_node_id)?.inputs[input_index] {
			let sibling_node = &self.network.nodes.get(node_id)?;
			let node_id = *node_id;
			let output_index = *output_index;
			let sibling_layer = if sibling_node.name == "Layer" {
				// There is already a layer node
				NodeOutput::new(node_id, 0)
			} else {
				// The user has connected another node to the output. Insert a layer node between the output and the node.
				let mut node = resolve_document_node_type("Layer").expect("Layer node").default_document_node();
				let node_id = self.insert_between(generate_uuid(), NodeOutput::new(node_id, output_index), output, node, 0, 0, IVec2::new(-8, 0))?;
				NodeOutput::new(node_id, 0)
			};

			let node = resolve_document_node_type("Layer").expect("Layer node").default_document_node();
			self.insert_between(new_id, sibling_layer, output, node, 7, 0, IVec2::new(0, 3))
		} else {
			let mut layer_node = resolve_document_node_type("Layer").expect("Node").default_document_node();
			self.insert_node_before(new_id, output_node_id, input_index, layer_node, IVec2::new(-5, 3))
		};

		// Update the document metadata structure
		if let Some(new_id) = new_id {
			let parent = if self.network.nodes.get(&output_node_id).is_some_and(|node| node.name == "Layer") {
				LayerNodeIdentifier::new(output_node_id, self.network)
			} else {
				LayerNodeIdentifier::ROOT
			};
			let new_child = LayerNodeIdentifier::new(new_id, self.network);
			parent.push_front_child(self.document_metadata, new_child);
			self.responses.add(DocumentMessage::DocumentStructureChanged);
		}

		new_id
	}

	fn insert_artboard(&mut self, artboard: Artboard, layer: NodeId) -> Option<NodeId> {
		let artboard_node = resolve_document_node_type("Artboard").expect("Node").to_document_node_default_inputs(
			[
				None,
				Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
				Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
				Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
			],
			Default::default(),
		);
		self.responses.add(NodeGraphMessage::SendGraph { should_rerender: true });
		let cull_id = generate_uuid();
		self.insert_node_before(generate_uuid(), layer, 0, artboard_node, IVec2::new(-8, 0))
	}

	fn insert_vector_data(&mut self, subpaths: Vec<Subpath<ManipulatorGroupId>>, layer: NodeId) {
		let shape = {
			let node_type = resolve_document_node_type("Shape").expect("Shape node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))], Default::default())
		};
		let cull = resolve_document_node_type("Cull").expect("Cull node does not exist").default_document_node();
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = generate_uuid();
		self.insert_node_before(stroke_id, layer, 0, stroke, IVec2::new(-8, 0));
		let fill_id = generate_uuid();
		self.insert_node_before(fill_id, stroke_id, 0, fill, IVec2::new(-8, 0));
		let transform_id = generate_uuid();
		self.insert_node_before(transform_id, fill_id, 0, transform, IVec2::new(-8, 0));
		let cull_id = generate_uuid();
		self.insert_node_before(cull_id, transform_id, 0, cull, IVec2::new(-8, 0));
		let shape_id = generate_uuid();
		self.insert_node_before(shape_id, cull_id, 0, shape, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::SendGraph { should_rerender: true });
	}

	fn insert_text(&mut self, text: String, font: Font, size: f64, layer: NodeId) {
		let text = resolve_document_node_type("Text").expect("Text node does not exist").to_document_node(
			[
				NodeInput::Network(graph_craft::concrete!(graphene_std::wasm_application_io::WasmEditorApi)),
				NodeInput::value(TaggedValue::String(text), false),
				NodeInput::value(TaggedValue::Font(font), false),
				NodeInput::value(TaggedValue::F64(size), false),
			],
			Default::default(),
		);
		let cull = resolve_document_node_type("Cull").expect("Cull node does not exist").default_document_node();
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = generate_uuid();
		self.insert_node_before(stroke_id, layer, 0, stroke, IVec2::new(-8, 0));
		let fill_id = generate_uuid();
		self.insert_node_before(fill_id, stroke_id, 0, fill, IVec2::new(-8, 0));
		let transform_id = generate_uuid();
		self.insert_node_before(transform_id, fill_id, 0, transform, IVec2::new(-8, 0));
		let cull_id = generate_uuid();
		self.insert_node_before(cull_id, transform_id, 0, cull, IVec2::new(-8, 0));
		let text_id = generate_uuid();
		self.insert_node_before(text_id, cull_id, 0, text, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::SendGraph { should_rerender: true });
	}

	fn insert_image_data(&mut self, image_frame: ImageFrame<Color>, layer: NodeId) {
		let image = {
			let node_type = resolve_document_node_type("Image").expect("Image node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))], Default::default())
		};
		let sample = resolve_document_node_type("Sample").expect("Sample node does not exist").default_document_node();
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();

		let transform_id = generate_uuid();
		self.insert_node_before(transform_id, layer, 0, transform, IVec2::new(-8, 0));
		let sample_id = generate_uuid();
		self.insert_node_before(sample_id, transform_id, 0, sample, IVec2::new(-8, 0));
		let image_id = generate_uuid();
		self.insert_node_before(image_id, sample_id, 0, image, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::SendGraph { should_rerender: true });
	}

	fn shift_upstream(&mut self, node_id: NodeId, shift: IVec2) {
		let mut shift_nodes = HashSet::new();
		let mut stack = vec![node_id];
		while let Some(node_id) = stack.pop() {
			let Some(node) = self.network.nodes.get(&node_id) else { continue };
			for input in &node.inputs {
				let NodeInput::Node { node_id, .. } = input else { continue };
				if shift_nodes.insert(*node_id) {
					stack.push(*node_id);
				}
			}
		}

		for node_id in shift_nodes {
			if let Some(node) = self.network.nodes.get_mut(&node_id) {
				node.metadata.position += shift;
			}
		}
	}

	/// Inserts a new node and modifies the inputs
	fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let output_node_id = self.layer_node.unwrap_or(self.network.outputs[0].node_id);
		let Some(output_node) = self.network.nodes.get_mut(&output_node_id) else {
			warn!("Output node doesn't exist");
			return;
		};

		let metadata = output_node.metadata.clone();
		let new_input = output_node.inputs[0].clone();
		let node_id = generate_uuid();

		output_node.metadata.position.x += 8;
		output_node.inputs[0] = NodeInput::node(node_id, 0);

		let Some(node_type) = resolve_document_node_type(name) else {
			warn!("Node type \"{name}\" doesn't exist");
			return;
		};
		let mut new_document_node = node_type.to_document_node_default_inputs([Some(new_input)], metadata);
		update_input(&mut new_document_node.inputs, node_id, &self.document_metadata);
		self.network.nodes.insert(node_id, new_document_node);
	}

	/// Changes the inputs of a specific node
	fn modify_inputs(&mut self, name: &'static str, skip_rerender: bool, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_node_id = self.network.primary_flow_from_opt(self.layer_node).find(|(node, _)| node.name == name).map(|(_, id)| id);
		if let Some(node_id) = existing_node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		} else {
			self.modify_new_node(name, update_input);
		}

		self.node_graph.network.clear();
		self.responses.add(PropertiesPanelMessage::ResendActiveProperties);
		let layer_path = self.layer.to_vec();

		if !skip_rerender {
			self.responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
		} else {
			self.responses.add(DocumentMessage::FrameClear);
		}
		if existing_node_id.is_none() {
			self.responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
		}
	}

	/// Changes the inputs of a all of the existing instances of a node name
	fn modify_all_node_inputs(&mut self, name: &'static str, skip_rerender: bool, mut update_input: impl FnMut(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_nodes: Vec<_> = self.network.primary_flow_from_opt(self.layer_node).filter(|(node, _)| node.name == name).map(|(_, id)| id).collect();
		for existing_node_id in existing_nodes {
			self.modify_existing_node_inputs(existing_node_id, &mut update_input);
		}

		self.responses.add(PropertiesPanelMessage::ResendActiveProperties);
		let layer_path = self.layer.to_vec();

		if !skip_rerender {
			self.responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
		} else {
			self.responses.add(DocumentMessage::FrameClear);
		}
	}

	fn fill_set(&mut self, fill: Fill) {
		self.modify_inputs("Fill", false, |inputs, _node_id, _metadata| {
			let fill_type = match fill {
				Fill::None => FillType::None,
				Fill::Solid(_) => FillType::Solid,
				Fill::Gradient(_) => FillType::Gradient,
			};
			inputs[1] = NodeInput::value(TaggedValue::FillType(fill_type), false);
			if Fill::None == fill {
				inputs[2] = NodeInput::value(TaggedValue::OptionalColor(None), false);
			} else if let Fill::Solid(color) = fill {
				inputs[2] = NodeInput::value(TaggedValue::OptionalColor(Some(color)), false);
			} else if let Fill::Gradient(gradient) = fill {
				inputs[3] = NodeInput::value(TaggedValue::GradientType(gradient.gradient_type), false);
				inputs[4] = NodeInput::value(TaggedValue::DVec2(gradient.start), false);
				inputs[5] = NodeInput::value(TaggedValue::DVec2(gradient.end), false);
				inputs[6] = NodeInput::value(TaggedValue::DAffine2(gradient.transform), false);
				inputs[7] = NodeInput::value(TaggedValue::GradientPositions(gradient.positions), false);
			}
		});
	}

	fn stroke_set(&mut self, stroke: Stroke) {
		self.modify_inputs("Stroke", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::OptionalColor(stroke.color), false);
			inputs[2] = NodeInput::value(TaggedValue::F32(stroke.weight as f32), false);
			inputs[3] = NodeInput::value(TaggedValue::VecF32(stroke.dash_lengths), false);
			inputs[4] = NodeInput::value(TaggedValue::F32(stroke.dash_offset as f32), false);
			inputs[5] = NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false);
			inputs[6] = NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false);
			inputs[7] = NodeInput::value(TaggedValue::F32(stroke.line_join_miter_limit as f32), false);
		});
	}

	fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, node_id, metadata| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let upstream_transform = metadata.upstream_transform(node_id);
			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(upstream_transform.transform_point2(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs))));
			let transform = pivot.inverse() * to.inverse() * transform * to * pivot * layer_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}

	fn transform_set(&mut self, mut transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, current_transform: Option<DAffine2>, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, node_id, metadata| {
			let upstream_transform = metadata.upstream_transform(node_id);

			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(upstream_transform.transform_point2(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs))));

			if let Some(current_transform) = current_transform.filter(|transform| transform.matrix2.determinant() != 0. && upstream_transform.matrix2.determinant() != 0.) {
				transform = transform * upstream_transform.inverse();
			}
			let final_transform = pivot.inverse() * to.inverse() * transform * pivot;
			transform_utils::update_transform(inputs, final_transform);
		});
	}

	fn pivot_set(&mut self, new_pivot: DVec2, bounds: LayerBounds) {
		self.modify_inputs("Transform", false, |inputs, node_id, metadata| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let upstream_transform = metadata.upstream_transform(node_id);
			let old_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(bounds.local_pivot(transform_utils::get_current_normalized_pivot(inputs))));
			let new_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(bounds.local_pivot(new_pivot)));
			let transform = new_pivot_transform.inverse() * old_pivot_transform * layer_transform * old_pivot_transform.inverse() * new_pivot_transform;
			transform_utils::update_transform(inputs, transform);
			inputs[5] = NodeInput::value(TaggedValue::DVec2(new_pivot), false);
		});
	}

	fn update_bounds(&mut self, [old_bounds_min, old_bounds_max]: [DVec2; 2], [new_bounds_min, new_bounds_max]: [DVec2; 2]) {
		self.modify_all_node_inputs("Transform", false, |inputs, node_id, metadata| {
			let upstream_transform = metadata.upstream_transform(node_id);
			let layer_transform = transform_utils::get_current_transform(inputs);
			let normalized_pivot = transform_utils::get_current_normalized_pivot(inputs);

			let old_layerspace_pivot = (old_bounds_max - old_bounds_min) * normalized_pivot + old_bounds_min;
			let new_layerspace_pivot = (new_bounds_max - new_bounds_min) * normalized_pivot + new_bounds_min;
			let new_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(new_layerspace_pivot));
			let old_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(old_layerspace_pivot));

			let transform = new_pivot_transform.inverse() * old_pivot_transform * layer_transform * old_pivot_transform.inverse() * new_pivot_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}

	fn vector_modify(&mut self, modification: VectorDataModification) {
		// TODO: Allow modifying a graph with a "Text" node.
		if self.network.nodes.values().any(|node| node.name == "Text") {
			return;
		}

		let [mut old_bounds_min, mut old_bounds_max] = [DVec2::ZERO, DVec2::ONE];
		let [mut new_bounds_min, mut new_bounds_max] = [DVec2::ZERO, DVec2::ONE];

		self.modify_inputs("Shape", false, |inputs, _node_id, _metadata| {
			let [subpaths, mirror_angle_groups] = inputs.as_mut_slice() else {
				panic!("Shape does not have subpath and mirror angle inputs");
			};

			let NodeInput::Value {
				tagged_value: TaggedValue::Subpaths(subpaths),
				..
			} = subpaths
			else {
				return;
			};
			let NodeInput::Value {
				tagged_value: TaggedValue::ManipulatorGroupIds(mirror_angle_groups),
				..
			} = mirror_angle_groups
			else {
				return;
			};

			[old_bounds_min, old_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);

			transform_utils::VectorModificationState { subpaths, mirror_angle_groups }.modify(modification);

			[new_bounds_min, new_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);
		});

		self.update_bounds([old_bounds_min, old_bounds_max], [new_bounds_min, new_bounds_max]);
	}

	fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		self.modify_inputs("Brush", false, |inputs, _node_id, _metadata| {
			inputs[2] = NodeInput::value(TaggedValue::BrushStrokes(strokes), false);
		});
	}

	fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		self.modify_inputs("Artboard", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::IVec2(location), false);
			inputs[2] = NodeInput::value(TaggedValue::IVec2(dimensions), false);
		});
	}

	fn delete_layer(&mut self, id: NodeId) {
		let Some(node) = self.network.nodes.get(&id) else {
			warn!("Deleting layer node that does not exist");
			return;
		};

		LayerNodeIdentifier::new(id, self.network).delete(self.document_metadata);

		let new_input = node.inputs[7].clone();

		for post_node in self.outwards_links.get(&id).unwrap_or(&Vec::new()) {
			let Some(node) = self.network.nodes.get_mut(post_node) else {
				continue;
			};

			for input in &mut node.inputs {
				if let NodeInput::Node { node_id, .. } = input {
					if *node_id == id {
						*input = new_input.clone();
					}
				}
			}
		}

		let mut delete_nodes = vec![id];
		for (_node, id) in self.network.primary_flow_from_opt(Some(id)) {
			if self.outwards_links.get(&id).is_some_and(|outwards| outwards.len() == 1) {
				delete_nodes.push(id);
			}
		}

		for node_id in &delete_nodes {
			self.network.nodes.remove(node_id);
		}
		self.responses.add(self.document_metadata.retain_selected_nodes(|id| !delete_nodes.contains(id)));

		self.responses.add(DocumentMessage::DocumentStructureChanged);
		self.responses.add(NodeGraphMessage::SendGraph { should_rerender: true });
	}
}

impl MessageHandler<GraphOperationMessage, (&mut Document, &mut NodeGraphMessageHandler)> for GraphOperationMessageHandler {
	fn process_message(&mut self, message: GraphOperationMessage, responses: &mut VecDeque<Message>, (document, node_graph): (&mut Document, &mut NodeGraphMessageHandler)) {
		match message {
			GraphOperationMessage::FillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.fill_set(fill);
				} else {
					responses.add(Operation::SetLayerFill { path: layer, fill });
				}
			}
			GraphOperationMessage::UpdateBounds { layer, old_bounds, new_bounds } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.update_bounds(old_bounds, new_bounds);
				}
			}
			GraphOperationMessage::StrokeSet { layer, stroke } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.stroke_set(stroke);
				} else {
					responses.add(Operation::SetLayerStroke { path: layer, stroke });
				}
			}
			GraphOperationMessage::TransformChange {
				layer,
				transform,
				transform_in,
				skip_rerender,
			} => {
				let parent_transform = document.metadata.document_to_viewport * document.multiply_transforms(&layer[..layer.len() - 1]).unwrap_or_default();
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.transform_change(transform, transform_in, parent_transform, bounds, skip_rerender);
				}

				let transform = transform.to_cols_array();
				responses.add(match transform_in {
					TransformIn::Local => Operation::TransformLayer { path: layer, transform },
					TransformIn::Scope { scope } => {
						let scope = scope.to_cols_array();
						Operation::TransformLayerInScope { path: layer, transform, scope }
					}
					TransformIn::Viewport => Operation::TransformLayerInViewport { path: layer, transform },
				});
			}
			GraphOperationMessage::TransformSet {
				layer,
				transform,
				transform_in,
				skip_rerender,
			} => {
				let parent_transform = document.metadata.document_to_viewport * document.multiply_transforms(&layer[..layer.len() - 1]).unwrap_or_default();
				let current_transform = Some(document.metadata.transform_to_viewport(LayerNodeIdentifier::new(*layer.last().unwrap(), &document.document_network)));
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, current_transform, bounds, skip_rerender);
				}
				let transform = transform.to_cols_array();
			}
			GraphOperationMessage::TransformSetPivot { layer, pivot } => {
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.pivot_set(pivot, bounds);
				}
			}
			GraphOperationMessage::Vector { layer, modification } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.vector_modify(modification);
				}
			}
			GraphOperationMessage::Brush { layer, strokes } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&layer, document, node_graph, responses) {
					modify_inputs.brush_modify(strokes);
				}
			}
			GraphOperationMessage::NewArtboard { id, artboard } => {
				let mut modify_inputs = ModifyInputsContext::new(document, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer(id, modify_inputs.network.original_outputs()[0].node_id, 0) {
					modify_inputs.insert_artboard(artboard, layer);
				}
			}
			GraphOperationMessage::NewBitmapLayer { id, image_frame } => {
				let mut modify_inputs = ModifyInputsContext::new(document, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer(id, modify_inputs.network.original_outputs()[0].node_id, 0) {
					modify_inputs.insert_image_data(image_frame, layer);
				}
			}
			GraphOperationMessage::NewVectorLayer { id, subpaths } => {
				let mut modify_inputs = ModifyInputsContext::new(document, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer(id, modify_inputs.network.original_outputs()[0].node_id, 0) {
					modify_inputs.insert_vector_data(subpaths, layer);
				}
			}
			GraphOperationMessage::NewTextLayer { id, text, font, size } => {
				let mut modify_inputs = ModifyInputsContext::new(document, node_graph, responses);
				if let Some(layer) = modify_inputs.create_layer(id, modify_inputs.network.original_outputs()[0].node_id, 0) {
					modify_inputs.insert_text(text, font, size, layer);
				}
			}
			GraphOperationMessage::ResizeArtboard { id, location, dimensions } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new_layer(&[id], document, node_graph, responses) {
					modify_inputs.resize_artboard(location, dimensions);
				}
			}
			GraphOperationMessage::DeleteLayer { id } => {
				let mut modify_inputs = ModifyInputsContext::new(document, node_graph, responses);
				modify_inputs.delete_layer(id);
			}
			GraphOperationMessage::ClearArtboards => {
				let mut modify_inputs = ModifyInputsContext::new(document, node_graph, responses);
				let artboard_nodes = modify_inputs.network.nodes.iter().filter(|(_, node)| node.name == "Artboard").map(|(id, _)| *id).collect::<Vec<_>>();
				for id in artboard_nodes {
					modify_inputs.delete_layer(id);
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GraphOperationMessage; )
	}
}
