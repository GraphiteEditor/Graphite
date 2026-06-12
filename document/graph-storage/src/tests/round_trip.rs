use std::borrow::Cow;
use std::collections::HashMap;

use core_types::context::ContextDependencies;
use core_types::uuid::NodeId;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::{ProtoNodeIdentifier, Type, concrete};

use crate::{NodeMetadataSource, PeerId, Position, Registry};

/// Helper function to verify a NodeNetwork can be compiled successfully.
/// Note: This only works for complete networks with all inputs resolved.
/// Test networks with Import inputs will fail compilation (which is expected).
fn verify_network_compiles(network: &NodeNetwork) -> Result<(), String> {
	let compiler = Compiler {};
	compiler.compile_single(network.clone()).map_err(|e| format!("Compilation failed: {:?}", e))?;
	Ok(())
}

/// Convert a runtime network to a storage `Registry`, returning the declarations alongside it.
/// Proto-node declaration content is no longer stored in the registry (it lives in a byte store);
/// these tests have no byte store, so they keep the extracted bytes in hand and rebuild a
/// `Declarations` map for the back-conversion.
fn to_registry(network: &NodeNetwork) -> (Registry, crate::Declarations) {
	let conversion = Registry::convert_from_runtime(network, &crate::NoMetadata, &Default::default(), PeerId(0)).expect("Failed to convert NodeNetwork to Registry");
	let declarations = conversion.declarations().expect("rebuild declarations");
	(conversion.registry, declarations)
}

/// A one-node network whose single node references `id` via a `TaggedValue::Resource` input, so
/// `convert_resources` (which only snapshots network-referenced resources) carries the resource.
fn network_referencing_resource(id: graphene_resource::ResourceId) -> NodeNetwork {
	network_referencing_resources(&[id])
}

