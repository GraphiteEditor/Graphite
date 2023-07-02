use super::{resolve_document_node_type, VectorDataModification};
use crate::messages::prelude::*;

use document_legacy::document::Document;
use document_legacy::{LayerId, Operation};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork, NodeOutput};
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, FillType, Stroke};
use graphene_core::Artboard;
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
}
impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	fn new(layer: &'a [LayerId], document: &'a mut Document, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Option<Self> {
		document.layer_mut(layer).ok().and_then(|layer| layer.as_layer_network_mut().ok()).map(|network| Self {
			outwards_links: network.collect_outwards_links(),
			network,
			node_graph,
			responses,
			layer,
			layer_node: None,
		})
	}

	/// Get the node network from the document
	fn new_doc(document: &'a mut Document, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			outwards_links: document.document_network.collect_outwards_links(),
			network: &mut document.document_network,
			node_graph,
			responses,
			layer: &[],
			layer_node: None,
		}
	}

	fn locate_layer(&mut self, mut id: NodeId) -> Option<NodeId> {
		while self.network.nodes.get(&id)?.name != "Layer" {
			id = self.outwards_links.get(&id)?.first().copied()?;
		}
		self.layer_node = Some(id);
		Some(id)
	}

	/// Updates the input of an existing node
	fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
		let document_node = self.network.nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs);
	}

	pub fn insert_between(&mut self, pre: NodeOutput, post: NodeOutput, mut node: DocumentNode, input: usize, output: usize) -> Option<NodeId> {
		let id = generate_uuid();
		let pre_node = self.network.nodes.get_mut(&pre.node_id)?;
		node.metadata.position = pre_node.metadata.position;

		let post_node = self.network.nodes.get_mut(&post.node_id)?;
		node.inputs[input] = NodeInput::node(pre.node_id, pre.node_output_index);
		post_node.inputs[post.node_output_index] = NodeInput::node(id, output);

		self.network.nodes.insert(id, node);

		self.shift_upstream(id, IVec2::new(-8, 0));

		Some(id)
	}

	pub fn insert_layer_below(&mut self, node_id: NodeId, input_index: usize) -> Option<NodeId> {
		let layer_node = resolve_document_node_type("Layer").expect("Layer node");

		let new_id = generate_uuid();
		let post_node = self.network.nodes.get_mut(&node_id)?;
		post_node.inputs[input_index] = NodeInput::node(new_id, 0);
		let document_node = layer_node.to_document_node_default_inputs([], DocumentNodeMetadata::position(post_node.metadata.position + IVec2::new(0, 2)));

		self.network.nodes.insert(new_id, document_node);

		Some(new_id)
	}

	pub fn insert_node_before(&mut self, node_id: NodeId, input_index: usize, mut document_node: DocumentNode, offset: IVec2) -> Option<NodeId> {
		let new_id = generate_uuid();
		let post_node = self.network.nodes.get_mut(&node_id)?;

		post_node.inputs[input_index] = NodeInput::node(new_id, 0);
		document_node.metadata.position = post_node.metadata.position + offset;
		self.network.nodes.insert(new_id, document_node);

		Some(new_id)
	}

	pub fn create_layer(&mut self, output_node_id: NodeId) -> Option<NodeId> {
		let mut current_node = output_node_id;
		let mut input_index = 0;
		let mut current_input = &self.network.nodes.get(&current_node)?.inputs[input_index];
		info!("Got input");

		while let NodeInput::Node { node_id, output_index, .. } = current_input {
			let mut sibling_node = &self.network.nodes.get(node_id)?;
			info!("Sibling {}", sibling_node.name);
			if sibling_node.name == "Layer" {
				current_node = *node_id;
				input_index = 7;
				current_input = &self.network.nodes.get(&current_node)?.inputs[input_index];
			} else {
				// Insert a layer node between the output and the new
				let layer_node = resolve_document_node_type("Layer").expect("Layer node");
				let node = layer_node.to_document_node_default_inputs([], DocumentNodeMetadata::default());
				let node_id = self.insert_between(NodeOutput::new(*node_id, *output_index), NodeOutput::new(current_node, input_index), node, 0, 0)?;
				current_node = node_id;
				input_index = 7;
				current_input = &self.network.nodes.get(&current_node)?.inputs[input_index];
				info!("Fini insert");
			}
		}
		info!("Insert layer below");

		let layer_node = resolve_document_node_type("Layer").expect("Node").to_document_node_default_inputs([], Default::default());
		let layer_node = self.insert_node_before(current_node, input_index, layer_node, IVec2::new(0, 3))?;

		Some(layer_node)
	}

	fn insert_artboard(&mut self, artboard: Artboard, layer: NodeId) -> Option<NodeId> {
		let artboard_node = resolve_document_node_type("Artboard").expect("Node").to_document_node_default_inputs(
			[
				None,
				Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
				Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
			],
			Default::default(),
		);
		self.insert_node_before(layer, 0, artboard_node, IVec2::new(-8, 0))
	}

	fn shift_upstream(&mut self, node_id: NodeId, shift: IVec2) {
		let mut shift_nodes = HashSet::new();
		let mut stack = vec![node_id];
		while let Some(node) = stack.pop() {
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
	fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
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
		update_input(&mut new_document_node.inputs);
		self.network.nodes.insert(node_id, new_document_node);
	}

	/// Changes the inputs of a specific node
	fn modify_inputs(&mut self, name: &'static str, skip_rerender: bool, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
		let existing_node_id = self.network.primary_flow_from_opt(self.layer_node).find(|(node, _)| node.name == name).map(|(_, id)| id);
		if let Some(node_id) = existing_node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		} else {
			self.modify_new_node(name, update_input);
		}
		self.node_graph.update_layer_path(Some(self.layer.to_vec()), self.responses);
		self.node_graph.nested_path.clear();
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

	fn fill_set(&mut self, fill: Fill) {
		self.modify_inputs("Fill", false, |inputs| {
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
		self.modify_inputs("Stroke", false, |inputs| {
			inputs[1] = NodeInput::value(TaggedValue::OptionalColor(stroke.color), false);
			inputs[2] = NodeInput::value(TaggedValue::F64(stroke.weight), false);
			inputs[3] = NodeInput::value(TaggedValue::VecF32(stroke.dash_lengths), false);
			inputs[4] = NodeInput::value(TaggedValue::F64(stroke.dash_offset), false);
			inputs[5] = NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false);
			inputs[6] = NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false);
			inputs[7] = NodeInput::value(TaggedValue::F64(stroke.line_join_miter_limit), false);
		});
	}

	fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs)));
			let transform = pivot.inverse() * to.inverse() * transform * to * pivot * layer_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}

	fn transform_set(&mut self, mut transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, current_transform: Option<DAffine2>, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs| {
			let current_transform_node = transform_utils::get_current_transform(inputs);

			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs)));

			if let Some(current_transform) = current_transform.filter(|transform| transform.inverse().is_finite() && current_transform_node.inverse().is_finite()) {
				// this_transform * upstream_transforms = current_transform
				// So this_transform.inverse() * current_transform = upstream_transforms
				let upstream_transform = (pivot * current_transform_node * pivot.inverse()).inverse() * current_transform;
				// desired_final_transform = this_transform * upstream_transform
				// So this_transform = desired_final_transform * upstream_transform.inverse()
				transform = transform * upstream_transform.inverse();
			}

			let transform = pivot.inverse() * to.inverse() * transform * pivot;
			transform_utils::update_transform(inputs, transform);
		});
	}

	fn pivot_set(&mut self, new_pivot: DVec2, bounds: LayerBounds) {
		self.modify_inputs("Transform", false, |inputs| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let old_pivot_transform = DAffine2::from_translation(bounds.local_pivot(transform_utils::get_current_normalized_pivot(inputs)));
			let new_pivot_transform = DAffine2::from_translation(bounds.local_pivot(new_pivot));
			let transform = new_pivot_transform.inverse() * old_pivot_transform * layer_transform * old_pivot_transform.inverse() * new_pivot_transform;
			transform_utils::update_transform(inputs, transform);
			inputs[5] = NodeInput::value(TaggedValue::DVec2(new_pivot), false);
		});
	}

	fn update_bounds(&mut self, [old_bounds_min, old_bounds_max]: [DVec2; 2], [new_bounds_min, new_bounds_max]: [DVec2; 2]) {
		self.modify_inputs("Transform", false, |inputs| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let normalized_pivot = transform_utils::get_current_normalized_pivot(inputs);

			let old_layerspace_pivot = (old_bounds_max - old_bounds_min) * normalized_pivot + old_bounds_min;
			let new_layerspace_pivot = (new_bounds_max - new_bounds_min) * normalized_pivot + new_bounds_min;
			let new_pivot_transform = DAffine2::from_translation(new_layerspace_pivot);
			let old_pivot_transform = DAffine2::from_translation(old_layerspace_pivot);

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

		self.modify_inputs("Path Generator", false, |inputs| {
			let [subpaths, mirror_angle_groups] = inputs.as_mut_slice() else {
				panic!("Path generator does not have subpath and mirror angle inputs");
			};

			let NodeInput::Value {
				tagged_value: TaggedValue::Subpaths(subpaths),
				..
			} = subpaths else {
				return;
			};
			let NodeInput::Value {
				tagged_value: TaggedValue::ManipulatorGroupIds(mirror_angle_groups),
				..
			} = mirror_angle_groups else {
				return;
			};

			[old_bounds_min, old_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);

			transform_utils::VectorModificationState { subpaths, mirror_angle_groups }.modify(modification);

			[new_bounds_min, new_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);
		});

		self.update_bounds([old_bounds_min, old_bounds_max], [new_bounds_min, new_bounds_max]);
	}

	fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		self.modify_inputs("Brush", false, |inputs| {
			inputs[2] = NodeInput::value(TaggedValue::BrushStrokes(strokes), false);
		});
	}

	fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		self.modify_inputs("Artboard", false, |inputs| {
			inputs[1] = NodeInput::value(TaggedValue::IVec2(location), false);
			inputs[2] = NodeInput::value(TaggedValue::IVec2(dimensions), false);
		});
	}

	fn delete_layer(&mut self, id: NodeId) {
		let mut new_input = None;
		let post_node = self.outwards_links.get(&id).and_then(|links| links.first().copied());
		let mut delete_nodes = vec![id];
		for (node, id) in self.network.primary_flow_from_opt(Some(id)) {
			delete_nodes.push(id);
			if node.name == "Artboard" {
				new_input = Some(node.inputs[0].clone());
				break;
			}
		}

		for node_id in delete_nodes {
			self.network.nodes.remove(&node_id);
		}

		if let (Some(new_input), Some(post_node)) = (new_input, post_node) {
			if let Some(node) = self.network.nodes.get_mut(&post_node) {
				node.inputs[0] = new_input;
			}
		}
	}
}

