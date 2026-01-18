use std::borrow::Cow;

use core_types::context::{ContextDependencies, ContextFeature};
use core_types::uuid::NodeId;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::{Type, concrete, ProtoNodeIdentifier};

use crate::Registry;

/// Helper function to verify a NodeNetwork can be compiled successfully.
/// Note: This only works for complete networks with all inputs resolved.
/// Test networks with Import inputs will fail compilation (which is expected).
fn verify_network_compiles(network: &NodeNetwork) -> Result<(), String> {
	let compiler = Compiler {};
	compiler.compile_single(network.clone())
		.map_err(|e| format!("Compilation failed: {:?}", e))?;
	Ok(())
}

/// Helper to try compiling a network, returning true if successful, false if it fails
/// (without panicking). Used for networks that may have unresolved imports.
fn try_compile_network(network: &NodeNetwork) -> bool {
	verify_network_compiles(network).is_ok()
}

/// Creates a simple test network with two nodes:
/// - Node 0: ConsNode that takes two u32 imports
/// - Node 1: AddPairNode that adds the cons pair
fn create_simple_network() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(u32), 0), NodeInput::import(concrete!(u32), 1)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::structural::ConsNode")),
					..Default::default()
				},
			),
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::AddPairNode")),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

/// Creates a network with a nested sub-network
fn create_nested_network() -> NodeNetwork {
	// Create a simple inner network
	let inner_network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(10), 0)],
		nodes: [(
			NodeId(10),
			DocumentNode {
				inputs: vec![NodeInput::import(concrete!(u32), 0)],
				implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	};

	// Create outer network that uses the inner network
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(u32), 0)],
					implementation: DocumentNodeImplementation::Network(inner_network),
					..Default::default()
				},
			),
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

#[test]
fn test_simple_round_trip() {
	let original_network = create_simple_network();

	// Convert to Registry
	let registry = Registry::try_from(&original_network).expect("Failed to convert NodeNetwork to Registry");

	// Convert back to NodeNetwork
	let converted_network = NodeNetwork::try_from(&registry).expect("Failed to convert Registry back to NodeNetwork");

	// Verify structure is preserved
	assert_eq!(
		converted_network.nodes.len(),
		original_network.nodes.len(),
		"Node count should be preserved"
	);
	assert_eq!(
		converted_network.exports.len(),
		original_network.exports.len(),
		"Export count should be preserved"
	);

	// Verify exports reference the correct nodes
	match (&original_network.exports[0], &converted_network.exports[0]) {
		(NodeInput::Node { node_id: orig_id, output_index: orig_idx }, NodeInput::Node { node_id: conv_id, output_index: conv_idx }) => {
			assert_eq!(orig_id, conv_id, "Export should reference the same node");
			assert_eq!(orig_idx, conv_idx, "Export output index should match");
		}
		_ => panic!("Exports should both be Node inputs"),
	}

	// Verify node implementations are preserved
	for (node_id, orig_node) in &original_network.nodes {
		let conv_node = converted_network.nodes.get(node_id).expect("Node should exist after round-trip");

		match (&orig_node.implementation, &conv_node.implementation) {
			(DocumentNodeImplementation::ProtoNode(orig_ident), DocumentNodeImplementation::ProtoNode(conv_ident)) => {
				assert_eq!(orig_ident.as_str(), conv_ident.as_str(), "ProtoNode identifier should be preserved");
			}
			_ => panic!("Implementation type should be preserved"),
		}

		// Verify input count is preserved
		assert_eq!(conv_node.inputs.len(), orig_node.inputs.len(), "Input count should be preserved");
	}
}