/// A network with one node per resource, each referencing its resource via a `TaggedValue::Resource`
/// input, so all listed resources are network-referenced and survive conversion.
fn network_referencing_resources(ids: &[graphene_resource::ResourceId]) -> NodeNetwork {
	use graph_craft::document::value::TaggedValue;

	let nodes = ids
		.iter()
		.enumerate()
		.map(|(i, id)| {
			(
				NodeId(i as u64),
				DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::Resource(*id), false)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
					..Default::default()
				},
			)
		})
		.collect();

	NodeNetwork { nodes, ..Default::default() }
}

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
	let (registry, declarations) = to_registry(&original_network);

	// Convert back to NodeNetwork
	let (converted_network, _) = registry.to_runtime_with_metadata(&declarations).expect("Failed to convert Registry back to NodeNetwork");

	// Verify structure is preserved
	assert_eq!(converted_network.nodes.len(), original_network.nodes.len(), "Node count should be preserved");
	assert_eq!(converted_network.exports.len(), original_network.exports.len(), "Export count should be preserved");

	// Verify exports reference the correct nodes
	match (&original_network.exports[0], &converted_network.exports[0]) {
		(
			NodeInput::Node {
				node_id: orig_id,
				output_index: orig_idx,
			},
			NodeInput::Node {
				node_id: conv_id,
				output_index: conv_idx,
			},
		) => {
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
	let (registry, declarations) = to_registry(&original_network);

	// Convert back to NodeNetwork
	let (converted_network, _) = registry.to_runtime_with_metadata(&declarations).expect("Failed to convert Registry back to NodeNetwork");

	// Verify structure is preserved
	assert_eq!(converted_network.nodes.len(), original_network.nodes.len(), "Node count should be preserved");

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

	let (registry, _declarations) = to_registry(&network);

	assert!(registry.resources.len() >= 2, "Should have proto-node declaration resources");
	assert!(!registry.networks.is_empty(), "Should have at least one network");

	let root_network = registry.networks.get(&crate::ROOT_NETWORK).expect("Root network should exist");
	assert_eq!(root_network.exports.len(), network.exports.len(), "Export count should match");

	// Exports are first-class slots, no synthetic identity nodes in node_instances.
	for slot in &root_network.exports {
		assert!(slot.target.is_some(), "Round-tripped exports should have a target");
	}
}

#[test]
fn test_nested_network_flattening() {
	let network = create_nested_network();

	let registry = Registry::try_from(&network).expect("Failed to convert to Registry");

	// Outer network has 2 nodes, one of which contains a nested network with 1 node.
	// No more identity-node padding, so node_instances has exactly the real nodes.
	let expected_nodes = 3;
	assert_eq!(
		registry.node_instances.len(),
		expected_nodes,
		"Registry should have exactly {} nodes, found {}",
		expected_nodes,
		registry.node_instances.len()
	);

	// Two networks: root (ROOT_NETWORK) and nested (1).
	assert!(registry.networks.len() >= 2, "Should have at least 2 networks (root + nested)");
}

#[test]
fn test_metadata_preservation() {
	// Create a network with nodes that have non-default metadata
	let context_features = ContextDependencies {
		extract: core_types::context::ContextFeatures::FOOTPRINT | core_types::context::ContextFeatures::REAL_TIME,
		..Default::default()
	};

	let network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(f64), 0), NodeInput::import(Type::Generic(Cow::Borrowed("T")), 1)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("test::NodeWithMetadata")),
					call_argument: concrete!(String),
					context_features,
					visible: false,           // Non-default value
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
	let (registry, declarations) = to_registry(&network);
	let (converted, _) = registry.to_runtime_with_metadata(&declarations).expect("Failed to convert back to NodeNetwork");

	// Verify call_argument is preserved
	let orig_node_0 = network.nodes.get(&NodeId(0)).unwrap();
	let conv_node_0 = converted.nodes.get(&NodeId(0)).unwrap();
	assert_eq!(orig_node_0.call_argument, conv_node_0.call_argument, "call_argument for node 0 should be preserved");

	let orig_node_1 = network.nodes.get(&NodeId(1)).unwrap();
	let conv_node_1 = converted.nodes.get(&NodeId(1)).unwrap();
	assert_eq!(orig_node_1.call_argument, conv_node_1.call_argument, "call_argument for node 1 should be preserved");

	// Verify context_features is preserved
	assert_eq!(orig_node_0.context_features, conv_node_0.context_features, "context_features should be preserved");

	// Verify visible is preserved
	assert_eq!(orig_node_0.visible, conv_node_0.visible, "visible should be preserved");

	// Verify skip_deduplication is preserved
	assert_eq!(orig_node_0.skip_deduplication, conv_node_0.skip_deduplication, "skip_deduplication should be preserved");

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
	use graph_craft::util::{DEMO_ART, load_from_name};

	// Test each demo artwork
	for artwork_name in DEMO_ART {
		println!("Testing artwork: {}", artwork_name);

		let original_network = load_from_name(artwork_name);

		// Convert to Registry
		let (registry, declarations) = to_registry(&original_network);

		// Convert back to NodeNetwork
		let (converted_network, _) = registry
			.to_runtime_with_metadata(&declarations)
			.unwrap_or_else(|e| panic!("Failed to convert {} back to NodeNetwork: {:?}", artwork_name, e));

		// Basic structural checks
		assert_eq!(original_network.nodes.len(), converted_network.nodes.len(), "{}: Node count should be preserved", artwork_name);

		assert_eq!(original_network.exports.len(), converted_network.exports.len(), "{}: Export count should be preserved", artwork_name);

		// Verify each node's metadata is preserved
		for (node_id, orig_node) in &original_network.nodes {
			let conv_node = converted_network
				.nodes
				.get(node_id)
				.unwrap_or_else(|| panic!("{}: Node {:?} should exist after round-trip", artwork_name, node_id));

			// Check metadata fields
			assert_eq!(
				orig_node.call_argument, conv_node.call_argument,
				"{}: call_argument should be preserved for node {:?}",
				artwork_name, node_id
			);
			assert_eq!(
				orig_node.context_features, conv_node.context_features,
				"{}: context_features should be preserved for node {:?}",
				artwork_name, node_id
			);
			assert_eq!(orig_node.visible, conv_node.visible, "{}: visible should be preserved for node {:?}", artwork_name, node_id);
			assert_eq!(
				orig_node.skip_deduplication, conv_node.skip_deduplication,
				"{}: skip_deduplication should be preserved for node {:?}",
				artwork_name, node_id
			);

			// Check input count
			assert_eq!(
				orig_node.inputs.len(),
				conv_node.inputs.len(),
				"{}: Input count should be preserved for node {:?}",
				artwork_name,
				node_id
			);
		}

		// Verify the converted demo artwork can be compiled (demo artworks are complete networks)
		verify_network_compiles(&converted_network).unwrap_or_else(|e| panic!("{}: Converted artwork should compile successfully: {}", artwork_name, e));

		println!("✓ {} passed", artwork_name);
	}
}

