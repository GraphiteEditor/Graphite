use bezier_rs::Subpath;
use graph_craft::document::{value::TaggedValue, NodeInput};
use graphene_core::text::TypesettingConfig;
use graphene_std::vector::{
	style::{Fill, FillType, Gradient},
	VectorDataTable,
};

use super::*;
use crate::messages::portfolio::document::{
	node_graph::document_node_definitions::resolve_document_node_type,
	utility_types::network_interface::{InputConnector, NodeNetworkInterface},
};

/// Upgrade node's manual composition if needed
pub fn upgrade_node_manual_composition(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	let Some(network) = network_interface.network(network_path) else {
		return Err(EditorError::Document(format!("Failed to access network for upgrading manual composition of node {}", node_id)));
	};

	// Check if node needs manual composition upgrade
	if let Some(node) = network.nodes.get(node_id) {
		if node.manual_composition == Some(graph_craft::concrete!(())) || node.manual_composition == Some(graph_craft::concrete!(graphene_std::transform::Footprint)) {
			network_interface.set_manual_compostion(node_id, network_path, Some(graph_craft::concrete!(graphene_std::Context)));
		}
	}

	Ok(())
}

/// Apply definition-based upgrade to a node if needed
pub fn upgrade_node_from_definition(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get node metadata to find its reference
	let Some(network_metadata) = network_interface.network_metadata(network_path) else {
		return Err(EditorError::Document(format!("Failed to access network metadata for node definition upgrade of node {}", node_id)));
	};

	let reference = match network_metadata
		.persistent_metadata
		.node_metadata
		.get(node_id)
		.and_then(|node| node.persistent_metadata.reference.as_ref())
	{
		Some(reference) => reference.clone(),
		None => return Ok(()),
	};

	// Get node definition
	let Some(node_definition) = resolve_document_node_type(&reference) else {
		return Ok(());
	};

	let default_definition_node = node_definition.default_node_template();

	// Replace implementation
	network_interface.replace_implementation(node_id, network_path, default_definition_node.document_node.implementation);

	// Replace implementation metadata
	network_interface.replace_implementation_metadata(node_id, network_path, default_definition_node.persistent_node_metadata);

	// Set manual composition
	network_interface.set_manual_compostion(node_id, network_path, default_definition_node.document_node.manual_composition);

	Ok(())
}

/// Upgrade Fill node to new format from PR #1778
pub fn upgrade_fill_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get node to check if it needs an upgrade
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for Fill node upgrade of node {}", node_id)));
		}
	};

	// Only upgrade Fill nodes with 8 inputs
	if node.inputs.len() != 8 {
		return Ok(());
	}

	// Get the node definition
	let node_definition = match resolve_document_node_type("Fill") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Fill node definition not found".to_string()));
		}
	};

	let document_node = node_definition.default_node_template().document_node;

	// Replace the implementation
	network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

	// Replace inputs and save old inputs
	let old_inputs = network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

	// Set the first input to the old first input
	network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);

	// Extract values from old inputs
	let fill_type = old_inputs[1].as_value().cloned();
	let solid_color = old_inputs[2].as_value().cloned();
	let gradient_type = old_inputs[3].as_value().cloned();
	let start = old_inputs[4].as_value().cloned();
	let end = old_inputs[5].as_value().cloned();
	let transform = old_inputs[6].as_value().cloned();
	let positions = old_inputs[7].as_value().cloned();

	// Extract typed values (with pattern matching)
	if let (
		Some(TaggedValue::FillType(fill_type)),
		Some(TaggedValue::OptionalColor(solid_color)),
		Some(TaggedValue::GradientType(gradient_type)),
		Some(TaggedValue::DVec2(start)),
		Some(TaggedValue::DVec2(end)),
		Some(TaggedValue::DAffine2(transform)),
		Some(TaggedValue::GradientStops(positions)),
	) = (fill_type, solid_color, gradient_type, start, end, transform, positions)
	{
		// Create the fill based on the type
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

		// Set the fill input
		network_interface.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Fill(fill.clone()), false), network_path);

		// Set additional inputs based on fill type
		match fill {
			Fill::None => {
				network_interface.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::OptionalColor(None), false), network_path);
			}
			Fill::Solid(color) => {
				network_interface.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::OptionalColor(Some(color)), false), network_path);
			}
			Fill::Gradient(gradient) => {
				network_interface.set_input(&InputConnector::node(*node_id, 3), NodeInput::value(TaggedValue::Gradient(gradient), false), network_path);
			}
		}
	}

	Ok(())
}

/// Rename "Splines from Points" node to "Spline"
pub fn upgrade_splines_from_points_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	network_interface.set_reference(node_id, network_path, Some("Spline".to_string()));
	Ok(())
}