impl MessageHandler<GraphOperationMessage, (&mut Document, &mut NodeGraphMessageHandler)> for GraphOperationMessageHandler {
	fn process_message(&mut self, message: GraphOperationMessage, responses: &mut VecDeque<Message>, (document, node_graph): (&mut Document, &mut NodeGraphMessageHandler)) {
		match message {
			GraphOperationMessage::FillSet { layer, fill } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.fill_set(fill);
				} else {
					responses.add(Operation::SetLayerFill { path: layer, fill });
				}
			}
			GraphOperationMessage::UpdateBounds { layer, old_bounds, new_bounds } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.update_bounds(old_bounds, new_bounds);
				}
			}
			GraphOperationMessage::StrokeSet { layer, stroke } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
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
				let parent_transform = document.multiply_transforms(&layer[..layer.len() - 1]).unwrap_or_default();
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
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
				let parent_transform = document.multiply_transforms(&layer[..layer.len() - 1]).unwrap_or_default();
				let current_transform = document.layer(&layer).ok().map(|layer| layer.transform);
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, current_transform, bounds, skip_rerender);
				}
				let transform = transform.to_cols_array();
				responses.add(match transform_in {
					TransformIn::Local => Operation::SetLayerTransform { path: layer, transform },
					TransformIn::Scope { scope } => {
						let scope = scope.to_cols_array();
						Operation::SetLayerTransformInScope { path: layer, transform, scope }
					}
					TransformIn::Viewport => Operation::SetLayerTransformInViewport { path: layer, transform },
				});
			}
			GraphOperationMessage::TransformSetPivot { layer, pivot } => {
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.pivot_set(pivot, bounds);
				}

				let pivot = pivot.into();
				responses.add(Operation::SetPivot { layer_path: layer, pivot });
			}
			GraphOperationMessage::Vector { layer, modification } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.vector_modify(modification);
				}
			}
			GraphOperationMessage::Brush { layer, strokes } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.brush_modify(strokes);
				}
			}
			GraphOperationMessage::NewArtboard { id, artboard } => {
				let mut modify_inputs = ModifyInputsContext::new_doc(document, node_graph, responses);
				info!("Gen artboard");
				if let Some(layer) = modify_inputs.create_layer(modify_inputs.network.outputs[0].node_id) {
					modify_inputs.insert_artboard(artboard, layer);
				}

				//modify_inputs.brush_modify(strokes);
			}
			GraphOperationMessage::ResizeArtboard { id, location, dimensions } => {
				let mut modify_inputs = ModifyInputsContext::new_doc(document, node_graph, responses);
				info!("Gen artboard");
				if let Some(layer) = modify_inputs.locate_layer(id) {
					modify_inputs.resize_artboard(location, dimensions);
				}
			}
			GraphOperationMessage::DeleteArtboard { id } => {
				let mut modify_inputs = ModifyInputsContext::new_doc(document, node_graph, responses);
				modify_inputs.delete_layer(id);
			}
			GraphOperationMessage::ClearArtboards => {
				let mut modify_inputs = ModifyInputsContext::new_doc(document, node_graph, responses);
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