#[test]
fn test_nested_network_round_trip() {
	let original_network = create_nested_network();

	// Convert to Registry
	let registry = Registry::try_from(&original_network).expect("Failed to convert NodeNetwork to Registry");

	// Convert back to NodeNetwork
	let converted_network = NodeNetwork::try_from(&registry).expect("Failed to convert Registry back to NodeNetwork");

	// Verify structure is preserved
	assert_eq!(
		converted_network.nodes.len(),
		original_network.nodes.len(),
		"Node count should be preserved"
	);

	// Find the node with nested network
	let orig_nested_node = original_network.nodes.get(&NodeId(0)).expect("Node 0 should exist");
	let conv_nested_node = converted_network.nodes.get(&NodeId(0)).expect("Node 0 should exist after round-trip");

	// Verify nested network is preserved
	match (&orig_nested_node.implementation, &conv_nested_node.implementation) {
		(DocumentNodeImplementation::Network(orig_inner), DocumentNodeImplementation::Network(conv_inner)) => {
			assert_eq!(orig_inner.nodes.len(), conv_inner.nodes.len(), "Inner network node count should be preserved");
			assert_eq!(orig_inner.exports.len(), conv_inner.exports.len(), "Inner network export count should be preserved");
		}
		_ => panic!("Nested network should be preserved"),
	}
}

#[test]
fn test_registry_structure() {
	let network = create_simple_network();

	// Convert to Registry
	let registry = Registry::try_from(&network).expect("Failed to convert to Registry");

	// Verify Registry structure
	assert!(registry.node_declarations.len() >= 2, "Should have proto node declarations");
	assert!(registry.networks.len() >= 1, "Should have at least one network");

	// Verify identity nodes were created for exports
	let root_network = registry.networks.get(&0).expect("Root network should exist");
	assert_eq!(root_network.exports.len(), network.exports.len(), "Should have same number of exports");

	// Verify identity nodes exist in node_instances
	for &identity_node_id in &root_network.exports {
		let identity_node = registry.node_instances.get(&identity_node_id).expect("Identity node should exist");
		assert_eq!(identity_node.inputs.len(), 1, "Identity node should have exactly one input");
	}
}

#[test]
fn test_nested_network_flattening() {
	let network = create_nested_network();

	// Convert to Registry - should flatten nested networks
	let registry = Registry::try_from(&network).expect("Failed to convert to Registry");

	// The outer network has 2 nodes, and one of them contains a nested network with 1 node
	// So we should have at least 3 nodes total (2 outer + 1 inner) plus identity nodes
	// Identity nodes: 1 for root network, 1 for inner network
	let expected_min_nodes = 3 + 2; // regular nodes + identity nodes
	assert!(
		registry.node_instances.len() >= expected_min_nodes,
		"Registry should have at least {} nodes (including identity nodes), found {}",
		expected_min_nodes,
		registry.node_instances.len()
	);

	// Should have 2 networks: root (0) and nested (1)
	assert!(registry.networks.len() >= 2, "Should have at least 2 networks (root + nested)");
}

