// TODO: Eventually remove this document upgrade code
// This file contains lots of hacky code for upgrading old documents to the new format

use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
use crate::messages::prelude::DocumentMessageHandler;
use bezier_rs::Subpath;
use glam::IVec2;
use graph_craft::document::{DocumentNodeImplementation, NodeInput, value::TaggedValue};
use graphene_std::text::TypesettingConfig;
use graphene_std::uuid::NodeId;
use graphene_std::vector::style::{Fill, FillType, Gradient, PaintOrder, StrokeAlign};
use graphene_std::vector::{VectorData, VectorDataTable};
use std::collections::HashMap;

const TEXT_REPLACEMENTS: &[(&str, &str)] = &[
	("graphene_core::vector::vector_nodes::SamplePointsNode", "graphene_core::vector::SamplePolylineNode"),
	("graphene_core::vector::vector_nodes::SubpathSegmentLengthsNode", "graphene_core::vector::SubpathSegmentLengthsNode"),
];

const REPLACEMENTS: &[(&str, &str)] = &[
	("graphene_core::AddArtboardNode", "graphene_core::graphic_element::AppendArtboardNode"),
	("graphene_core::ConstructArtboardNode", "graphene_core::graphic_element::ToArtboardNode"),
	("graphene_core::ToGraphicElementNode", "graphene_core::graphic_element::ToElementNode"),
	("graphene_core::ToGraphicGroupNode", "graphene_core::graphic_element::ToGroupNode"),
	("graphene_core::ops::MathNode", "graphene_math_nodes::MathNode"),
	("graphene_core::ops::AddNode", "graphene_math_nodes::AddNode"),
	("graphene_core::ops::SubtractNode", "graphene_math_nodes::SubtractNode"),
	("graphene_core::ops::MultiplyNode", "graphene_math_nodes::MultiplyNode"),
	("graphene_core::ops::DivideNode", "graphene_math_nodes::DivideNode"),
	("graphene_core::ops::ModuloNode", "graphene_math_nodes::ModuloNode"),
	("graphene_core::ops::ExponentNode", "graphene_math_nodes::ExponentNode"),
	("graphene_core::ops::RootNode", "graphene_math_nodes::RootNode"),
	("graphene_core::ops::LogarithmNode", "graphene_math_nodes::LogarithmNode"),
	("graphene_core::ops::SineNode", "graphene_math_nodes::SineNode"),
	("graphene_core::ops::CosineNode", "graphene_math_nodes::CosineNode"),
	("graphene_core::ops::TangentNode", "graphene_math_nodes::TangentNode"),
	("graphene_core::ops::SineInverseNode", "graphene_math_nodes::SineInverseNode"),
	("graphene_core::ops::CosineInverseNode", "graphene_math_nodes::CosineInverseNode"),
	("graphene_core::ops::TangentInverseNode", "graphene_math_nodes::TangentInverseNode"),
	("graphene_core::ops::RandomNode", "graphene_math_nodes::RandomNode"),
	("graphene_core::ops::ToU32Node", "graphene_math_nodes::ToU32Node"),
	("graphene_core::ops::ToU64Node", "graphene_math_nodes::ToU64Node"),
	("graphene_core::ops::ToF64Node", "graphene_math_nodes::ToF64Node"),
	("graphene_core::ops::RoundNode", "graphene_math_nodes::RoundNode"),
	("graphene_core::ops::FloorNode", "graphene_math_nodes::FloorNode"),
	("graphene_core::ops::CeilingNode", "graphene_math_nodes::CeilingNode"),
	("graphene_core::ops::MinNode", "graphene_math_nodes::MinNode"),
	("graphene_core::ops::MaxNode", "graphene_math_nodes::MaxNode"),
	("graphene_core::ops::ClampNode", "graphene_math_nodes::ClampNode"),
	("graphene_core::ops::EqualsNode", "graphene_math_nodes::EqualsNode"),
	("graphene_core::ops::NotEqualsNode", "graphene_math_nodes::NotEqualsNode"),
	("graphene_core::ops::LessThanNode", "graphene_math_nodes::LessThanNode"),
	("graphene_core::ops::GreaterThanNode", "graphene_math_nodes::GreaterThanNode"),
	("graphene_core::ops::LogicalOrNode", "graphene_math_nodes::LogicalOrNode"),
	("graphene_core::ops::LogicalAndNode", "graphene_math_nodes::LogicalAndNode"),
	("graphene_core::ops::LogicalNotNode", "graphene_math_nodes::LogicalNotNode"),
	("graphene_core::ops::BoolValueNode", "graphene_math_nodes::BoolValueNode"),
	("graphene_core::ops::NumberValueNode", "graphene_math_nodes::NumberValueNode"),
	("graphene_core::ops::PercentageValueNode", "graphene_math_nodes::PercentageValueNode"),
	("graphene_core::ops::CoordinateValueNode", "graphene_math_nodes::CoordinateValueNode"),
	("graphene_core::ops::ColorValueNode", "graphene_math_nodes::ColorValueNode"),
	("graphene_core::ops::GradientValueNode", "graphene_math_nodes::GradientValueNode"),
	("graphene_core::ops::StringValueNode", "graphene_math_nodes::StringValueNode"),
	("graphene_core::ops::DotProductNode", "graphene_math_nodes::DotProductNode"),
	("graphene_core::ops::SizeOfNode", "graphene_core::debug::SizeOfNode"),
	("graphene_core::ops::SomeNode", "graphene_core::debug::SomeNode"),
	("graphene_core::ops::UnwrapNode", "graphene_core::debug::UnwrapNode"),
	("graphene_core::ops::CloneNode", "graphene_core::debug::CloneNode"),
	("graphene_core::ops::ExtractXyNode", "graphene_core::extract_xy::ExtractXyNode"),
	("graphene_core::logic::LogicAndNode", "graphene_core::ops::LogicAndNode"),
	("graphene_core::logic::LogicNotNode", "graphene_core::ops::LogicNotNode"),
	("graphene_core::logic::LogicOrNode", "graphene_core::ops::LogicOrNode"),
	("graphene_core::ops::ConstructVector2", "graphene_core::ops::CoordinateValueNode"),
	("graphene_core::ops::Vector2ValueNode", "graphene_core::ops::CoordinateValueNode"),
	("graphene_core::raster::BlackAndWhiteNode", "graphene_core::raster::adjustments::BlackAndWhiteNode"),
	("graphene_core::raster::BlendNode", "graphene_core::raster::adjustments::BlendNode"),
	("graphene_core::raster::BlendModeNode", "graphene_element_nodes::blending_nodes::BlendModeNode"),
	("graphene_core::blending_nodes::BlendModeNode", "graphene_element_nodes::blending_nodes::BlendModeNode"),
	("graphene_core::raster::OpacityNode", "graphene_element_nodes::blending_nodes::OpacityNode"),
	("graphene_core::blending_nodes::OpacityNode", "graphene_element_nodes::blending_nodes::OpacityNode"),
	("graphene_core::raster::BlendingNode", "graphene_element_nodes::blending_nodes::BlendingNode"),
	("graphene_core::blending_nodes::BlendingNode", "graphene_element_nodes::blending_nodes::BlendingNode"),
	("graphene_core::raster::ChannelMixerNode", "graphene_core::raster::adjustments::ChannelMixerNode"),
	("graphene_core::raster::adjustments::ColorOverlayNode", "graphene_core::raster::adjustments::ColorOverlayNode"),
	("graphene_core::raster::ExposureNode", "graphene_core::raster::adjustments::ExposureNode"),
	("graphene_core::raster::ExtractChannelNode", "graphene_core::raster::adjustments::ExtractChannelNode"),
	("graphene_core::raster::GradientMapNode", "graphene_core::raster::adjustments::GradientMapNode"),
	("graphene_core::raster::HueSaturationNode", "graphene_core::raster::adjustments::HueSaturationNode"),
	("graphene_core::vector::GenerateHandlesNode", "graphene_core::vector::AutoTangentsNode"),
	("graphene_core::vector::RemoveHandlesNode", "graphene_core::vector::AutoTangentsNode"),
	("graphene_core::raster::InvertNode", "graphene_core::raster::adjustments::InvertNode"),
	("graphene_core::raster::InvertRGBNode", "graphene_core::raster::adjustments::InvertNode"),
	("graphene_core::raster::LevelsNode", "graphene_core::raster::adjustments::LevelsNode"),
	("graphene_core::raster::LuminanceNode", "graphene_core::raster::adjustments::LuminanceNode"),
	("graphene_core::raster::ExtractOpaqueNode", "graphene_core::raster::adjustments::MakeOpaqueNode"),
	("graphene_core::raster::PosterizeNode", "graphene_core::raster::adjustments::PosterizeNode"),
	("graphene_core::raster::ThresholdNode", "graphene_core::raster::adjustments::ThresholdNode"),
	("graphene_core::raster::VibranceNode", "graphene_core::raster::adjustments::VibranceNode"),
	("graphene_core::text::TextGeneratorNode", "graphene_core::text::TextNode"),
	("graphene_core::transform::SetTransformNode", "graphene_element_nodes::transform_nodes::ReplaceTransformNode"),
	("graphene_core::transform::ReplaceTransformNode", "graphene_element_nodes::transform_nodes::ReplaceTransformNode"),
	("graphene_core::transform::TransformNode", "graphene_element_nodes::transform_nodes::TransformNode"),
	("graphene_core::transform_nodes::TransformNode", "graphene_element_nodes::transform_nodes::TransformNode"),
	("graphene_core::transform::BoundlessFootprintNode", "graphene_element_nodes::transform_nodes::BoundlessFootprintNode"),
	(
		"graphene_core::transform_nodes::BoundlessFootprintNode",
		"graphene_element_nodes::transform_nodes::BoundlessFootprintNode",
	),
	("graphene_core::transform::FreezeRealTimeNode", "graphene_element_nodes::transform_nodes::FreezeRealTimeNode"),
	("graphene_core::transform_nodes::FreezeRealTimeNode", "graphene_element_nodes::transform_nodes::FreezeRealTimeNode"),
	("graphene_core::animation::RealTimeNode", "graphene_element_nodes::animation::RealTimeNode"),
	("graphene_core::animation::AnimationTimeNode", "graphene_element_nodes::animation::AnimationTimeNode"),
	("graphene_core::instance::InstanceOnPointsNode", "graphene_element_nodes::instance::InstanceOnPointsNode"),
	("graphene_core::instance::InstanceRepeatNode", "graphene_element_nodes::instance::InstanceRepeatNode"),
	("graphene_core::instance::InstancePositionNode", "graphene_element_nodes::instance::InstancePositionNode"),
	("graphene_core::instance::InstanceIndexNode", "graphene_element_nodes::instance::InstanceIndexNode"),
	("graphene_core::vector::SplinesFromPointsNode", "graphene_core::vector::SplineNode"),
	("graphene_core::vector::generator_nodes::EllipseGenerator", "graphene_core::vector::generator_nodes::EllipseNode"),
	("graphene_core::vector::generator_nodes::LineGenerator", "graphene_core::vector::generator_nodes::LineNode"),
	("graphene_core::vector::generator_nodes::RectangleGenerator", "graphene_core::vector::generator_nodes::RectangleNode"),
	(
		"graphene_core::vector::generator_nodes::RegularPolygonGenerator",
		"graphene_core::vector::generator_nodes::RegularPolygonNode",
	),
	("graphene_core::vector::generator_nodes::StarGenerator", "graphene_core::vector::generator_nodes::StarNode"),
	("graphene_std::executor::BlendGpuImageNode", "graphene_std::gpu_nodes::BlendGpuImageNode"),
	("graphene_std::raster::SampleNode", "graphene_std::raster::SampleImageNode"),
	("graphene_core::transform::CullNode", "graphene_core::ops::IdentityNode"),
	("graphene_std::raster::MaskImageNode", "graphene_std::raster::MaskNode"),
	("graphene_core::vector::FlattenVectorElementsNode", "graphene_core::vector::FlattenPathNode"),
	("graphene_std::vector::BooleanOperationNode", "graphene_path_bool::BooleanOperationNode"),
];

