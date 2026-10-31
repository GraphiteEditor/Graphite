//! Shared helpers for the storage integration tests ([`metadata_tests`](super::metadata_tests) and
//! [`round_trip_tests`](super::round_trip_tests)): loading demo artwork, walking every addressable
//! node, and pushing a document through a real `Gdd` save/reopen.

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::PeerId;
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork, OwnedNetworkLocation};

use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{DocumentSettings, StorageMetadataView, build_interface_from_storage};
use crate::messages::portfolio::document::utility_types::network_interface::{NodeLocator, NodeNetworkInterface};

/// Load a demo `.graphite` straight into a `DocumentMessageHandler` for inspection, with its nested networks lifted into the definitions table.
pub fn load_demo(file_name: &str) -> DocumentMessageHandler {
	let path = format!("../demo-artwork/{file_name}");
	let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
	let mut document = DocumentMessageHandler::deserialize_document(&content).unwrap_or_else(|e| panic!("Failed to deserialize {path}: {e:?}"));
	document.network_interface.lift_nested_networks();
	document
}

/// Open a demo `.graphite` the way the editor does, with its migrations and load-time fix-ups.
pub fn open_demo(file_name: &str) -> DocumentMessageHandler {
	let path = format!("../demo-artwork/{file_name}");
	let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
	crate::messages::portfolio::document_storage_io::load_legacy_document(content, &HashMapResourceStorage::new(), false).unwrap_or_else(|e| panic!("Failed to open {path}: {e:?}"))
}

/// Load a demo `.graphite` and describe every subgraph node's outputs, which the demos predate, so a round trip has output descriptions to carry.
pub fn load_demo_with_output_descriptions(file_name: &str) -> DocumentMessageHandler {
	let mut document = load_demo(file_name);
	let interface = &mut document.network_interface;

	for (network_path, node_id) in node_paths(interface) {
		let node_path = [network_path.as_slice(), &[node_id]].concat();
		let Some(export_count) = interface.nested_network(&node_path).map(|network| network.exports.len()) else {
			continue;
		};

		let output_descriptions = (0..export_count).map(|index| format!("Output {index} of node {node_id}.")).collect();
		if let Some(mut node) = interface.node_mut(NodeLocator::new(node_id, &network_path)) {
			node.set_output_descriptions(output_descriptions);
		}
	}

	document
}

/// Walk every node in every nested network and collect `(network_path, local_id)` pairs, so a test can
/// iterate every node addressable from the metadata side.
pub fn node_paths(interface: &NodeNetworkInterface) -> Vec<(Vec<NodeId>, NodeId)> {
	fn walk(interface: &NodeNetworkInterface, path: Vec<NodeId>, out: &mut Vec<(Vec<NodeId>, NodeId)>) {
		let Some(network) = interface.nested_network(&path) else { return };
		for (&local_id, node) in &network.nodes {
			out.push((path.clone(), local_id));

			if matches!(node.implementation, DocumentNodeImplementation::Network(_) | DocumentNodeImplementation::Definition(_)) {
				let mut child = path.clone();
				child.push(local_id);
				walk(interface, child, out);
			}
		}
	}

	let mut out = Vec::new();
	walk(interface, Vec::new(), &mut out);
	out
}

/// Every network the document holds, once each: the document network, its definitions, and the inline networks nested in either.
pub fn network_locations(interface: &NodeNetworkInterface) -> Vec<OwnedNetworkLocation> {
	fn walk(network: &NodeNetwork, location: OwnedNetworkLocation, out: &mut Vec<OwnedNetworkLocation>) {
		for (&local_id, node) in &network.nodes {
			if let DocumentNodeImplementation::Network(nested) = &node.implementation {
				walk(nested, location.as_location().child(local_id), out);
			}
		}
		out.push(location);
	}

	let document_network = interface.document_network();
	let mut out = Vec::new();
	walk(document_network, OwnedNetworkLocation::default(), &mut out);
	for (&id, definition) in &document_network.definitions {
		walk(definition, OwnedNetworkLocation::definition(id), &mut out);
	}
	out
}

/// Every node the document holds, once each, addressed by the location of the network holding it.
pub fn node_locations(interface: &NodeNetworkInterface) -> Vec<(OwnedNetworkLocation, NodeId)> {
	network_locations(interface)
		.into_iter()
		.flat_map(|location| {
			let node_ids: Vec<NodeId> = located_network(interface, &location).map(|network| network.nodes.keys().copied().collect()).unwrap_or_default();
			node_ids.into_iter().map(move |node_id| (location.clone(), node_id))
		})
		.collect()
}

pub fn located_network<'a>(interface: &'a NodeNetworkInterface, location: &OwnedNetworkLocation) -> Option<&'a NodeNetwork> {
	interface.document_network().located_network(location.as_location())
}

pub fn located_node<'a>(interface: &'a NodeNetworkInterface, location: &OwnedNetworkLocation, node_id: NodeId) -> Option<&'a DocumentNode> {
	located_network(interface, location)?.nodes.get(&node_id)
}

/// Result of round-tripping a document through a `Gdd`: the runtime interface rebuilt from the reopened
/// storage, the reopened registry, and the reopened per-peer view settings (`ui::doc::*`).
pub struct RoundTrip {
	pub rebuilt: NodeNetworkInterface,
	pub registry: document_graph_storage::Registry,
	pub view_settings: std::collections::BTreeMap<String, document_graph_storage::Value>,
}

/// Push `document`'s current runtime state through a fresh in-memory `Gdd` and reopen it. The commit goes
/// through `commit_from_runtime` (the real autosave path: stage hot ops -> retire -> persist); the reopen
/// reads the working copy back from the same container bytes, exercising the whole codec / file / replay
/// pipeline rather than just the in-memory conversion.
pub async fn round_trip_through_gdd(document: &DocumentMessageHandler) -> RoundTrip {
	let byte_store = HashMapResourceStorage::new();

	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into()).expect("create_in");

	let network = document.network_interface.document_network().clone();
	let view = StorageMetadataView::new(&document.network_interface);
	gdd.commit_from_runtime(&network, &view, &document.resources.registry, &byte_store).expect("commit_from_runtime");

	// Per-peer view settings persist in session.json, separate from the registry commit.
	let view_settings = DocumentSettings {
		document_ptz: &document.document_ptz,
		render_mode: &document.render_mode,
		overlays_visibility: &document.overlays_visibility_settings,
		rulers_visible: document.rulers_visible,
		snapping_state: &document.snapping_state,
		collapsed: &document.collapsed,
	}
	.to_view_map();
	gdd.set_view_settings(view_settings).expect("set_view_settings");

	let (working, layout) = gdd.into_storage();
	let reopened = GddV1::open_in(working, layout).await.expect("open_in");
	let declarations = reopened.declarations(&byte_store).await;
	let registry = reopened.registry().clone();
	let (rebuilt_network, node_entries, network_entries) = registry.to_runtime_with_full_metadata(&declarations).expect("to_runtime_with_full_metadata");
	let rebuilt = build_interface_from_storage(rebuilt_network, node_entries, network_entries).expect("build_interface_from_storage");
	let view_settings = reopened.view_settings().clone();

	RoundTrip { rebuilt, registry, view_settings }
}
