use crate::messages::prelude::*;

use document_legacy::document::Document;
use document_legacy::{LayerId, Operation};
use glam::DAffine2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, NodeId, NodeInput, NodeNetwork};
use graphene_core::vector::style::{Fill, FillType, Stroke};
use transform_utils::LayerBounds;

use super::resolve_document_node_type;

mod transform_utils;

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
		document.layer_mut(&layer).ok().and_then(|layer| layer.as_node_graph_mut().ok()).map(|network| Self {
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
	/// Insert a new node and modify the inputs
	fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
		let output_node_id = self.network.outputs[0].node_id;
		let Some(output_node) = self.network.nodes.get_mut(&output_node_id) else {
			warn!("Output node doesn't exist");
			return;
		};

		let metadata = output_node.metadata.clone();
		let new_input = output_node.inputs[0].clone();
		let node_id = generate_uuid();
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
	fn modify_inputs(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>)) {
		let node_id = self.network.primary_flow().find(|(node, _)| node.name == name).map(|(_, id)| id);
		if let Some(node_id) = node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		} else {
			self.modify_new_node(name, update_input);
		}
		self.node_graph.layer_path = Some(self.layer.to_vec());
		self.node_graph.nested_path.clear();
		self.responses.add(PropertiesPanelMessage::ResendActiveProperties);
		self.responses.add(DocumentMessage::NodeGraphFrameGenerate);
	}
	fn fill_set(&mut self, fill: Fill) {
		self.modify_inputs("Fill", |inputs| {
			let fill_type = match fill {
				Fill::None => FillType::None,
				Fill::Solid(_) => FillType::Solid,
				Fill::Gradient(_) => FillType::Gradient,
			};
			inputs[1] = NodeInput::value(TaggedValue::FillType(fill_type), false);
			if let Fill::Solid(color) = fill {
				inputs[2] = NodeInput::value(TaggedValue::Color(color), false)
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
		self.modify_inputs("Stroke", |inputs| {
			inputs[1] = NodeInput::value(TaggedValue::Color(stroke.color.unwrap_or_default()), false);
			inputs[2] = NodeInput::value(TaggedValue::F64(stroke.weight), false);
			inputs[3] = NodeInput::value(TaggedValue::VecF32(stroke.dash_lengths), false);
			inputs[4] = NodeInput::value(TaggedValue::F64(stroke.dash_offset), false);
			inputs[5] = NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false);
			inputs[6] = NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false);
			inputs[7] = NodeInput::value(TaggedValue::F64(stroke.line_join_miter_limit), false);
		});
	}

	fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, bounds: LayerBounds) {
		self.modify_inputs("Transform", |inputs| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(bounds.local_pivot(inputs));
			let transform = to.inverse() * pivot.inverse() * transform * pivot * to * layer_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}
	fn transform_set(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, bounds: LayerBounds) {
		self.modify_inputs("Transform", |inputs| {
			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(bounds.local_pivot(inputs));
			let transform = to.inverse() * pivot.inverse() * transform * pivot;
			transform_utils::update_transform(inputs, transform);
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

			GraphOperationMessage::StrokeSet { layer, stroke } => {
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.stroke_set(stroke);
				} else {
					responses.add(Operation::SetLayerStroke { path: layer, stroke });
				}
			}

			GraphOperationMessage::TransformChange { layer, transform, transform_in } => {
				let parent_transform = document.multiply_transforms(&layer[..layer.len() - 1]).unwrap_or_default();
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.transform_change(transform, transform_in, parent_transform, bounds);
				} else {
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
			}
			GraphOperationMessage::TransformSet { layer, transform, transform_in } => {
				let parent_transform = document.multiply_transforms(&layer[..layer.len() - 1]).unwrap_or_default();
				let bounds = LayerBounds::new(document, &layer);
				if let Some(mut modify_inputs) = ModifyInputsContext::new(&layer, document, node_graph, responses) {
					modify_inputs.transform_set(transform, transform_in, parent_transform, bounds);
				} else {
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
			}

			GraphOperationMessage::Vector { layer: _, modification: _ } => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(GraphOperationMessage; )
	}
}
