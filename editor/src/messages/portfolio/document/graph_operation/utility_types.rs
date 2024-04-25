use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, FillType, Stroke};
use graphene_core::{Artboard, Color};
use graphene_std::vector::ManipulatorPointId;

use glam::{DAffine2, DVec2, IVec2};

use super::transform_utils::{self, LayerBounds};

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformIn {
	Local,
	Scope { scope: DAffine2 },
	Viewport,
}

type ManipulatorGroup = bezier_rs::ManipulatorGroup<ManipulatorGroupId>;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorDataModification {
	AddEndManipulatorGroup { subpath_index: usize, manipulator_group: ManipulatorGroup },
	AddManipulatorGroup { manipulator_group: ManipulatorGroup, after_id: ManipulatorGroupId },
	AddStartManipulatorGroup { subpath_index: usize, manipulator_group: ManipulatorGroup },
	RemoveManipulatorGroup { id: ManipulatorGroupId },
	RemoveManipulatorPoint { point: ManipulatorPointId },
	SetClosed { index: usize, closed: bool },
	SetManipulatorColinearHandlesState { id: ManipulatorGroupId, colinear: bool },
	SetManipulatorPosition { point: ManipulatorPointId, position: DVec2 },
	ToggleManipulatorColinearHandlesState { id: ManipulatorGroupId },
	UpdateSubpaths { subpaths: Vec<Subpath<ManipulatorGroupId>> },
}

pub struct ModifyInputsContext<'a> {
	pub document_metadata: &'a mut DocumentMetadata,
	pub document_network: &'a mut NodeNetwork,
	pub node_graph: &'a mut NodeGraphMessageHandler,
	pub responses: &'a mut VecDeque<Message>,
	pub outwards_links: HashMap<NodeId, Vec<NodeId>>,
	pub layer_node: Option<NodeId>,
}

impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	pub fn new(document_network: &'a mut NodeNetwork, document_metadata: &'a mut DocumentMetadata, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			outwards_links: document_network.collect_outwards_links(),
			document_network,
			node_graph,
			responses,
			layer_node: None,
			document_metadata,
		}
	}

	pub fn new_with_layer(
		id: NodeId,
		document_network: &'a mut NodeNetwork,
		document_metadata: &'a mut DocumentMetadata,
		node_graph: &'a mut NodeGraphMessageHandler,
		responses: &'a mut VecDeque<Message>,
	) -> Option<Self> {
		let mut document = Self::new(document_network, document_metadata, node_graph, responses);

		let mut id = id;
		while !document.document_network.nodes.get(&id)?.is_layer {
			id = document.outwards_links.get(&id)?.first().copied()?;
		}

		document.layer_node = Some(id);
		Some(document)
	}

	/// Updates the input of an existing node
	pub fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let document_node = self.document_network.nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs, node_id, self.document_metadata);
	}

	pub fn insert_between(
		&mut self,
		id: NodeId,
		mut new_node: DocumentNode,
		new_node_input: NodeInput,
		new_node_input_index: usize,
		post_node_id: NodeId,
		post_node_input: NodeInput,
		post_node_input_index: usize,
		shift_upstream: IVec2,
	) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&id), "Creating already existing node");
		let pre_node = self.document_network.nodes.get_mut(&new_node_input.as_node().expect("Input should reference a node"))?;
		new_node.metadata.position = pre_node.metadata.position;

		let post_node = self.document_network.nodes.get_mut(&post_node_id)?;
		new_node.inputs[new_node_input_index] = new_node_input;
		post_node.inputs[post_node_input_index] = post_node_input;

		self.document_network.nodes.insert(id, new_node);

		self.shift_upstream(id, shift_upstream);

		Some(id)
	}

	pub fn insert_node_before(&mut self, new_id: NodeId, node_id: NodeId, input_index: usize, mut document_node: DocumentNode, offset: IVec2) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&new_id), "Creating already existing node");

		let post_node = self.document_network.nodes.get_mut(&node_id)?;
		post_node.inputs[input_index] = NodeInput::node(new_id, 0);
		document_node.metadata.position = post_node.metadata.position + offset;
		self.document_network.nodes.insert(new_id, document_node);

		Some(new_id)
	}

	pub fn create_layer(&mut self, new_id: NodeId, output_node_id: NodeId, skip_layer_nodes: usize) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&new_id), "Creating already existing layer");

		// Get the node which the new layer will output to (post node). First check if the output_node_id is the Output node, and set the output_node_id to the top most artboard,
		// if there is one. Then skip layers based on skip_layer_nodes from the post_node.
		// TODO: Smarter placement of layers into artboards https://github.com/GraphiteEditor/Graphite/issues/1507
		let mut post_node_id: NodeId = output_node_id;
		let mut post_node_input_index = 1; // Assume post node is a layer type.
		if post_node_id == NodeId(0) {
			post_node_input_index = 0; // Output node input index.
						   // Check if an artboard is connected, and switch post node to the artboard.
			if let Some(NodeInput::Node { node_id, .. }) = &self.document_network.nodes.get(&post_node_id).expect("Output node should always exist").inputs.get(0) {
				let input_node = self.document_network.nodes.get(&node_id).expect("First input node should exist");
				if input_node.is_artboard() {
					post_node_id = *node_id;
					post_node_input_index = 1; //Children for artboards connect to the second ("Over") input.
				}
			}
		}
		// Skip layers based on skip_layer_nodes, which inserts the new layer at a certain index of the layer stack.
		//			-> Post node
		//		⬆️		if skip_layer_nodes == 0, insert new layer here
		//	->	Layer 	stack_index: 0
		//      ⬆️		if skip_layer_nodes == 1, insert new layer here
		//  -> 	Layer	stack_index: 1
		for _ in 0..skip_layer_nodes {
			let next_node_in_stack_id = self
				.document_network
				.nodes
				.get(&post_node_id)
				.expect("Post node should always exist")
				.inputs
				.get(post_node_input_index)
				.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id.clone()) } else { None });

			if let Some(mut next_node_in_stack_id) = next_node_in_stack_id {
				let next_node_in_stack = self.document_network.nodes.get(&next_node_in_stack_id).expect("Stack node should always exist");

				//If an NLN (non layer node) is in the stack, create a Layer node for it to feed into
				if !next_node_in_stack.is_layer {
					let nln_layer_id = NodeId(generate_uuid());
					let nln_layer_node = resolve_document_node_type("Merge").expect("Merge node").default_document_node();
					let nln_layer_input = NodeInput::node(next_node_in_stack_id, 0);
					let nln_layer_input_index = 1; //Input NLN node to the "Over" for the new layer
					let post_node_input = NodeInput::node(nln_layer_id, 0);
					// Add the NLN layer between the NLN and post_node. The new layer will be added between this layer and the post_node
					next_node_in_stack_id = self
						.insert_between(
							nln_layer_id,
							nln_layer_node,
							nln_layer_input,
							nln_layer_input_index,
							post_node_id,
							post_node_input,
							post_node_input_index,
							IVec2::new(-8, 3),
						)
						.expect("Could not create layer node for NLN")
				}

				post_node_id = next_node_in_stack_id;
				post_node_input_index = 0; //Input as a sibling to the Layer node above
			} else {
				log::info!("Error creating layer: skip_layer_nodes index out of bounds");
				return None;
			};
		}

		let post_node = self.document_network.nodes.get(&post_node_id).expect("Post node id should always refer to a node");
		let pre_node_id = post_node
			.inputs
			.get(post_node_input_index)
			.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id.clone()) } else { None });

		if let Some(mut pre_node_id) = pre_node_id {
			// A new layer needs to be inserted between nodes. Post node can either be a parent or sibling. Siblings are part of the same stack.
			// The follow diagrams are for a parent post node, but same concept applies for sibling post node (which happens if skip_layer_nodes is > 0).
			// Before
			// 				 	-> Post node
			//				⬆️
			//			->	Old Layer
			// After
			//					-> Post node
			//				⬆️
			//			->	New Layer (id: new_id)
			//				⬆️
			//			->	Old Layer
			//
			// Before
			// 		    	NLN	-> Post node
			// After
			// 		          	-> Post node
			//              ⬆️
			// 			-> 	New Layer (id: new_id)
			//              ⬆️
			//      NLN	-> 	New Layer (id: random_id)

			// Pre_node will never be an artboard, since all artboards must be connected to root, and output node will be the artboard if it exists.
			let pre_node = self.document_network.nodes.get(&pre_node_id).expect("Pre node id should always refer to a node");

			if !pre_node.is_layer {
				// If pre_node is a non-layer node (NLN), create a layer for this node, and set pre_node to that layer.
				let nln_layer_id = NodeId(generate_uuid());
				let nln_layer_node = resolve_document_node_type("Merge").expect("Merge node").default_document_node();
				let nln_layer_input = NodeInput::node(pre_node_id, 0);
				let nln_layer_input_index = 1;
				let post_node_input = NodeInput::node(nln_layer_id, 0);
				// Add the NLN layer between the NLN and post_node. The new layer will be added between this layer and the post_node
				self.insert_between(
					nln_layer_id,
					nln_layer_node,
					nln_layer_input,
					nln_layer_input_index,
					post_node_id,
					post_node_input,
					post_node_input_index,
					IVec2::new(-8, 3),
				);

				pre_node_id = nln_layer_id;
			};
			let new_layer_node = resolve_document_node_type("Merge").expect("Merge node").default_document_node();
			self.insert_between(
				new_id,
				new_layer_node,
				NodeInput::node(pre_node_id, 0),
				0, // Pre_node is a sibling so it connects to the first input.
				post_node_id,
				NodeInput::node(new_id, 0),
				post_node_input_index,
				IVec2::new(0, 3),
			);
		} else {
			let new_layer_node = resolve_document_node_type("Merge").expect("Merge node").default_document_node();
			self.insert_node_before(new_id, post_node_id, post_node_input_index, new_layer_node, IVec2::new(-8, 3));
		}

		//TODO: Is this necessary? When load_structure is called it resets these changes and builds the structure from scratch.
		let parent = LayerNodeIdentifier::new(post_node_id, self.document_network);
		let new_child = LayerNodeIdentifier::new(new_id, self.document_network);
		// Check if new layer is a child or sibling to post_node
		if post_node_input_index == 0 {
			parent.add_after(self.document_metadata, new_child);
		} else {
			parent.push_front_child(self.document_metadata, new_child);
		}

		Some(new_id)
	}

	pub fn create_layer_with_insert_index(&mut self, new_id: NodeId, insert_index: isize, parent: LayerNodeIdentifier) -> Option<NodeId> {
		let skip_layer_nodes = if insert_index < 0 { (-1 - insert_index) as usize } else { insert_index as usize };

		let output_node_id = if parent == LayerNodeIdentifier::ROOT {
			self.document_network.original_outputs()[0].node_id
		} else {
			parent.to_node()
		};
		self.create_layer(new_id, output_node_id, skip_layer_nodes)
	}

	/// Creates an artboard that outputs to the output node.
	pub fn create_artboard(&mut self, new_id: NodeId, artboard: Artboard) -> Option<NodeId> {
		let output_node_id = self.document_network.original_outputs()[0].node_id;
		let mut shift = IVec2::new(0, 3);

		let artboard_node = resolve_document_node_type("Artboard").expect("Node").to_document_node_default_inputs(
			[
				Some(NodeInput::value(TaggedValue::ArtboardGroup(graphene_std::ArtboardGroup::EMPTY), true)),
				Some(NodeInput::value(TaggedValue::GraphicGroup(graphene_core::GraphicGroup::EMPTY), true)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
				Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
				Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
			],
			Default::default(),
		);

		// Get node that feeds into output. If it exists, connect the new artboard node in between. Else connect the new artboard directly to output.
		let output_node_primary_input = self.document_network.nodes.get(&output_node_id)?.inputs.get(0);
		let created_node_id = if let NodeInput::Node { node_id, .. } = &output_node_primary_input? {
			let pre_node = self.document_network.nodes.get(node_id)?;
			// If the node currently connected the Output is an artboard, connect to input 0 (Artboards input) of the new artboard. Else connect to the Over input.
			let artboard_input_index = if pre_node.is_artboard() { 0 } else { 1 };

			self.insert_between(
				new_id,
				artboard_node,
				NodeInput::node(*node_id, 0),
				artboard_input_index,
				output_node_id,
				NodeInput::node(new_id, 0),
				0,
				shift,
			)
		} else {
			shift = IVec2::new(-8, 3);
			self.insert_node_before(new_id, output_node_id, 0, artboard_node, shift)
		};

		if let Some(new_id) = created_node_id {
			let new_child = LayerNodeIdentifier::new_unchecked(new_id);
			LayerNodeIdentifier::ROOT.push_front_child(self.document_metadata, new_child);
		}
		//self.responses.add(NodeGraphMessage::RunDocumentGraph);
		created_node_id
	}
	pub fn insert_vector_data(&mut self, subpaths: Vec<Subpath<ManipulatorGroupId>>, layer: NodeId) {
		let shape = {
			let node_type = resolve_document_node_type("Shape").expect("Shape node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))], Default::default())
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_before(stroke_id, layer, 1, stroke, IVec2::new(-8, 0));
		let fill_id = NodeId(generate_uuid());
		self.insert_node_before(fill_id, stroke_id, 0, fill, IVec2::new(-8, 0));
		let transform_id = NodeId(generate_uuid());
		self.insert_node_before(transform_id, fill_id, 0, transform, IVec2::new(-8, 0));
		let shape_id = NodeId(generate_uuid());
		self.insert_node_before(shape_id, transform_id, 0, shape, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_text(&mut self, text: String, font: Font, size: f64, layer: NodeId) {
		let text = resolve_document_node_type("Text").expect("Text node does not exist").to_document_node(
			[
				NodeInput::Network(graph_craft::concrete!(graphene_std::wasm_application_io::WasmEditorApi)),
				NodeInput::value(TaggedValue::String(text), false),
				NodeInput::value(TaggedValue::Font(font), false),
				NodeInput::value(TaggedValue::F64(size), false),
			],
			Default::default(),
		);
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_before(stroke_id, layer, 1, stroke, IVec2::new(-8, 0));
		let fill_id = NodeId(generate_uuid());
		self.insert_node_before(fill_id, stroke_id, 0, fill, IVec2::new(-8, 0));
		let transform_id = NodeId(generate_uuid());
		self.insert_node_before(transform_id, fill_id, 0, transform, IVec2::new(-8, 0));
		let text_id = NodeId(generate_uuid());
		self.insert_node_before(text_id, transform_id, 0, text, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_image_data(&mut self, image_frame: ImageFrame<Color>, layer: NodeId) {
		let image = {
			let node_type = resolve_document_node_type("Image").expect("Image node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))], Default::default())
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();

		let transform_id = NodeId(generate_uuid());
		self.insert_node_before(transform_id, layer, 1, transform, IVec2::new(-8, 0));

		let image_id = NodeId(generate_uuid());
		self.insert_node_before(image_id, transform_id, 0, image, IVec2::new(-8, 0));

		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn shift_upstream(&mut self, node_id: NodeId, shift: IVec2) {
		let mut shift_nodes = HashSet::new();
		let mut stack = vec![node_id];
		while let Some(node_id) = stack.pop() {
			let Some(node) = self.document_network.nodes.get(&node_id) else { continue };
			for input in &node.inputs {
				let NodeInput::Node { node_id, .. } = input else { continue };
				if shift_nodes.insert(*node_id) {
					stack.push(*node_id);
				}
			}
		}

		for node_id in shift_nodes {
			if let Some(node) = self.document_network.nodes.get_mut(&node_id) {
				node.metadata.position += shift;
			}
		}
	}

	/// Inserts a new node and modifies the inputs
	pub fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let output_node_id = self.layer_node.unwrap_or(self.document_network.exports[0].node_id);
		let Some(output_node) = self.document_network.nodes.get_mut(&output_node_id) else {
			warn!("Output node doesn't exist");
			return;
		};

		let input_index = if output_node.is_layer { 1 } else { 0 };
		let metadata = output_node.metadata.clone();
		let new_input = output_node.inputs.get(input_index).cloned().filter(|input| input.as_node().is_some());
		let node_id = NodeId(generate_uuid());

		output_node.inputs[input_index] = NodeInput::node(node_id, 0);

		let Some(node_type) = resolve_document_node_type(name) else {
			warn!("Node type \"{name}\" doesn't exist");
			return;
		};
		let mut new_document_node = node_type.to_document_node_default_inputs([new_input], metadata);
		update_input(&mut new_document_node.inputs, node_id, self.document_metadata);
		self.document_network.nodes.insert(node_id, new_document_node);

		let upstream_nodes = self
			.document_network
			.upstream_flow_back_from_nodes(vec![node_id], graph_craft::document::FlowType::HorizontalFlow)
			.map(|(_, id)| id)
			.collect::<Vec<_>>();
		for node_id in upstream_nodes {
			let Some(node) = self.document_network.nodes.get_mut(&node_id) else { continue };
			node.metadata.position.x -= 8;
		}
	}

	/// Changes the inputs of a specific node
	pub fn modify_inputs(&mut self, name: &'static str, skip_rerender: bool, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_node_id = self
			.document_network
			.upstream_flow_back_from_nodes(
				self.layer_node
					.map_or_else(|| self.document_network.exports.iter().map(|output| output.node_id).collect(), |id| vec![id]),
				graph_craft::document::FlowType::HorizontalFlow,
			)
			.find(|(node, _)| node.name == name)
			.map(|(_, id)| id);
		if let Some(node_id) = existing_node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		} else {
			self.modify_new_node(name, update_input);
		}

		self.node_graph.network.clear();
		self.responses.add(PropertiesPanelMessage::Refresh);

		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	/// Changes the inputs of a all of the existing instances of a node name
	pub fn modify_all_node_inputs(&mut self, name: &'static str, skip_rerender: bool, mut update_input: impl FnMut(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_nodes: Vec<_> = self
			.document_network
			.upstream_flow_back_from_nodes(
				self.layer_node
					.map_or_else(|| self.document_network.exports.iter().map(|output| output.node_id).collect(), |id| vec![id]),
				graph_craft::document::FlowType::HorizontalFlow,
			)
			.filter(|(node, _)| node.name == name)
			.map(|(_, id)| id)
			.collect();
		for existing_node_id in existing_nodes {
			self.modify_existing_node_inputs(existing_node_id, &mut update_input);
		}

		self.responses.add(PropertiesPanelMessage::Refresh);

		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		} else {
			// Code was removed from here which cleared the frame
		}
	}

	pub fn fill_set(&mut self, fill: Fill) {
		self.modify_inputs("Fill", false, |inputs, _node_id, _metadata| {
			let fill_type = match fill {
				Fill::None | Fill::Solid(_) => FillType::Solid,
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

	pub fn opacity_set(&mut self, opacity: f64) {
		self.modify_inputs("Opacity", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::F64(opacity * 100.), false);
		});
	}

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		self.modify_inputs("Blend Mode", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::BlendMode(blend_mode), false);
		});
	}

	pub fn stroke_set(&mut self, stroke: Stroke) {
		self.modify_inputs("Stroke", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::OptionalColor(stroke.color), false);
			inputs[2] = NodeInput::value(TaggedValue::F64(stroke.weight), false);
			inputs[3] = NodeInput::value(TaggedValue::VecF64(stroke.dash_lengths), false);
			inputs[4] = NodeInput::value(TaggedValue::F64(stroke.dash_offset), false);
			inputs[5] = NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false);
			inputs[6] = NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false);
			inputs[7] = NodeInput::value(TaggedValue::F64(stroke.line_join_miter_limit), false);
		});
	}

	pub fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, bounds: LayerBounds, skip_rerender: bool) {
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

	pub fn transform_set(&mut self, mut transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, current_transform: Option<DAffine2>, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, node_id, metadata| {
			let upstream_transform = metadata.upstream_transform(node_id);

			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(upstream_transform.transform_point2(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs))));

			if current_transform
				.filter(|transform| transform.matrix2.determinant() != 0. && upstream_transform.matrix2.determinant() != 0.)
				.is_some()
			{
				transform *= upstream_transform.inverse();
			}
			let final_transform = pivot.inverse() * to.inverse() * transform * pivot;
			transform_utils::update_transform(inputs, final_transform);
		});
	}

	pub fn pivot_set(&mut self, new_pivot: DVec2, bounds: LayerBounds) {
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

	pub fn update_bounds(&mut self, [old_bounds_min, old_bounds_max]: [DVec2; 2], [new_bounds_min, new_bounds_max]: [DVec2; 2]) {
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

	pub fn vector_modify(&mut self, modification: VectorDataModification) {
		let [mut old_bounds_min, mut old_bounds_max] = [DVec2::ZERO, DVec2::ONE];
		let [mut new_bounds_min, mut new_bounds_max] = [DVec2::ZERO, DVec2::ONE];
		let mut empty = false;

		self.modify_inputs("Shape", false, |inputs, _node_id, _metadata| {
			let [subpaths, colinear_manipulators] = inputs.as_mut_slice() else {
				panic!("Shape does not have both `subpath` and `colinear_manipulators` inputs");
			};

			let NodeInput::Value {
				tagged_value: TaggedValue::Subpaths(subpaths),
				..
			} = subpaths
			else {
				return;
			};
			let NodeInput::Value {
				tagged_value: TaggedValue::ManipulatorGroupIds(colinear_manipulators),
				..
			} = colinear_manipulators
			else {
				return;
			};

			[old_bounds_min, old_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);

			transform_utils::VectorModificationState { subpaths, colinear_manipulators }.modify(modification);
			empty = !subpaths.iter().any(|subpath| !subpath.is_empty());

			[new_bounds_min, new_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);
		});

		self.update_bounds([old_bounds_min, old_bounds_max], [new_bounds_min, new_bounds_max]);
		if empty {
			if let Some(id) = self.layer_node {
				self.responses.add(DocumentMessage::DeleteLayer { id })
			}
		}
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		self.modify_inputs("Brush", false, |inputs, _node_id, _metadata| {
			inputs[2] = NodeInput::value(TaggedValue::BrushStrokes(strokes), false);
		});
	}

	pub fn resize_artboard(&mut self, mut location: IVec2, mut dimensions: IVec2) {
		self.modify_inputs("Artboard", false, |inputs, _node_id, _metadata| {
			if dimensions.x < 0 {
				dimensions.x = -dimensions.x;
				location.x -= dimensions.x;
			}
			if dimensions.y < 0 {
				dimensions.y = -dimensions.y;
				location.y -= dimensions.y;
			}
			inputs[2] = NodeInput::value(TaggedValue::IVec2(location), false);
			inputs[3] = NodeInput::value(TaggedValue::IVec2(dimensions), false);
		});
	}
}
