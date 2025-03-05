use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::error::EditorError;
use crate::messages::portfolio::document::utility_types::network_interface::OutputConnector;
use glam::IVec2;
use graph_craft::document::DocumentNodeImplementation;
use graphene_core::vector::VectorData;
use graphene_std::uuid::NodeId;
use log;
use std::collections::HashMap;

mod node_upgrades;
use node_upgrades::*;

use super::DocumentMessageHandler;

/// Main entry point for upgrading documents when they are opened
pub fn process_open_document_file_with_id(document_name: String, document_serialized_content: String) -> Result<DocumentMessageHandler, EditorError> {
	// Detect upgrades needed
	let (document_name, upgrade_flags, document_serialized_content) = detect_needed_upgrades(&document_name, &document_serialized_content);

	// Apply text-based replacements
	let document_serialized_content = apply_text_replacements(document_serialized_content);

	// Deserialize the document
	let mut document = DocumentMessageHandler::deserialize_document(&document_serialized_content).map(|mut document| {
		document.name.clone_from(&document_name);
		document
	})?;

	// Apply node path-based replacements
	apply_node_path_replacements(&mut document);

	// Recursively upgrade all nodes in the document
	upgrade_network(&mut document, &[], &upgrade_flags)?;

	// Ensure layers are positioned as stacks if necessary
	ensure_layer_stack_positions(&mut document);

	Ok(document)
}

/// Struct to hold upgrade flags
pub struct UpgradeFlags {
	pub replace_implementations_from_definition: bool,
	pub upgrade_from_before_returning_nested_click_targets: bool,
	pub upgrade_vector_manipulation_format: bool,
}

/// Detect what upgrades are needed for this document
pub(super) fn detect_needed_upgrades(document_name: &str, document_serialized_content: &str) -> (String, UpgradeFlags, String) {
	let replace_implementations_from_definition = document_serialized_content.contains("node_output_index");
	let upgrade_from_before_returning_nested_click_targets =
		document_serialized_content.contains("graphene_core::ConstructLayerNode") || document_serialized_content.contains("graphene_core::AddArtboardNode");
	let upgrade_vector_manipulation_format = document_serialized_content.contains("ManipulatorGroupIds") && !document_name.contains("__DO_NOT_UPGRADE__");

	let document_name = document_name.replace("__DO_NOT_UPGRADE__", "");

	let upgrade_flags = UpgradeFlags {
		replace_implementations_from_definition,
		upgrade_from_before_returning_nested_click_targets,
		upgrade_vector_manipulation_format,
	};

	(document_name, upgrade_flags, document_serialized_content.to_string())
}

/// Apply text-based replacements to the serialized document content
fn apply_text_replacements(document_serialized_content: String) -> String {
	const TEXT_REPLACEMENTS: [(&str, &str); 2] = [
		("graphene_core::vector::vector_nodes::SamplePointsNode", "graphene_core::vector::SamplePointsNode"),
		("graphene_core::vector::vector_nodes::SubpathSegmentLengthsNode", "graphene_core::vector::SubpathSegmentLengthsNode"),
	];

	TEXT_REPLACEMENTS
		.iter()
		.fold(document_serialized_content, |document_serialized_content, (old, new)| document_serialized_content.replace(old, new))
}

