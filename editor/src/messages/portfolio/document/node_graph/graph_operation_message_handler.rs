use super::{resolve_document_node_type, VectorDataModification};
use crate::messages::prelude::*;

use document_legacy::document::Document;
use document_legacy::{LayerId, Operation};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, NodeId, NodeInput, NodeNetwork};
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, FillType, Stroke};
use transform_utils::LayerBounds;

use glam::{DAffine2, DVec2};

pub mod transform_utils;

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct GraphOperationMessageHandler;

struct ModifyInputsContext<'a> {
	network: &'a mut NodeNetwork,
	node_graph: &'a mut NodeGraphMessageHandler,
	responses: &'a mut VecDeque<Message>,
	layer: &'a [LayerId],
}
impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	fn new(layer: &'a [LayerId], document: &'a mut Document, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Option<Self> {
		document.layer_mut(layer).ok().and_then(|layer| layer.as_layer_network_mut().ok()).map(|network| Self {
			network,
			node_graph,
			responses,
			layer,
		})
	}

	/// Updates the input of an existing node
	fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
		let document_node = self.network.nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs);
	}

	/// Inserts a new node and modifies the inputs
	fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
		let output_node_id = self.network.outputs[0].node_id;
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
		let existing_node_id = self.network.primary_flow().find(|(node, _)| node.name == name).map(|(_, id)| id);
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
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GraphOperationMessage; )
	}
}