/// Upgrade Spline node to the new format that uses Path nodes
pub fn upgrade_spline_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Retrieve the proto node identifier and verify it is the old "Spline" node
	let identifier = network_interface.implementation(node_id, network_path).and_then(|implementation| implementation.get_proto_node());

	// Skip if this is not the old Spline node
	if identifier.map(|identifier| &identifier.name) != Some(&"graphene_core::vector::generator_nodes::SplineNode".into()) {
		return Ok(());
	}

	// Get the document node to extract vector points
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for Spline node upgrade of node {}", node_id)));
		}
	};

	// Extract vector points
	let points = match node.inputs.get(1).and_then(|input| input.as_value()) {
		Some(TaggedValue::VecDVec2(points)) => points,
		_ => {
			return Err(EditorError::DocumentDeserialization(format!(
				"The old Spline node's input at index 1 is not a TaggedValue::VecDVec2 for node {}",
				node_id
			)));
		}
	};

	// Create vector data from points
	let vector_data = VectorData::from_subpath(Subpath::from_anchors_linear(points.to_vec(), false));

	// Get output connections from the old Spline node
	let outward_wires = match network_interface.outward_wires(network_path) {
		Some(wires) => wires,
		None => {
			return Err(EditorError::Document(format!("Failed to get outward wires for Spline node {}", node_id)));
		}
	};

	let spline_outputs = outward_wires.get(&OutputConnector::node(*node_id, 0)).cloned().unwrap_or_default();

	// Get the node's position
	let node_position = match network_interface.position(node_id, network_path) {
		Some(pos) => pos,
		None => {
			return Err(EditorError::Document(format!("Failed to get position for Spline node {}", node_id)));
		}
	};

	// Get the Path node definition and create it with vector data
	let path_node_type = match resolve_document_node_type("Path") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Path node definition not found".to_string()));
		}
	};

	let path_node = path_node_type.node_template_input_override([
		Some(NodeInput::value(TaggedValue::VectorData(VectorDataTable::new(vector_data)), true)),
		Some(NodeInput::value(TaggedValue::VectorModification(Default::default()), false)),
	]);

	// Get the Spline node definition and wire it to the Path node
	let spline_node_type = match resolve_document_node_type("Spline") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Spline node definition not found".to_string()));
		}
	};

	let spline_node = spline_node_type.node_template_input_override([Some(NodeInput::node(NodeId(1), 0))]);

	// Create a node group with Path and Spline nodes
	let nodes = vec![(NodeId(1), path_node), (NodeId(0), spline_node)];
	let mut new_ids = HashMap::new();
	new_ids.insert(NodeId(0), NodeId::new());
	new_ids.insert(NodeId(1), NodeId::new());

	let new_spline_id = *new_ids.get(&NodeId(0)).unwrap();
	let new_path_id = *new_ids.get(&NodeId(1)).unwrap();

	// Remove the old Spline node
	network_interface.delete_nodes(vec![*node_id], false, network_path);

	// Insert the new nodes
	network_interface.insert_node_group(nodes, new_ids, network_path);

	// Position the new nodes
	network_interface.shift_node(&new_spline_id, node_position, network_path);
	network_interface.shift_node(&new_path_id, node_position + IVec2::new(-7, 0), network_path);

	// Redirect output connections to the new Spline node
	for input_connector in spline_outputs {
		network_interface.set_input(&input_connector, NodeInput::node(new_spline_id, 0), network_path);
	}

	Ok(())
}

/// Upgrade Text node to include line height and character spacing
pub fn upgrade_text_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get the node to check if it needs upgrading
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for Text node upgrade of node {}", node_id)));
		}
	};

	// Skip if node already has the right number of inputs
	let inputs_count = node.inputs.len();
	if inputs_count == 8 {
		return Ok(());
	}

	// Get the node definition
	let node_definition = match resolve_document_node_type("Text") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Text node definition not found".to_string()));
		}
	};

	let document_node = node_definition.default_node_template().document_node;

	// Replace the implementation
	network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

	// Replace inputs and save old inputs
	let old_inputs = network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

	// Set the first 4 inputs to the old inputs
	for i in 0..4 {
		if i < old_inputs.len() {
			network_interface.set_input(&InputConnector::node(*node_id, i), old_inputs[i].clone(), network_path);
		}
	}

	// Set line height (input 4)
	network_interface.set_input(
		&InputConnector::node(*node_id, 4),
		if inputs_count == 6 {
			old_inputs[4].clone()
		} else {
			NodeInput::value(TaggedValue::F64(TypesettingConfig::default().line_height_ratio), false)
		},
		network_path,
	);

	// Set character spacing (input 5)
	network_interface.set_input(
		&InputConnector::node(*node_id, 5),
		if inputs_count == 6 {
			old_inputs[5].clone()
		} else {
			NodeInput::value(TaggedValue::F64(TypesettingConfig::default().character_spacing), false)
		},
		network_path,
	);

	// Set max width (input 6)
	network_interface.set_input(
		&InputConnector::node(*node_id, 6),
		NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_width), false),
		network_path,
	);

	// Set max height (input 7)
	network_interface.set_input(
		&InputConnector::node(*node_id, 7),
		NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_height), false),
		network_path,
	);

	Ok(())
}

