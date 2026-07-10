//! Shared helpers for the storage integration tests ([`metadata_tests`](super::metadata_tests) and
//! [`round_trip_tests`](super::round_trip_tests)): loading demo artwork, walking every addressable
//! node, and pushing a document through a real `Gdd` save/reopen.

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNodeImplementation, NodeId};
use document_graph::PeerId;

use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{DocumentSettings, StorageMetadataView, build_interface_from_storage};

/// Load a demo `.graphite` straight into a `DocumentMessageHandler` for inspection.
pub fn load_demo(file_name: &str) -> DocumentMessageHandler {
	let path = format!("../demo-artwork/{file_name}");
	let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
	DocumentMessageHandler::deserialize_document(&content).unwrap_or_else(|e| panic!("Failed to deserialize {path}: {e:?}"))
}

/// Walk every node in every nested network and collect `(network_path, local_id)` pairs, so a test can
/// iterate every node addressable from the metadata side.
pub fn node_paths(interface: &NodeNetworkInterface) -> Vec<(Vec<NodeId>, NodeId)> {
	fn walk(interface: &NodeNetworkInterface, path: Vec<NodeId>, out: &mut Vec<(Vec<NodeId>, NodeId)>) {
		let Some(network) = interface.nested_network(&path) else { return };
		for (&local_id, node) in &network.nodes {
			out.push((path.clone(), local_id));

			if matches!(node.implementation, DocumentNodeImplementation::Network(_)) {
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

/// Result of round-tripping a document through a `Gdd`: the runtime interface rebuilt from the reopened
/// storage, the reopened registry, and the reopened per-peer view settings (`ui::doc::*`).
pub struct RoundTrip {
	pub rebuilt: NodeNetworkInterface,
	pub registry: document_graph::Registry,
	pub view_settings: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Push `document`'s current runtime state through a fresh in-memory `Gdd` and reopen it. The commit goes
/// through `commit_from_runtime` (the real autosave path: stage hot ops -> retire -> persist); the reopen
/// reads the working copy back from the same container bytes, exercising the whole codec / file / replay
/// pipeline rather than just the in-memory conversion.
pub async fn round_trip_through_gdd(document: &DocumentMessageHandler) -> RoundTrip {
	let byte_store = HashMapResourceStorage::new();

	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into())
		.await
		.expect("create_in");

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