/// Apply path-based replacements to node implementations
/// These were renamed before editable subgraphs were introduced so
/// don't need to apply replacements recursively
fn apply_node_path_replacements(document: &mut DocumentMessageHandler) {
	const REPLACEMENTS: [(&str, &str); 34] = [
		("graphene_core::AddArtboardNode", "graphene_core::graphic_element::AppendArtboardNode"),
		("graphene_core::ConstructArtboardNode", "graphene_core::graphic_element::ToArtboardNode"),
		("graphene_core::ToGraphicElementNode", "graphene_core::graphic_element::ToElementNode"),
		("graphene_core::ToGraphicGroupNode", "graphene_core::graphic_element::ToGroupNode"),
		("graphene_core::logic::LogicAndNode", "graphene_core::ops::LogicAndNode"),
		("graphene_core::logic::LogicNotNode", "graphene_core::ops::LogicNotNode"),
		("graphene_core::logic::LogicOrNode", "graphene_core::ops::LogicOrNode"),
		("graphene_core::ops::ConstructVector2", "graphene_core::ops::Vector2ValueNode"),
		("graphene_core::raster::BlackAndWhiteNode", "graphene_core::raster::adjustments::BlackAndWhiteNode"),
		("graphene_core::raster::BlendNode", "graphene_core::raster::adjustments::BlendNode"),
		("graphene_core::raster::ChannelMixerNode", "graphene_core::raster::adjustments::ChannelMixerNode"),
		("graphene_core::raster::adjustments::ColorOverlayNode", "graphene_core::raster::adjustments::ColorOverlayNode"),
		("graphene_core::raster::ExposureNode", "graphene_core::raster::adjustments::ExposureNode"),
		("graphene_core::raster::ExtractChannelNode", "graphene_core::raster::adjustments::ExtractChannelNode"),
		("graphene_core::raster::GradientMapNode", "graphene_core::raster::adjustments::GradientMapNode"),
		("graphene_core::raster::HueSaturationNode", "graphene_core::raster::adjustments::HueSaturationNode"),
		("graphene_core::raster::InvertNode", "graphene_core::raster::adjustments::InvertNode"),
		// ("graphene_core::raster::IndexNode", "graphene_core::raster::adjustments::IndexNode"),
		("graphene_core::raster::InvertRGBNode", "graphene_core::raster::adjustments::InvertNode"),
		("graphene_core::raster::LevelsNode", "graphene_core::raster::adjustments::LevelsNode"),
		("graphene_core::raster::LuminanceNode", "graphene_core::raster::adjustments::LuminanceNode"),
		("graphene_core::raster::ExtractOpaqueNode", "graphene_core::raster::adjustments::MakeOpaqueNode"),
		("graphene_core::raster::PosterizeNode", "graphene_core::raster::adjustments::PosterizeNode"),
		("graphene_core::raster::ThresholdNode", "graphene_core::raster::adjustments::ThresholdNode"),
		("graphene_core::raster::VibranceNode", "graphene_core::raster::adjustments::VibranceNode"),
		("graphene_core::text::TextGeneratorNode", "graphene_core::text::TextNode"),
		("graphene_core::transform::SetTransformNode", "graphene_core::transform::ReplaceTransformNode"),
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
	];

	// Get all node IDs in the document
	let Some(network_metadata) = document.network_interface.network_metadata(&[]) else {
		log::error!("Failed to get network metadata during node path replacements");
		return;
	};

	let node_ids = network_metadata.persistent_metadata.node_metadata.keys().cloned().collect::<Vec<NodeId>>();

	// Apply replacements to each node
	for node_id in node_ids {
		let Some(network) = document.network_interface.network(&[]) else {
			log::error!("Failed to get network during node path replacements");
			continue;
		};

		if let Some(DocumentNodeImplementation::ProtoNode(protonode_id)) = network.nodes.get(&node_id).map(|node| node.implementation.clone()) {
			for (old, new) in REPLACEMENTS {
				let node_path_without_type_args = protonode_id.name.split('<').next();
				if node_path_without_type_args == Some(old) {
					document
						.network_interface
						.replace_implementation(&node_id, &[], DocumentNodeImplementation::ProtoNode(new.to_string().into()));
					document.network_interface.set_manual_compostion(&node_id, &[], Some(graph_craft::Type::Generic("T".into())));
				}
			}
		}
	}
}