/// Upgrade trigonometric nodes (Sine, Cosine, Tangent) to include radians input
pub fn upgrade_trigonometric_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId], node_type: &str) -> Result<(), EditorError> {
	// Get the node to check if it needs upgrading
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for {} node upgrade of node {}", node_type, node_id)));
		}
	};

	// Skip if node already has more than 1 input
	if node.inputs.len() != 1 {
		return Ok(());
	}

	// Get the node definition
	let node_definition = match resolve_document_node_type(node_type) {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization(format!("{} node definition not found", node_type)));
		}
	};

	let document_node = node_definition.default_node_template().document_node;

	// Replace the implementation
	network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

	// Replace inputs and save old inputs
	let old_inputs = network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

	// Set the first input to the old input
	network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);

	// Set the radians input to true (since old nodes always used radians)

	network_interface.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Bool(true), false), network_path);

	Ok(())
}

/// Upgrade Modulo node to include "always positive" input
pub fn upgrade_modulo_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get the node to check if it needs upgrading
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for Modulo node upgrade of node {}", node_id)));
		}
	};

	// Skip if node already has more than 2 inputs
	if node.inputs.len() != 2 {
		return Ok(());
	}

	// Get the node definition
	let node_definition = match resolve_document_node_type("Modulo") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Modulo node definition not found".to_string()));
		}
	};

	let document_node = node_definition.default_node_template().document_node;

	// Replace the implementation
	network_interface.replace_implementation(node_id, network_path, document_node.implementation.clone());

	// Replace inputs and save old inputs
	let old_inputs = network_interface.replace_inputs(node_id, document_node.inputs.clone(), network_path);

	// Set the first two inputs to the old inputs
	network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
	network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);

	// Set the "always positive" input to false (to match old behavior)

	network_interface.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);

	Ok(())
}

/// Upgrade Artboard node to include name input
pub fn upgrade_artboard_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get the display name
	let label = network_interface.frontend_display_name(node_id, network_path);

	// Set the name input (at node 0, input 1)

	network_interface.set_input(&InputConnector::node(NodeId(0), 1), NodeInput::value(TaggedValue::String(label), false), &[*node_id]);

	Ok(())
}

/// Upgrade Image node to add empty input
pub fn upgrade_image_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get the node to check if it needs upgrading
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for Image node upgrade of node {}", node_id)));
		}
	};

	// Skip if node already has more than 1 input
	if node.inputs.len() != 1 {
		return Ok(());
	}

	// Get the node definition
	let node_definition = match resolve_document_node_type("Image") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Image node definition not found".to_string()));
		}
	};

	let new_image_node = node_definition.default_node_template();

	// Replace the implementation
	network_interface.replace_implementation(node_id, network_path, new_image_node.document_node.implementation);

	// Insert a new empty input for the image
	network_interface.add_import(TaggedValue::None, false, 0, "Empty", &[*node_id]);

	// Set the reference
	network_interface.set_reference(node_id, network_path, Some("Image".to_string()));

	Ok(())
}

/// Upgrade Noise Pattern node with 15 inputs
pub fn upgrade_noise_pattern_node(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, network_path: &[NodeId]) -> Result<(), EditorError> {
	// Get the node to check if it needs upgrading
	let node = match network_interface.document_node(node_id, network_path) {
		Some(node) => node,
		None => {
			return Err(EditorError::Document(format!("Failed to get document node for Noise Pattern node upgrade of node {}", node_id)));
		}
	};

	// Skip if node does not have exactly 15 inputs
	if node.inputs.len() != 15 {
		return Ok(());
	}

	// Get the node definition
	let node_definition = match resolve_document_node_type("Noise Pattern") {
		Some(def) => def,
		None => {
			return Err(EditorError::DocumentDeserialization("Noise Pattern node definition not found".to_string()));
		}
	};

	let new_noise_pattern_node = node_definition.default_node_template();

	// Replace the implementation

	network_interface.replace_implementation(node_id, network_path, new_noise_pattern_node.document_node.implementation);

	// Replace inputs and save old inputs
	let old_inputs = network_interface.replace_inputs(node_id, new_noise_pattern_node.document_node.inputs.clone(), network_path);

	// Set the first input to None

	network_interface.set_input(&InputConnector::node(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);

	// Copy over the old inputs to the new inputs, offset by 1
	for (i, input) in old_inputs.iter().enumerate() {
		network_interface.set_input(&InputConnector::node(*node_id, i + 1), input.clone(), network_path);
	}

	Ok(())
}