/// Per-node UI state used by the in-test metadata source. Keyed by `(network_path, local_id)`.
#[derive(Clone, Debug, Default, PartialEq)]
struct UiState {
	position: Option<Position>,
	is_layer: bool,
	display_name: Option<String>,
	locked: bool,
	pinned: bool,
}

/// In-test `NodeMetadataSource` backed by a `HashMap` keyed on the full `(network_path, local_id)`
/// addressing the editor would use.
struct TestMetadata {
	entries: HashMap<(Vec<NodeId>, NodeId), UiState>,
}

impl TestMetadata {
	fn new() -> Self {
		Self { entries: HashMap::new() }
	}

	fn insert(&mut self, network_path: &[NodeId], local_id: NodeId, state: UiState) {
		self.entries.insert((network_path.to_vec(), local_id), state);
	}

	fn get(&self, network_path: &[NodeId], local_id: NodeId) -> Option<&UiState> {
		self.entries.get(&(network_path.to_vec(), local_id))
	}
}

impl NodeMetadataSource for TestMetadata {
	fn position(&self, network_path: &[NodeId], local_id: NodeId) -> Option<Position> {
		self.get(network_path, local_id).and_then(|s| s.position)
	}
	fn is_layer(&self, network_path: &[NodeId], local_id: NodeId) -> bool {
		self.get(network_path, local_id).is_some_and(|s| s.is_layer)
	}
	fn display_name(&self, network_path: &[NodeId], local_id: NodeId) -> Option<&str> {
		self.get(network_path, local_id).and_then(|s| s.display_name.as_deref())
	}
	fn locked(&self, network_path: &[NodeId], local_id: NodeId) -> bool {
		self.get(network_path, local_id).is_some_and(|s| s.locked)
	}
	fn pinned(&self, network_path: &[NodeId], local_id: NodeId) -> bool {
		self.get(network_path, local_id).is_some_and(|s| s.pinned)
	}
}