/// Recursively upgrade all nodes in the network and sub-networks
fn upgrade_network(document: &mut DocumentMessageHandler, network_path: &[NodeId], upgrade_flags: &UpgradeFlags) -> Result<(), EditorError> {
	// Get all node IDs in this network
	let Some(network_metadata) = document.network_interface.network_metadata(&[]).cloned() else {
		return Err(EditorError::Document("Failed to access network metadata for upgrading".to_string()));
	};

	// First, handle removal of special node with ID 0 if it's an Output node
	let has_output_node = network_metadata
		.persistent_metadata
		.node_metadata
		.iter()
		.any(|(node_id, node)| node.persistent_metadata.reference.as_ref().is_some_and(|reference| reference == "Output") && *node_id == NodeId(0));

	if has_output_node {
		document.network_interface.delete_nodes(vec![NodeId(0)], true, network_path);
	}

	let mut network = document.network_interface.network(&[]).unwrap().clone();
	network.generate_node_paths(&[]);

	let node_ids: Vec<_> = network.recursive_nodes().map(|(&id, node)| (id, node.original_location.path.clone().unwrap())).collect();

	// Apply upgrades to each node
	for (node_id, path) in &node_ids {
		let network_path: Vec<_> = path.iter().copied().take(path.len() - 1).collect();
		let network_path = &network_path;

		let network_interface = &mut document.network_interface;

		// Apply general node upgrades
		if let Err(e) = upgrade_node_manual_composition(network_interface, node_id, network_path) {
			log::error!("Failed to upgrade manual composition for node {node_id}: {e}");
		}
		// Get node metadata
		let node_metadata = match network_metadata.persistent_metadata.node_metadata.get(node_id) {
			Some(metadata) => metadata,
			None => {
				log::error!("Could not get node metadata for node {node_id} in network path {:?}", network_path);
				continue;
			}
		};

		// Check if node has a reference (node type)
		let reference = match &node_metadata.persistent_metadata.reference {
			Some(reference) => reference.clone(),
			None => {
				log::error!("Node {node_id} in network path {:?} has no reference", network_path);
				continue;
			}
		};

		// Apply node-specific upgrades
		let result = match reference.as_str() {
			"Fill" => upgrade_fill_node(network_interface, node_id, network_path),
			"Splines from Points" => upgrade_splines_from_points_node(network_interface, node_id, network_path),
			"Spline" => upgrade_spline_node(network_interface, node_id, network_path),
			"Text" => upgrade_text_node(network_interface, node_id, network_path),
			"Sine" | "Cosine" | "Tangent" => upgrade_trigonometric_node(network_interface, node_id, network_path, &reference),
			"Modulo" => upgrade_modulo_node(network_interface, node_id, network_path),
			"Artboard" => {
				if upgrade_flags.upgrade_from_before_returning_nested_click_targets {
					upgrade_artboard_node(network_interface, node_id, network_path)
				} else {
					Ok(())
				}
			}
			"Image" => upgrade_image_node(network_interface, node_id, network_path),
			"Noise Pattern" => upgrade_noise_pattern_node(network_interface, node_id, network_path),
			_ => Ok(()),
		};

		if let Err(e) = result {
			log::error!("Failed to upgrade node {node_id} of type {reference}: {e}");
		}

		// Apply definition-based upgrades if needed
		if upgrade_flags.replace_implementations_from_definition || upgrade_flags.upgrade_from_before_returning_nested_click_targets {
			if let Err(e) = upgrade_node_from_definition(network_interface, node_id, network_path) {
				log::error!("Failed to upgrade node {node_id} from definition: {e}");
			}
		}

		// Recursively upgrade sub-networks if this node has them
		// Note: Would need network traversal logic here if nodes can have sub-networks
	}

	Ok(())
}

/// Ensure layers are positioned as stacks if they are upstream siblings of another layer
fn ensure_layer_stack_positions(document: &mut DocumentMessageHandler) {
	document.network_interface.load_structure();

	let metadata = document.network_interface.document_metadata();
	let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(metadata).collect::<Vec<_>>();

	for layer in all_layers {
		// Find if this layer is connected to another layer's input
		let downstream_connection = document
			.network_interface
			.outward_wires(&[])
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(layer.to_node(), 0)))
			.and_then(|outward_wires| outward_wires.first())
			.and_then(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())));

		if let Some((downstream_node, input_index)) = downstream_connection {
			// If the downstream node is a layer and the input is the first input and the current layer is not in a stack
			if input_index == 0 && document.network_interface.is_layer(&downstream_node, &[]) && !document.network_interface.is_stack(&layer.to_node(), &[]) {
				// Ensure the layer is horizontally aligned with the downstream layer to prevent changing the layout of old files
				let layer_position = document.network_interface.position(&layer.to_node(), &[]);
				let downstream_position = document.network_interface.position(&downstream_node, &[]);

				if let (Some(layer_position), Some(downstream_position)) = (layer_position, downstream_position) {
					if layer_position.x == downstream_position.x {
						document.network_interface.set_stack_position_calculated_offset(&layer.to_node(), &downstream_node, &[]);
					}
				} else {
					log::error!("Could not get position for layer {:?} or downstream node {} when opening file", layer.to_node(), downstream_node);
				}
			}
		}
	}
}
