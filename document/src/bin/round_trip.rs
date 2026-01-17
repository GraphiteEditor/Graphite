use std::fs;
use std::path::PathBuf;

use graph_craft::document::NodeNetwork;
use graph_storage::Registry;

fn main() {
	let args: Vec<String> = std::env::args().collect();

	if args.len() != 3 {
		eprintln!("Usage: {} <input.graphite> <output.graphite>", args[0]);
		eprintln!();
		eprintln!("Round-trips a Graphite artwork file through the Registry format.");
		eprintln!("This converts: .graphite -> NodeNetwork -> Registry -> NodeNetwork -> .graphite");
		std::process::exit(1);
	}

	let input_path = PathBuf::from(&args[1]);
	let output_path = PathBuf::from(&args[2]);

	println!("Loading artwork from: {}", input_path.display());

	// Read the input file
	let json_content = fs::read_to_string(&input_path).unwrap_or_else(|e| {
		eprintln!("Error reading input file: {}", e);
		std::process::exit(1);
	});

	// Parse the JSON
	let mut doc: serde_json::Value = serde_json::from_str(&json_content).unwrap_or_else(|e| {
		eprintln!("Error parsing JSON: {}", e);
		std::process::exit(1);
	});

	// Extract the network
	let network_json = doc["network_interface"]["network"].clone();
	let original_network: NodeNetwork = serde_json::from_value(network_json).unwrap_or_else(|e| {
		eprintln!("Error deserializing NodeNetwork: {}", e);
		std::process::exit(1);
	});

	println!("Original network: {} nodes", original_network.nodes.len());

	// Convert to Registry
	let registry = Registry::try_from(&original_network).unwrap_or_else(|e| {
		eprintln!("Error converting to Registry: {}", e);
		std::process::exit(1);
	});

	println!("Registry: {} node instances, {} networks", registry.node_instances.len(), registry.networks.len());

	// Debug: Print all node IDs in the registry
	let mut node_ids: Vec<_> = registry.node_instances.keys().copied().collect();
	node_ids.sort();
	println!("Registry node IDs: {:?}", node_ids);

	// Convert back to NodeNetwork
	let converted_network = NodeNetwork::try_from(&registry).unwrap_or_else(|e| {
		eprintln!("Error converting back to NodeNetwork: {}", e);
		std::process::exit(1);
	});

	println!("Converted network: {} nodes", converted_network.nodes.len());

	// Replace the network in the document
	doc["network_interface"]["network"] = serde_json::to_value(&converted_network).unwrap_or_else(|e| {
		eprintln!("Error serializing converted network: {}", e);
		std::process::exit(1);
	});

	// Write the output file
	let output_json = serde_json::to_string_pretty(&doc).unwrap_or_else(|e| {
		eprintln!("Error serializing output JSON: {}", e);
		std::process::exit(1);
	});

	fs::write(&output_path, output_json).unwrap_or_else(|e| {
		eprintln!("Error writing output file: {}", e);
		std::process::exit(1);
	});

	println!("Successfully wrote round-tripped artwork to: {}", output_path.display());
}