/// Round-trips a nested network with editor metadata: layer + absolute position on one node,
/// node-in-chain on another, layer-in-stack inside a nested network. Asserts every entry comes
/// back unchanged and addressed by the correct `(network_path, local_id)`.
#[test]
fn test_ui_metadata_round_trip() {
	let network = create_nested_network();

	let mut metadata = TestMetadata::new();

	// Root-network node 0 (the one with a nested network): a layer at an absolute position with
	// a display name. Editor `network_path` for root-network nodes is empty.
	metadata.insert(
		&[],
		NodeId(0),
		UiState {
			position: Some(Position::Absolute([3, 5])),
			is_layer: true,
			display_name: Some("Outer layer".into()),
			locked: true,
			pinned: false,
		},
	);

	// Root-network node 1: a plain node in a chain.
	metadata.insert(
		&[],
		NodeId(1),
		UiState {
			position: Some(Position::Chain),
			..Default::default()
		},
	);

	// Nested-network node 10 (lives under node 0): a layer in a stack.
	metadata.insert(
		&[NodeId(0)],
		NodeId(10),
		UiState {
			position: Some(Position::Stack(7)),
			is_layer: true,
			..Default::default()
		},
	);

	let conversion = Registry::convert_from_runtime(&network, &metadata, &Default::default(), PeerId(0)).expect("Failed to convert to Registry with metadata");
	let declarations = conversion.declarations().expect("rebuild declarations");
	let registry = conversion.registry;

	let (converted, entries) = registry.to_runtime_with_metadata(&declarations).expect("Failed to convert Registry back with metadata");

	// Graph structure still round-trips.
	assert_eq!(converted.nodes.len(), network.nodes.len());

	// Three entries — one per node we attached metadata to.
	assert_eq!(entries.len(), 3, "expected 3 metadata entries, got {}: {entries:#?}", entries.len());

	// Look entries back up by their address so we don't rely on emission order.
	let lookup: HashMap<(Vec<NodeId>, NodeId), &crate::NodeMetadataEntry> = entries.iter().map(|e| ((e.network_path.clone(), e.local_id), e)).collect();

	let root_layer = lookup.get(&(vec![], NodeId(0))).expect("entry for root-network layer node missing");
	assert_eq!(root_layer.position, Some(Position::Absolute([3, 5])));
	assert!(root_layer.is_layer);
	assert_eq!(root_layer.display_name.as_deref(), Some("Outer layer"));
	assert!(root_layer.locked);
	assert!(!root_layer.pinned);

	let root_node = lookup.get(&(vec![], NodeId(1))).expect("entry for root-network chain node missing");
	assert_eq!(root_node.position, Some(Position::Chain));
	assert!(!root_node.is_layer);

	let nested_layer = lookup.get(&(vec![NodeId(0)], NodeId(10))).expect("entry for nested layer-in-stack missing");
	assert_eq!(nested_layer.position, Some(Position::Stack(7)));
	assert!(nested_layer.is_layer);
}

/// A runtime `ResourceRegistry` (source chain + resolved hash) survives conversion into the storage
/// `Registry`: source bodies are preserved in priority order and the hash carries through.
#[test]
fn resources_round_trip_through_from_runtime() {
	use graphene_resource::{DataSource, ResourceHash, ResourceId, ResourceRegistry};

	let mut resources = ResourceRegistry::new();
	let id = ResourceId::new();
	// Two sources in chain order: an embedded fallback then a URL.
	resources.push_source_back(&id, DataSource::Embedded);
	resources.push_source_back(&id, DataSource::Url("https://example.com/img.png".parse().unwrap()));
	let hash = ResourceHash::from(&b"image bytes"[..]);
	resources.resolve(&id, hash);

	// The resource must be referenced by a node to be snapshotted: `convert_resources` only carries
	// resources the network uses (orphans in the runtime cache, e.g. retained across undo, are dropped).
	let network = network_referencing_resource(id);

	let registry = Registry::from_runtime_with_metadata(&network, &crate::NoMetadata, &resources, PeerId(7)).expect("from_runtime failed");

	let entry = registry.resources.get(&id).expect("resource entry present in storage registry");
	assert_eq!(entry.hash, Some(hash), "resolved hash carried through");
	assert_eq!(entry.sources.len(), 2, "both sources carried through");

	// The chain iterates in priority order; decode bodies back to DataSource to compare.
	let decoded: Vec<DataSource> = entry.sources.iter().map(|(_, v)| serde_json::from_value(v.source.clone()).expect("source body decodes")).collect();
	assert_eq!(decoded, vec![DataSource::Embedded, DataSource::Url("https://example.com/img.png".parse().unwrap())]);

	// All source keys carry the document peer.
	assert!(entry.sources.iter().all(|(key, _)| key.peer == PeerId(7)), "source keys scoped to the document peer");
}