#[test]
fn test_metadata_preservation() {
	// Create a network with nodes that have non-default metadata
	let mut context_features = ContextDependencies::default();
	context_features.extract = core_types::context::ContextFeatures::FOOTPRINT | core_types::context::ContextFeatures::REAL_TIME;

	let network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(f64), 0), NodeInput::import(Type::Generic(Cow::Borrowed("T")), 1)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("test::NodeWithMetadata")),
					call_argument: concrete!(String),
					context_features: context_features.clone(),
					visible: false, // Non-default value
					skip_deduplication: true, // Non-default value
					..Default::default()
				},
			),
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("test::OutputNode")),
					call_argument: concrete!((u32, u32)),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	};

	// Convert to Registry and back
	let registry = Registry::try_from(&network).expect("Failed to convert to Registry");
	let converted = NodeNetwork::try_from(&registry).expect("Failed to convert back to NodeNetwork");

	// Verify call_argument is preserved
	let orig_node_0 = network.nodes.get(&NodeId(0)).unwrap();
	let conv_node_0 = converted.nodes.get(&NodeId(0)).unwrap();
	assert_eq!(orig_node_0.call_argument, conv_node_0.call_argument, "call_argument for node 0 should be preserved");

	let orig_node_1 = network.nodes.get(&NodeId(1)).unwrap();
	let conv_node_1 = converted.nodes.get(&NodeId(1)).unwrap();
	assert_eq!(orig_node_1.call_argument, conv_node_1.call_argument, "call_argument for node 1 should be preserved");

	// Verify context_features is preserved
	assert_eq!(
		orig_node_0.context_features, conv_node_0.context_features,
		"context_features should be preserved"
	);

	// Verify visible is preserved
	assert_eq!(orig_node_0.visible, conv_node_0.visible, "visible should be preserved");

	// Verify skip_deduplication is preserved
	assert_eq!(
		orig_node_0.skip_deduplication, conv_node_0.skip_deduplication,
		"skip_deduplication should be preserved"
	);

	// Verify import_type is preserved for Import inputs
	match (&orig_node_0.inputs[0], &conv_node_0.inputs[0]) {
		(NodeInput::Import { import_type: orig_type, .. }, NodeInput::Import { import_type: conv_type, .. }) => {
			assert_eq!(orig_type, conv_type, "import_type for first import should be preserved (f64)");
		}
		_ => panic!("First input should be Import"),
	}

	match (&orig_node_0.inputs[1], &conv_node_0.inputs[1]) {
		(NodeInput::Import { import_type: orig_type, .. }, NodeInput::Import { import_type: conv_type, .. }) => {
			assert_eq!(orig_type, conv_type, "import_type for second import should be preserved (generic T)");
		}
		_ => panic!("Second input should be Import"),
	}
}

#[test]
fn test_demo_artwork_round_trip() {
	use graph_craft::util::{load_network, DEMO_ART};

	// Test each demo artwork
	for artwork_name in DEMO_ART {
		println!("Testing artwork: {}", artwork_name);

		// Load the original network
		let path = format!("../demo-artwork/{}.graphite", artwork_name);
		let document_string = std::fs::read_to_string(&path)
			.unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
		let original_network = load_network(&document_string);

		// Convert to Registry
		let registry = Registry::try_from(&original_network)
			.unwrap_or_else(|e| panic!("Failed to convert {} to Registry: {:?}", artwork_name, e));

		// Convert back to NodeNetwork
		let converted_network = NodeNetwork::try_from(&registry)
			.unwrap_or_else(|e| panic!("Failed to convert {} back to NodeNetwork: {:?}", artwork_name, e));

		// Basic structural checks
		assert_eq!(
			original_network.nodes.len(),
			converted_network.nodes.len(),
			"{}: Node count should be preserved",
			artwork_name
		);

		assert_eq!(
			original_network.exports.len(),
			converted_network.exports.len(),
			"{}: Export count should be preserved",
			artwork_name
		);

		// Verify each node's metadata is preserved
		for (node_id, orig_node) in &original_network.nodes {
			let conv_node = converted_network.nodes.get(node_id)
				.unwrap_or_else(|| panic!("{}: Node {:?} should exist after round-trip", artwork_name, node_id));

			// Check metadata fields
			assert_eq!(
				orig_node.call_argument, conv_node.call_argument,
				"{}: call_argument should be preserved for node {:?}", artwork_name, node_id
			);
			assert_eq!(
				orig_node.context_features, conv_node.context_features,
				"{}: context_features should be preserved for node {:?}", artwork_name, node_id
			);
			assert_eq!(
				orig_node.visible, conv_node.visible,
				"{}: visible should be preserved for node {:?}", artwork_name, node_id
			);
			assert_eq!(
				orig_node.skip_deduplication, conv_node.skip_deduplication,
				"{}: skip_deduplication should be preserved for node {:?}", artwork_name, node_id
			);

			// Check input count
			assert_eq!(
				orig_node.inputs.len(), conv_node.inputs.len(),
				"{}: Input count should be preserved for node {:?}", artwork_name, node_id
			);
		}

		// Verify the converted demo artwork can be compiled (demo artworks are complete networks)
		verify_network_compiles(&converted_network)
			.unwrap_or_else(|e| panic!("{}: Converted artwork should compile successfully: {}", artwork_name, e));

		println!("✓ {} passed", artwork_name);
	}
}
