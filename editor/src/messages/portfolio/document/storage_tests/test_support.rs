//! Shared helpers for the storage integration tests ([`metadata_tests`](super::metadata_tests) and
//! [`round_trip_tests`](super::round_trip_tests)): loading demo artwork, walking every addressable
//! node, and pushing a document through a real `Gdd` save/reopen.

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{PeerId, UserId};
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkLocation, NodeId, NodeNetwork};

use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::misc::DocumentId;
use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{DocumentSettings, StorageMetadataView, build_interface_from_storage};
use crate::messages::portfolio::document::utility_types::network_interface::{NodeLocator, NodeNetworkInterface};
use crate::messages::portfolio::document_storage_io::load_from_container;
use crate::messages::resource_storage::ResourcesHandle;
use std::sync::Arc;

/// Open a demo artwork the way the editor opens a document file.
pub fn open_demo(file_name: &str) -> DocumentMessageHandler {
	let path = format!("../demo-artwork/{file_name}");
	let archive = std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));

	let mut container = AnyContainer::Memory(MemoryBackend::new());
	document_container::archive::open_auto(&archive, &mut container).unwrap_or_else(|e| panic!("Failed to unpack {path}: {e}"));

	let resources = ResourcesHandle::new(Arc::new(HashMapResourceStorage::new()));
	let (document, _) = futures::executor::block_on(load_from_container(container, DocumentId(0), &resources, false));
	document.unwrap_or_else(|| panic!("Failed to open {path}"))
}

/// Open a demo artwork, describe every subgraph node's outputs, and name one subgraph node's inputs, which the demos leave blank, so a round trip has an interface to carry.
pub fn open_demo_with_named_subgraph_interfaces(file_name: &str) -> DocumentMessageHandler {
	let mut document = open_demo(file_name);
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

	// Built-in subgraphs take their interface from the catalog, so this forks one into a copy the document stores.
	// It picks one sharing its catalog entry with another node, so the fork leaves no unused entry behind.
	let root_nodes = &interface.document_network().nodes;
	let shared_subgraph = root_nodes.iter().find_map(|(node_id, node)| {
		let is_built_in = interface.is_read_only(&[*node_id]);
		let is_shared = root_nodes.values().filter(|other| other.implementation == node.implementation).count() > 1;
		(is_built_in && is_shared).then_some((*node_id, node.inputs.len()))
	});
	if let Some((node_id, import_count)) = shared_subgraph
		&& let Some(mut node) = interface.node_interface_mut(NodeLocator::new(node_id, &[]))
	{
		for index in 0..import_count {
			node.set_input_name(index, format!("Input {index} of node {node_id}"));
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

/// Every network the document holds, once each: the document network and its definitions.
pub fn network_locations(interface: &NodeNetworkInterface) -> Vec<NetworkLocation> {
	std::iter::once(None).chain(interface.document_network().definitions.keys().copied().map(Some)).collect()
}

/// Every node the document holds, once each.
/// Each node is addressed by the location of its network.
pub fn node_locations(interface: &NodeNetworkInterface) -> Vec<(NetworkLocation, NodeId)> {
	network_locations(interface)
		.into_iter()
		.flat_map(|location| {
			let node_ids: Vec<NodeId> = located_network(interface, &location).map(|network| network.nodes.keys().copied().collect()).unwrap_or_default();
			node_ids.into_iter().map(move |node_id| (location, node_id))
		})
		.collect()
}

pub fn located_network<'a>(interface: &'a NodeNetworkInterface, location: &NetworkLocation) -> Option<&'a NodeNetwork> {
	interface.document_network().located_network(*location)
}

pub fn located_node<'a>(interface: &'a NodeNetworkInterface, location: &NetworkLocation, node_id: NodeId) -> Option<&'a DocumentNode> {
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

	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), UserId(1), 0xABCD, "test".into(), "test".into()).expect("create_in");

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
	let (rebuilt_network, node_entries, network_entries) = registry.to_runtime_with_full_metadata(&declarations, reopened.blobs()).expect("to_runtime_with_full_metadata");
	let rebuilt = build_interface_from_storage(rebuilt_network, node_entries, network_entries);
	let view_settings = reopened.view_settings().clone();

	RoundTrip { rebuilt, registry, view_settings }
}