/// Full resource round-trip: a runtime `ResourceRegistry` converted into storage and back is equal
/// to the original (source chains in order, resolved hashes preserved).
#[test]
fn resource_registry_round_trips_runtime_to_storage_to_runtime() {
	use graphene_resource::{DataSource, ResourceHash, ResourceId, ResourceRegistry};

	let mut original = ResourceRegistry::new();

	// A resolved resource with a two-entry fallback chain.
	let image = ResourceId::new();
	original.push_source_back(&image, DataSource::Embedded);
	original.push_source_back(&image, DataSource::Url("https://example.com/img.png".parse().unwrap()));
	original.resolve(&image, ResourceHash::from(&b"image bytes"[..]));

	// An unresolved resource (sources but no hash yet).
	let font = ResourceId::new();
	original.push_source_back(
		&font,
		DataSource::Font {
			family: "Inter".into(),
			style: Some("Bold".into()),
		},
	);

	// Both resources must be referenced by a node to be snapshotted (see `convert_resources`).
	let network = network_referencing_resources(&[image, font]);

	let registry = Registry::from_runtime_with_metadata(&network, &crate::NoMetadata, &original, PeerId(3)).expect("from_runtime failed");
	let restored = registry.to_resource_registry().expect("to_resource_registry failed");

	// Compare the two document resources specifically; the referencing nodes' proto-node declarations
	// also become resources in the registry, so the restored set is a superset of `original`.
	for id in [image, font] {
		assert_eq!(
			restored.info(&id).map(|info| info.sources),
			original.info(&id).map(|info| info.sources),
			"sources for {id:?} did not survive the round-trip"
		);
		assert_eq!(
			restored.info(&id).and_then(|info| info.hash.copied()),
			original.info(&id).and_then(|info| info.hash.copied()),
			"resolved hash for {id:?} did not survive the round-trip"
		);
	}
}

/// A resource present in the runtime cache but not referenced by any node is *not* snapshotted into the
/// storage registry. This is the orphan case: undoing an image paste removes the node but the runtime
/// keeps the resource alive for redo, so a later diff must not see the orphan as a new `AddResource`
/// (which would resurface the undone paste as a phantom interaction). Regression guard for that divergence.
#[test]
fn unreferenced_runtime_resource_is_not_snapshotted() {
	use graphene_resource::{DataSource, ResourceHash, ResourceId, ResourceRegistry};

	let referenced = ResourceId::new();
	let orphan = ResourceId::new();

	let mut resources = ResourceRegistry::new();
	for id in [referenced, orphan] {
		resources.push_source_back(&id, DataSource::Embedded);
		resources.resolve(&id, ResourceHash::from(&b"bytes"[..]));
	}

	// Only `referenced` is wired to a node; `orphan` lingers in the cache (as it would after an undo).
	let network = network_referencing_resource(referenced);

	let registry = Registry::from_runtime_with_metadata(&network, &crate::NoMetadata, &resources, PeerId(1)).expect("from_runtime failed");

	assert!(registry.resources.contains_key(&referenced), "the network-referenced resource must be snapshotted");
	assert!(!registry.resources.contains_key(&orphan), "the unreferenced (orphan) resource must not be snapshotted");
}