pub fn document_migration_string_preprocessing(document_serialized_content: String) -> String {
	TEXT_REPLACEMENTS
		.iter()
		.fold(document_serialized_content, |document_serialized_content, (old, new)| document_serialized_content.replace(old, new))
}

pub fn document_migration_reset_node_definition(document_serialized_content: &str) -> bool {
	// Upgrade a document being opened to use fresh copies of all nodes
	if document_serialized_content.contains("node_output_index") {
		return true;
	}

	// Upgrade layer implementation from https://github.com/GraphiteEditor/Graphite/pull/1946 (see also `fn fix_nodes()` in `main.rs` of Graphene CLI)
	if document_serialized_content.contains("graphene_core::ConstructLayerNode") || document_serialized_content.contains("graphene_core::AddArtboardNode") {
		return true;
	}

	false
}

pub fn document_migration_upgrades(document: &mut DocumentMessageHandler, reset_node_definitions_on_open: bool) {
	let mut network = document.network_interface.document_network().clone();
	network.generate_node_paths(&[]);

	// Apply string replacements to each node
	let node_ids: Vec<_> = network.recursive_nodes().map(|(&id, node)| (id, node.original_location.path.clone().unwrap())).collect();
	for (node_id, path) in &node_ids {
		let network_path: Vec<_> = path.iter().copied().take(path.len() - 1).collect();

		if let Some(DocumentNodeImplementation::ProtoNode(protonode_id)) = document
			.network_interface
			.nested_network(&network_path)
			.unwrap()
			.nodes
			.get(node_id)
			.map(|node| node.implementation.clone())
		{
			for (old, new) in REPLACEMENTS {
				let node_path_without_type_args = protonode_id.name.split('<').next();
				if node_path_without_type_args == Some(old) {
					document
						.network_interface
						.replace_implementation(node_id, &network_path, DocumentNodeImplementation::ProtoNode(new.to_string().into()));
					document.network_interface.set_manual_compostion(node_id, &network_path, Some(graph_craft::Type::Generic("T".into())));
				}
			}
		}
	}

	if reset_node_definitions_on_open {
		// This can be used, if uncommented, to upgrade demo artwork with outdated document node internals from their definitions. Delete when it's no longer needed.
		// Used for upgrading old internal networks for demo artwork nodes. Will reset all node internals for any opened file
		for node_id in &document
			.network_interface
			.document_network_metadata()
			.persistent_metadata
			.node_metadata
			.keys()
			.cloned()
			.collect::<Vec<NodeId>>()
		{
			if let Some(reference) = document
				.network_interface
				.document_network_metadata()
				.persistent_metadata
				.node_metadata
				.get(node_id)
				.and_then(|node| node.persistent_metadata.reference.as_ref())
			{
				let Some(node_definition) = resolve_document_node_type(reference) else { continue };
				let default_definition_node = node_definition.default_node_template();
				document.network_interface.replace_implementation(node_id, &[], default_definition_node.document_node.implementation);
				document
					.network_interface
					.replace_implementation_metadata(node_id, &[], default_definition_node.persistent_node_metadata);
				document.network_interface.set_manual_compostion(node_id, &[], default_definition_node.document_node.manual_composition);
			}
		}
	}

	if document
		.network_interface
		.document_network_metadata()
		.persistent_metadata
		.node_metadata
		.iter()
		.any(|(node_id, node)| node.persistent_metadata.reference.as_ref().is_some_and(|reference| reference == "Output") && *node_id == NodeId(0))
	{
		document.network_interface.delete_nodes(vec![NodeId(0)], true, &[]);
	}

	let mut network = document.network_interface.document_network().clone();
	network.generate_node_paths(&[]);

	let node_ids: Vec<_> = network.recursive_nodes().map(|(&id, node)| (id, node.original_location.path.clone().unwrap())).collect();

	// Apply upgrades to each node
	for (node_id, path) in &node_ids {
		let network_path: Vec<_> = path.iter().copied().take(path.len() - 1).collect();
		let network_path = &network_path;

		let Some(node) = document.network_interface.nested_network(network_path).unwrap().nodes.get(node_id).cloned() else {
			log::error!("could not get node in deserialize_document");
			continue;
		};

		// Upgrade old nodes to use `Context` instead of `()` or `Footprint` for manual composition
		if node.manual_composition == Some(graph_craft::concrete!(())) || node.manual_composition == Some(graph_craft::concrete!(graphene_std::transform::Footprint)) {
			document
				.network_interface
				.set_manual_compostion(node_id, network_path, graph_craft::concrete!(graphene_std::Context).into());
		}

		let Some(node_metadata) = document.network_interface.network_metadata(network_path).unwrap().persistent_metadata.node_metadata.get(node_id) else {
			log::error!("could not get node metadata for node {node_id} in deserialize_document");
			continue;
		};

		let Some(ref reference) = node_metadata.persistent_metadata.reference.clone() else {
			// TODO: Investigate if this should be an expected case, because currently it runs hundreds of times normally.
			// TODO: Either delete the commented out error below if this is normal, or fix the underlying issue if this is not expected.
			// log::error!("could not get reference in deserialize_document");
			continue;
		};

		let inputs_count = node.inputs.len();

		// Upgrade Fill nodes to the format change in #1778
		if reference == "Fill" && inputs_count == 8 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let document_node = node_definition.default_node_template().document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);

			let Some(fill_type) = old_inputs[1].as_value().cloned() else { continue };
			let TaggedValue::FillType(fill_type) = fill_type else { continue };
			let Some(solid_color) = old_inputs[2].as_value().cloned() else { continue };
			let TaggedValue::OptionalColor(solid_color) = solid_color else { continue };
			let Some(gradient_type) = old_inputs[3].as_value().cloned() else { continue };
			let TaggedValue::GradientType(gradient_type) = gradient_type else { continue };
			let Some(start) = old_inputs[4].as_value().cloned() else { continue };
			let TaggedValue::DVec2(start) = start else { continue };
			let Some(end) = old_inputs[5].as_value().cloned() else { continue };
			let TaggedValue::DVec2(end) = end else { continue };
			let Some(transform) = old_inputs[6].as_value().cloned() else { continue };
			let TaggedValue::DAffine2(transform) = transform else { continue };
			let Some(positions) = old_inputs[7].as_value().cloned() else { continue };
			let TaggedValue::GradientStops(positions) = positions else { continue };

			let fill = match (fill_type, solid_color) {
				(FillType::Solid, None) => Fill::None,
				(FillType::Solid, Some(color)) => Fill::Solid(color),
				(FillType::Gradient, _) => Fill::Gradient(Gradient {
					stops: positions,
					gradient_type,
					start,
					end,
					transform,
				}),
			};
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Fill(fill.clone()), false), network_path);
			match fill {
				Fill::None => {
					document
						.network_interface
						.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::OptionalColor(None), false), network_path);
				}
				Fill::Solid(color) => {
					document
						.network_interface
						.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::OptionalColor(Some(color)), false), network_path);
				}
				Fill::Gradient(gradient) => {
					document
						.network_interface
						.set_input(&InputConnector::node(*node_id, 3), NodeInput::value(TaggedValue::Gradient(gradient), false), network_path);
				}
			}
		}

		// Upgrade Stroke node to reorder parameters and add "Align" and "Paint Order" (#2644)
		if reference == "Stroke" && inputs_count == 8 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let document_node = node_definition.default_node_template().document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document.network_interface.insert_input_properties_row(node_id, 8, network_path);
			document.network_interface.insert_input_properties_row(node_id, 9, network_path);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);
			let align_input = NodeInput::value(TaggedValue::StrokeAlign(StrokeAlign::Center), false);
			let paint_order_input = NodeInput::value(TaggedValue::PaintOrder(PaintOrder::StrokeAbove), false);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 3), align_input, network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[5].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 5), old_inputs[6].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 6), old_inputs[7].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 7), paint_order_input, network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 8), old_inputs[3].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 9), old_inputs[4].clone(), network_path);
		}

		// Rename the old "Splines from Points" node to "Spline" and upgrade it to the new "Spline" node
		if reference == "Splines from Points" {
			document.network_interface.set_reference(node_id, network_path, Some("Spline".to_string()));
		}

		// Upgrade the old "Spline" node to the new "Spline" node
		if reference == "Spline" {
			// Retrieve the proto node identifier and verify it is the old "Spline" node, otherwise skip it if this is the new "Spline" node
			let identifier = document
				.network_interface
				.implementation(node_id, network_path)
				.and_then(|implementation| implementation.get_proto_node());
			if identifier.map(|identifier| &identifier.name) != Some(&"graphene_core::vector::generator_nodes::SplineNode".into()) {
				continue;
			}

			// Obtain the document node for the given node ID, extract the vector points, and create vector data from the list of points
			let node = document.network_interface.document_node(node_id, network_path).unwrap();
			let Some(TaggedValue::VecDVec2(points)) = node.inputs.get(1).and_then(|tagged_value| tagged_value.as_value()) else {
				log::error!("The old Spline node's input at index 1 is not a TaggedValue::VecDVec2");
				continue;
			};
			let vector_data = VectorData::from_subpath(Subpath::from_anchors_linear(points.to_vec(), false));

			// Retrieve the output connectors linked to the "Spline" node's output port
			let spline_outputs = document
				.network_interface
				.outward_wires(network_path)
				.unwrap()
				.get(&OutputConnector::node(*node_id, 0))
				.expect("Vec of InputConnector Spline node is connected to its output port 0.")
				.clone();

			// Get the node's current position in the graph
			let Some(node_position) = document.network_interface.position(node_id, network_path) else {
				log::error!("Could not get position of spline node.");
				continue;
			};

			// Get the "Path" node definition and fill it in with the vector data and default vector modification
			let path_node_type = resolve_document_node_type("Path").expect("Path node does not exist.");
			let path_node = path_node_type.node_template_input_override([
				Some(NodeInput::value(TaggedValue::VectorData(VectorDataTable::new(vector_data)), true)),
				Some(NodeInput::value(TaggedValue::VectorModification(Default::default()), false)),
			]);

			// Get the "Spline" node definition and wire it up with the "Path" node as input
			let spline_node_type = resolve_document_node_type("Spline").expect("Spline node does not exist.");
			let spline_node = spline_node_type.node_template_input_override([Some(NodeInput::node(NodeId(1), 0))]);

			// Create a new node group with the "Path" and "Spline" nodes and generate new node IDs for them
			let nodes = vec![(NodeId(1), path_node), (NodeId(0), spline_node)];
			let new_ids = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect::<HashMap<_, _>>();
			let new_spline_id = *new_ids.get(&NodeId(0)).unwrap();
			let new_path_id = *new_ids.get(&NodeId(1)).unwrap();

			// Remove the old "Spline" node from the document
			document.network_interface.delete_nodes(vec![*node_id], false, network_path);

			// Insert the new "Path" and "Spline" nodes into the network interface with generated IDs
			document.network_interface.insert_node_group(nodes.clone(), new_ids, network_path);

			// Reposition the new "Spline" node to match the original "Spline" node's position
			document.network_interface.shift_node(&new_spline_id, node_position, network_path);

			// Reposition the new "Path" node with an offset relative to the original "Spline" node's position
			document.network_interface.shift_node(&new_path_id, node_position + IVec2::new(-7, 0), network_path);

			// Redirect each output connection from the old node to the new "Spline" node's output port
			for input_connector in spline_outputs {
				document.network_interface.set_input(&input_connector, NodeInput::node(new_spline_id, 0), network_path);
			}
		}

		// Upgrade Text node to include line height and character spacing, which were previously hardcoded to 1, from https://github.com/GraphiteEditor/Graphite/pull/2016
		if reference == "Text" && inputs_count != 8 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let document_node = node_definition.default_node_template().document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[3].clone(), network_path);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 4),
				if inputs_count == 6 {
					old_inputs[4].clone()
				} else {
					NodeInput::value(TaggedValue::F64(TypesettingConfig::default().line_height_ratio), false)
				},
				network_path,
			);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 5),
				if inputs_count == 6 {
					old_inputs[5].clone()
				} else {
					NodeInput::value(TaggedValue::F64(TypesettingConfig::default().character_spacing), false)
				},
				network_path,
			);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 6),
				NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_width), false),
				network_path,
			);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 7),
				NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_height), false),
				network_path,
			);
		}

		// Upgrade Sine, Cosine, and Tangent nodes to include a boolean input for whether the output should be in radians, which was previously the only option but is now not the default
		if (reference == "Sine" || reference == "Cosine" || reference == "Tangent") && inputs_count == 1 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let document_node = node_definition.default_node_template().document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Bool(true), false), network_path);
		}

		// Upgrade the Modulo node to include a boolean input for whether the output should be always positive, which was previously not an option
		if reference == "Modulo" && inputs_count == 2 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let document_node = node_definition.default_node_template().document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);
		}

		// Upgrade the Mirror node to add the `keep_original` boolean input
		if reference == "Mirror" && inputs_count == 3 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let document_node = node_definition.default_node_template().document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 3), NodeInput::value(TaggedValue::Bool(true), false), network_path);
		}

		// Upgrade the Mirror node to add the `reference_point` input and change `offset` from `DVec2` to `f64`
		if reference == "Mirror" && inputs_count == 4 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			let Some(&TaggedValue::DVec2(old_offset)) = old_inputs[1].as_value() else { return };
			let old_offset = if old_offset.x.abs() > old_offset.y.abs() { old_offset.x } else { old_offset.y };

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 1),
				NodeInput::value(TaggedValue::ReferencePoint(graphene_std::transform::ReferencePoint::Center), false),
				network_path,
			);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::F64(old_offset), false), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[2].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[3].clone(), network_path);
		}

		// Upgrade artboard name being passed as hidden value input to "To Artboard"
		if reference == "Artboard" && reset_node_definitions_on_open {
			let label = document.network_interface.display_name(node_id, network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(NodeId(0), 1), NodeInput::value(TaggedValue::String(label), false), &[*node_id]);
		}

		if reference == "Image" && inputs_count == 1 {
			let node_definition = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(reference).unwrap();
			let new_image_node = node_definition.default_node_template();
			document.network_interface.replace_implementation(node_id, network_path, new_image_node.document_node.implementation);

			// Insert a new empty input for the image
			document.network_interface.add_import(TaggedValue::None, false, 0, "Empty", "", &[*node_id]);
			document.network_interface.set_reference(node_id, network_path, Some("Image".to_string()));
		}

		if reference == "Noise Pattern" && inputs_count == 15 {
			let node_definition = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(reference).unwrap();
			let new_noise_pattern_node = node_definition.default_node_template();
			document
				.network_interface
				.replace_implementation(node_id, network_path, new_noise_pattern_node.document_node.implementation);

			let old_inputs = document.network_interface.replace_inputs(node_id, new_noise_pattern_node.document_node.inputs.clone(), network_path);

			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);
			for (i, input) in old_inputs.iter().enumerate() {
				document.network_interface.set_input(&InputConnector::node(*node_id, i + 1), input.clone(), network_path);
			}
		}

		if reference == "Instance on Points" && inputs_count == 2 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		}

		if reference == "Morph" && inputs_count == 4 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
			// We have removed the last input, so we don't add index 3
		}

		if reference == "Brush" && inputs_count == 4 {
			let node_definition = resolve_document_node_type(reference).unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			// We have removed the second input ("bounds"), so we don't add index 1 and we shift the rest of the inputs down by one
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[2].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[3].clone(), network_path);
		}

		if reference == "Flatten Vector Elements" {
			let node_definition = resolve_document_node_type("Flatten Path").unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);

			document.network_interface.replace_reference_name(node_id, network_path, "Flatten Path".to_string());
		}

		if reference == "Remove Handles" {
			let node_definition = resolve_document_node_type("Auto-Tangents").unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::F64(0.), false), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);

			document.network_interface.replace_reference_name(node_id, network_path, "Auto-Tangents".to_string());
		}

		if reference == "Generate Handles" {
			let node_definition = resolve_document_node_type("Auto-Tangents").unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(true), false), network_path);

			document.network_interface.replace_reference_name(node_id, network_path, "Auto-Tangents".to_string());
		}

		if reference == "Merge by Distance" && inputs_count == 2 {
			let node_definition = resolve_document_node_type("Merge by Distance").unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 2),
				NodeInput::value(TaggedValue::MergeByDistanceAlgorithm(graphene_std::vector::misc::MergeByDistanceAlgorithm::Topological), false),
				network_path,
			);
		}

		if reference == "Spatial Merge by Distance" {
			let node_definition = resolve_document_node_type("Merge by Distance").unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(
				&InputConnector::node(*node_id, 2),
				NodeInput::value(TaggedValue::MergeByDistanceAlgorithm(graphene_std::vector::misc::MergeByDistanceAlgorithm::Spatial), false),
				network_path,
			);

			document.network_interface.replace_reference_name(node_id, network_path, "Merge by Distance".to_string());
		}

		if reference == "Sample Points" && inputs_count == 5 {
			let node_definition = resolve_document_node_type("Sample Polyline").unwrap();
			let new_node_template = node_definition.default_node_template();
			let document_node = new_node_template.document_node;
			document.network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());
			document
				.network_interface
				.replace_implementation_metadata(node_id, network_path, new_node_template.persistent_node_metadata);

			let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);
			let new_spacing_value = NodeInput::value(TaggedValue::PointSpacingType(graphene_std::vector::misc::PointSpacingType::Separation), false);

			document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 1), new_spacing_value, network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[1].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[2].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 5), old_inputs[3].clone(), network_path);
			document.network_interface.set_input(&InputConnector::node(*node_id, 6), old_inputs[4].clone(), network_path);

			document.network_interface.replace_reference_name(node_id, network_path, "Sample Polyline".to_string());
		}
	}

	// Ensure layers are positioned as stacks if they are upstream siblings of another layer
	document.network_interface.load_structure();
	let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(document.network_interface.document_metadata()).collect::<Vec<_>>();
	for layer in all_layers {
		let Some((downstream_node, input_index)) = document
			.network_interface
			.outward_wires(&[])
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(layer.to_node(), 0)))
			.and_then(|outward_wires| outward_wires.first())
			.and_then(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())))
		else {
			continue;
		};
		// If the downstream node is a layer and the input is the first input and the current layer is not in a stack
		if input_index == 0 && document.network_interface.is_layer(&downstream_node, &[]) && !document.network_interface.is_stack(&layer.to_node(), &[]) {
			// Ensure the layer is horizontally aligned with the downstream layer to prevent changing the layout of old files
			let (Some(layer_position), Some(downstream_position)) = (document.network_interface.position(&layer.to_node(), &[]), document.network_interface.position(&downstream_node, &[])) else {
				log::error!("Could not get position for layer {:?} or downstream node {} when opening file", layer.to_node(), downstream_node);
				continue;
			};
			if layer_position.x == downstream_position.x {
				document.network_interface.set_stack_position_calculated_offset(&layer.to_node(), &downstream_node, &[]);
			}
		}
	}
}