/// A node-input `TaggedValue::F64` must survive the storage round-trip bit-exact. Inputs are stored as a
/// self-describing `serde_json::Value` (encoded with the registry's MessagePack codec), so this guards
/// against any precision loss in the f64 -> serde_json::Number -> f64 path for a value with a full
/// 17-significant-digit mantissa.
#[test]
fn node_input_f64_round_trips_bit_exact() {
	use graph_craft::document::value::TaggedValue;

	// A value whose exact f64 bits matter: 1/3-ish with a non-terminating binary expansion.
	let precise = 107.33334350585939_f64;
	let network = NodeNetwork {
		nodes: [(
			NodeId(0),
			DocumentNode {
				inputs: vec![NodeInput::value(TaggedValue::F64(precise), false)],
				implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	};

	let (registry, declarations) = to_registry(&network);
	let (converted, _) = registry.to_runtime_with_metadata(&declarations).expect("to_runtime");

	let input = &converted.nodes.get(&NodeId(0)).expect("node 0").inputs[0];
	let NodeInput::Value { tagged_value, .. } = input else {
		panic!("expected a value input, got {input:?}")
	};
	let TaggedValue::F64(actual) = &**tagged_value else {
		panic!("expected F64, got {:?}", tagged_value)
	};

	assert_eq!(actual.to_bits(), precise.to_bits(), "f64 node input drifted: {actual} != {precise}");
}

/// Two storage nodes in one network carrying the same `ORIGINAL_NODE_ID` both map to one runtime ID.
/// Conversion must reject this rather than silently collapse them and drop a node.
#[test]
fn duplicate_runtime_node_id_is_rejected() {
	use crate::AttributesWrite;
	use crate::TimeStamp;
	use crate::to_runtime::ConversionError;

	let (mut registry, declarations) = to_registry(&create_simple_network());

	// Force both root-network nodes onto the same runtime ID.
	for node in registry.node_instances.values_mut() {
		node.attributes.set(crate::attr::node::ORIGINAL_NODE_ID, serde_json::json!(7), TimeStamp::ORIGIN);
	}

	let error = registry.to_runtime_with_metadata(&declarations).expect_err("duplicate runtime ID must error");
	assert!(
		matches!(error, ConversionError::DuplicateRuntimeNodeId { runtime_id: 7, .. }),
		"expected DuplicateRuntimeNodeId, got {error:?}"
	);
}

/// A node input referencing a node in a different network can't be remapped to a valid local runtime
/// ID, so conversion must reject it rather than emit a dangling reference.
#[test]
fn cross_network_reference_is_rejected() {
	use crate::to_runtime::ConversionError;
	use crate::{Network, NodeInput};

	let (mut registry, declarations) = to_registry(&create_simple_network());

	// `create_simple_network` wires one node's input to another, both in the root network. Find the
	// referenced storage ID, then move that node into a fresh second network so the reference crosses
	// a network boundary.
	let referenced_storage_id = registry
		.node_instances
		.values()
		.flat_map(|node| node.inputs())
		.find_map(|slot| match slot.input {
			NodeInput::Node { id: node_id, .. } => Some(node_id),
			_ => None,
		})
		.expect("simple network has a node-to-node reference");

	let other_network = 999;
	registry.networks.insert(other_network, Network::default());
	registry.node_instances.get_mut(&referenced_storage_id).expect("referenced node exists").network = other_network;

	let error = registry.to_runtime_with_metadata(&declarations).expect_err("cross-network reference must error");
	assert!(matches!(error, ConversionError::CrossNetworkReference { .. }), "expected CrossNetworkReference, got {error:?}");
}

/// A network's `scope_injections` (key -> (NodeId, Type)) must survive a storage round trip, with the
/// node reference resolved back to the same runtime-local ID it pointed at originally.
#[test]
fn scope_injections_round_trip() {
	let mut network = create_simple_network();
	network.scope_injections.insert("editor-api".to_string(), (NodeId(0), concrete!(u32)));

	let (registry, declarations) = to_registry(&network);
	let (converted, _) = registry.to_runtime_with_metadata(&declarations).expect("to_runtime");

	let (node_id, ty) = converted.scope_injections.get("editor-api").expect("scope injection must survive the round trip");
	assert_eq!(*node_id, NodeId(0), "the injection's node reference must resolve back to its original runtime ID");
	assert_eq!(*ty, concrete!(u32), "the injection's type must be preserved");
}

/// A stored scope injection whose node reference no longer resolves (node removed, or moved to another
/// network) must error rather than emit an injection pointing at a nonexistent runtime node.
#[test]
fn dangling_scope_injection_is_rejected() {
	use crate::AttributesWrite;
	use crate::TimeStamp;
	use crate::to_runtime::ConversionError;

	let (mut registry, declarations) = to_registry(&create_simple_network());

	// Store an injection pointing at a storage ID that no node carries, leaving the reference dangling
	// while the rest of the graph stays valid. The root network is whichever one holds the nodes.
	let root_network_id = registry.node_instances.values().next().expect("simple network has nodes").network();
	let injections: HashMap<String, (crate::NodeId, Type)> = [("editor-api".to_string(), (u64::MAX, concrete!(u32)))].into_iter().collect();
	registry
		.networks
		.get_mut(&root_network_id)
		.expect("root network exists")
		.attributes
		.set_serialized(crate::attr::network::SCOPE_INJECTIONS, &injections, TimeStamp::ORIGIN)
		.expect("serialize injections");

	let error = registry.to_runtime_with_metadata(&declarations).expect_err("dangling scope injection must error");
	assert!(matches!(error, ConversionError::DanglingScopeInjection { .. }), "expected DanglingScopeInjection, got {error:?}");
}
