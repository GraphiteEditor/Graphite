//! End-to-end storage round-trip tests: drive real edits through the editor, push the document
//! through a fresh in-memory `Gdd` (stage → retire → persist), reopen from the same container, and
//! assert the reopened document matches. Exercises the full persistence pipeline (conversion,
//! MessagePack codecs, hot-op retirement, file layout, replay-on-open) that the debug-only
//! `verify_storage_round_trip` only checks in-process without an actual save/reopen.

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{NodeMetadataSource, PeerId};
use graph_craft::application_io::resource::HashMapResourceStorage;

use super::test_support::{RoundTrip, node_paths, round_trip_through_gdd};
use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{StorageMetadataView, build_interface_from_storage};
use crate::test_utils::test_prelude::*;
use graphene_std::vector::style::RenderMode;

/// Every node addressable in `original` resolves identically through the round-tripped interface:
/// the compute graph itself, then per-node positions, layer flags, names, locked/pinned.
fn assert_documents_match(original: &NodeNetworkInterface, round_trip: &RoundTrip) {
	let original_view = StorageMetadataView::new(original);
	let rebuilt_view = StorageMetadataView::new(&round_trip.rebuilt);

	assert_eq!(
		round_trip.rebuilt.document_network(),
		original.document_network(),
		"compute graph drifted through the storage round-trip"
	);

	for (network_path, local_id) in node_paths(original) {
		let at = format!("node {local_id:?} in network {network_path:?}");
		assert_eq!(rebuilt_view.position(&network_path, local_id), original_view.position(&network_path, local_id), "position: {at}");
		assert_eq!(rebuilt_view.is_layer(&network_path, local_id), original_view.is_layer(&network_path, local_id), "is_layer: {at}");
		assert_eq!(
			rebuilt_view.display_name(&network_path, local_id),
			original_view.display_name(&network_path, local_id),
			"display_name: {at}"
		);
		assert_eq!(rebuilt_view.locked(&network_path, local_id), original_view.locked(&network_path, local_id), "locked: {at}");
		assert_eq!(rebuilt_view.pinned(&network_path, local_id), original_view.pinned(&network_path, local_id), "pinned: {at}");
	}
}

#[tokio::test]
async fn round_trip_drawn_shapes() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(100., 200., 300., 400.).await;
	editor.draw_ellipse(120., 220., 280., 380.).await;
	editor.draw_polygon(50., 60., 250., 360.).await;

	let document = editor.active_document();
	let round_trip = round_trip_through_gdd(document).await;
	assert_documents_match(&document.network_interface, &round_trip);
}

#[tokio::test]
async fn round_trip_after_reorder() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor.draw_rect(50., 50., 150., 150.).await;
	editor.draw_rect(100., 100., 200., 200.).await;

	// Reorder: raise the bottom layer so the layer stack differs from creation order.
	let bottom = editor.active_document().metadata().all_layers().next_back().expect("at least one layer");
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![bottom.to_node()] }).await;
	editor.handle_message(DocumentMessage::SelectedLayersRaise).await;

	let document = editor.active_document();
	let round_trip = round_trip_through_gdd(document).await;
	assert_documents_match(&document.network_interface, &round_trip);
}

#[tokio::test]
async fn round_trip_grouped_into_nested_network() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor.draw_ellipse(50., 50., 150., 150.).await;

	// Group both layers into a folder (a nested network), exercising flat-Registry addressing across
	// depth and `NetworkId` round-tripping.
	let layers: Vec<_> = editor.active_document().metadata().all_layers().map(|layer| layer.to_node()).collect();
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: layers }).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: GroupFolderType::Layer,
		})
		.await;

	let document = editor.active_document();
	let round_trip = round_trip_through_gdd(document).await;
	assert_documents_match(&document.network_interface, &round_trip);
}

/// Reproduces the first-commit-after-open failure: after reopening a `.gdd` and rebuilding the
/// runtime, re-converting that runtime with `from_runtime` must reproduce the reopened registry.
/// This is exactly what `verify_storage_round_trip` does on the first autosave after opening a `.gdd`.
/// Uses a grouped document (nested network) because the bug is `from_runtime` assigning `NetworkId`s
/// by traversal order rather than reproducing the stored ones, which cascades into node-path hashes.
#[tokio::test]
async fn recommit_after_open_is_stable() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor.draw_ellipse(50., 50., 150., 150.).await;

	let layers: Vec<_> = editor.active_document().metadata().all_layers().map(|layer| layer.to_node()).collect();
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: layers }).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: GroupFolderType::Layer,
		})
		.await;

	let document = editor.active_document();
	let round_trip = round_trip_through_gdd(document).await;

	// Re-convert the rebuilt runtime and assert it reproduces the reopened registry. `round_trip_through_gdd`
	// builds the `Gdd` with `PeerId(1)`, which the reopened registry inherits, so the re-conversion must use
	// that same peer for deterministic node-ID derivation.
	let rebuilt_network = round_trip.rebuilt.document_network().clone();
	let view = StorageMetadataView::new(&round_trip.rebuilt);
	let reconverted = document_graph_storage::Registry::convert_from_runtime(&rebuilt_network, &view, &Default::default(), PeerId(1)).expect("re-convert from_runtime");

	assert!(
		round_trip.registry.value_equal(&reconverted.registry),
		"re-converting the reopened runtime drifted from the reopened registry (network/node IDs not stable across to_runtime -> from_runtime)\n{}",
		crate::messages::portfolio::document::document_diff::diff_registries(&round_trip.registry, &reconverted.registry)
	);
}

/// Reproduces the live "edit after opening a .gdd fails to commit" bug: build a grouped document,
/// persist + reopen it as the editor does (registry -> runtime), then edit and commit through the
/// reopened document. The commit must not fail with "Target node does not exist".
#[tokio::test]
async fn edit_after_open_commits_cleanly() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor.draw_ellipse(50., 50., 150., 150.).await;

	let layers: Vec<_> = editor.active_document().metadata().all_layers().map(|layer| layer.to_node()).collect();
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: layers }).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: GroupFolderType::Layer,
		})
		.await;

	// Persist the document into a fresh Gdd and reopen it, then build a runtime document from the
	// reopened registry: the editor's .gdd-open path.
	let byte_store = HashMapResourceStorage::new();
	let source = editor.active_document();
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into())
		.await
		.expect("create_in");
	let source_network = source.network_interface.document_network().clone();
	let source_view = StorageMetadataView::new(&source.network_interface);
	gdd.commit_from_runtime(&source_network, &source_view, &source.resources.registry, &byte_store)
		.expect("commit_from_runtime");

	let (working, layout) = gdd.into_storage();
	let reopened = GddV1::open_in(working, layout).await.expect("open_in");
	let declarations = reopened.declarations(&byte_store).await;
	let (rebuilt_network, node_entries, network_entries) = reopened.registry().to_runtime_with_full_metadata(&declarations).expect("to_runtime");
	let rebuilt = build_interface_from_storage(rebuilt_network, node_entries, network_entries).expect("build_interface_from_storage");

	// Install the rebuilt interface + reopened storage and finalize like the editor's open path does.
	{
		let document = editor.active_document_mut();
		document.network_interface = rebuilt;
		document.set_storage(Some(reopened));
		document.finalize_storage_load();
	}

	// Mirror the live sequence via the real `commit_storage_snapshot`: initial commit after open, then a
	// deletion, then commit again. Deletion (not addition) is the trigger, since it emits a `RemoveNode`
	// whose retire/reverse is where "Target node does not exist" surfaced live.
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);

	let layer = editor.active_document().metadata().all_layers().next().expect("at least one layer");
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;
	editor.handle_message(DocumentMessage::DeleteSelectedLayers).await;

	// `commit_storage_snapshot` only logs commit failures, so assert on the underlying `commit_from_runtime`
	// result directly to make the deletion commit a hard test failure rather than a silent log line.
	let document = editor.active_document_mut();
	// Clone the interface for the metadata view so it doesn't borrow `document` across the mutable
	// `storage_mut()` borrow below.
	let interface = document.network_interface.clone();
	let network = interface.document_network().clone();
	let view = StorageMetadataView::new(&interface);
	document
		.storage_mut()
		.expect("storage mounted")
		.commit_from_runtime(&network, &view, &Default::default(), &byte_store)
		.expect("commit after deleting a layer post-open must not fail");
}

/// Drives the per-interaction undo/redo cursor: commit two interactions into a `Gdd`, then undo back to the
/// one-interaction state and redo forward, asserting the registry's node set matches at each cursor
/// position. Exercises `Gdd::commit_from_runtime` (inline interaction-end mark) + `Session::undo/redo`.
#[tokio::test]
async fn gdd_undo_redo_walks_interactions() {
	let byte_store = HashMapResourceStorage::new();
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into())
		.await
		.expect("create_in");

	// Commit the active document's current runtime state as one interaction.
	async fn commit_interaction(gdd: &mut GddV1, document: &DocumentMessageHandler, byte_store: &HashMapResourceStorage) {
		let network = document.network_interface.document_network().clone();
		let view = StorageMetadataView::new(&document.network_interface);
		gdd.commit_from_runtime(&network, &view, &document.resources.registry, byte_store).expect("commit_from_runtime");
	}

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	editor.draw_rect(0., 0., 100., 100.).await;
	commit_interaction(&mut gdd, editor.active_document(), &byte_store).await;
	let nodes_after_one: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();

	editor.draw_ellipse(50., 50., 150., 150.).await;
	commit_interaction(&mut gdd, editor.active_document(), &byte_store).await;
	let nodes_after_two: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();

	assert!(nodes_after_two.len() > nodes_after_one.len(), "second interaction should add nodes");
	assert!(gdd.can_undo(), "two committed interactions should be undoable");
	assert!(!gdd.can_redo());

	// Undo the second interaction: registry returns to the one-interaction node set.
	gdd.undo().expect("undo");
	let undone: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();
	assert_eq!(undone, nodes_after_one, "undo should restore the post-first-interaction node set");
	assert!(gdd.can_redo(), "after undo, redo must be available");

	// Redo: back to the two-interaction state.
	gdd.redo().expect("redo");
	let redone: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();
	assert_eq!(redone, nodes_after_two, "redo should restore the post-second-interaction node set");

	// A new edit after undo clears the redo stack.
	gdd.undo().expect("undo");
	editor.draw_rect(200., 200., 300., 300.).await;
	commit_interaction(&mut gdd, editor.active_document(), &byte_store).await;
	assert!(!gdd.can_redo(), "a new edit after undo must abandon the redo branch");
}

/// Reopen must restore the cursor *and* a working registry consistent with it. `Session::load` trusts
/// the persisted `registry.bin` to match the persisted `head`, so an undo that rewinds the working
/// registry has to re-snapshot it. Repro of the live "undo acts like redo after reload" bug: commit
/// three interactions, undo one (registry at two), reopen, and assert the reopened registry is the
/// two-interaction state (not the three-interaction state the last retirement wrote) and that undo from there
/// still walks back to the one-interaction node set.
#[tokio::test]
async fn reopen_after_undo_restores_consistent_registry() {
	let byte_store = HashMapResourceStorage::new();
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into())
		.await
		.expect("create_in");

	async fn commit_interaction(gdd: &mut GddV1, document: &DocumentMessageHandler, byte_store: &HashMapResourceStorage) {
		let network = document.network_interface.document_network().clone();
		let view = StorageMetadataView::new(&document.network_interface);
		gdd.commit_from_runtime(&network, &view, &document.resources.registry, byte_store).expect("commit_from_runtime");
	}

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	editor.draw_rect(0., 0., 100., 100.).await;
	commit_interaction(&mut gdd, editor.active_document(), &byte_store).await;
	let nodes_one: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();

	editor.draw_ellipse(50., 50., 150., 150.).await;
	commit_interaction(&mut gdd, editor.active_document(), &byte_store).await;
	let nodes_two: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();

	editor.draw_rect(200., 200., 300., 300.).await;
	commit_interaction(&mut gdd, editor.active_document(), &byte_store).await;
	let nodes_three: std::collections::BTreeSet<_> = gdd.registry().node_instances.keys().copied().collect();

	// Undo the third interaction: the in-memory registry is back at the two-interaction state, but the last
	// retirement wrote the three-interaction registry to disk. The undo must re-snapshot it.
	gdd.undo().expect("undo");
	assert_eq!(gdd.registry().node_instances.keys().copied().collect::<std::collections::BTreeSet<_>>(), nodes_two);

	// Reopen from the same container: the cursor and the working registry are read back from disk.
	let (working, layout) = gdd.into_storage();
	let mut reopened = GddV1::open_in(working, layout).await.expect("open_in");

	assert_eq!(
		reopened.registry().node_instances.keys().copied().collect::<std::collections::BTreeSet<_>>(),
		nodes_two,
		"reopened registry must match the undone (two-interaction) state, not the three-interaction state on disk"
	);
	assert!(reopened.can_undo(), "head is still above the first interaction, so undo is available after reopen");
	assert!(reopened.can_redo(), "the undone third interaction is still redoable after reopen");

	// Redo after reopen: the persisted redo stack must still reach the three-interaction state (redo across
	// reopen must not be capped at the open point).
	reopened.redo().expect("redo after reopen");
	assert_eq!(
		reopened.registry().node_instances.keys().copied().collect::<std::collections::BTreeSet<_>>(),
		nodes_three,
		"redo after reopen must restore the third interaction, not stop at the reopened (two-interaction) state"
	);

	// Undo twice from there: back through the third interaction to the one-interaction state, not a redo.
	reopened.undo().expect("first undo after reopen redo");
	reopened.undo().expect("second undo after reopen redo");
	assert_eq!(
		reopened.registry().node_instances.keys().copied().collect::<std::collections::BTreeSet<_>>(),
		nodes_one,
		"undo after reopen must rewind toward the one-interaction state, not act like a redo"
	);
}

/// Drives the *live* shadow path: a real edit through the message pipeline (which fires
/// `CommitTransaction`, committing one interaction into the `Gdd`), then a real `DocumentMessage::Undo`
/// (which runs `shadow_storage_undo_redo` + the debug cursor-vs-runtime check). A clean run asserts
/// the `Gdd` undo cursor reproduces the legacy-restored interface.
#[tokio::test]
async fn live_undo_shadows_storage_cursor() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let byte_store = mount_in_memory_storage(&mut editor).await;
	// Capture the loaded state as the base interaction, then make a real edit (fires CommitTransaction).
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);
	let before_edit = editor.active_document().network_interface.document_network().clone();
	editor.draw_ellipse(50., 50., 150., 150.).await;
	let after_edit = editor.active_document().network_interface.document_network().clone();
	assert_ne!(&before_edit, &after_edit, "edit should change the network");

	// Real undo: the shadow drives gdd.undo() and (debug) asserts the cursor matches the restored interface.
	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(editor.active_document().network_interface.document_network(), &before_edit, "undo should restore the pre-edit network");

	// Real redo: the shadow drives gdd.redo() and must reproduce the post-edit network.
	editor.handle_message(DocumentMessage::Redo).await;
	assert_eq!(editor.active_document().network_interface.document_network(), &after_edit, "redo should restore the post-edit network");
}

#[tokio::test]
async fn round_trip_document_settings() {
	use document_graph_storage::attr::session::doc;

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	// Set distinctive, non-default document-level settings so the round-trip proves real values move.
	{
		let document = editor.active_document_mut();
		document.render_mode = RenderMode::Outline;
		document.rulers_visible = false;
		document.snapping_state.snapping_enabled = !document.snapping_state.snapping_enabled;
	}

	let document = editor.active_document();
	let round_trip = round_trip_through_gdd(document).await;
	assert_documents_match(&document.network_interface, &round_trip);

	// The `ui::doc::*` view settings survived the persist/reopen cycle (in session.json, not the registry).
	let settings = &round_trip.view_settings;
	assert_eq!(settings.get(doc::RENDER_MODE), Some(&serde_json::to_value(RenderMode::Outline).unwrap()), "render_mode");
	assert_eq!(settings.get(doc::RULERS_VISIBLE), Some(&serde_json::to_value(false).unwrap()), "rulers_visible");
	assert_eq!(settings.get(doc::SNAPPING), Some(&serde_json::to_value(&document.snapping_state).unwrap()), "snapping_state");
}

/// The exported `.gdd` archive must carry `session.json` so view settings (notably the document PTZ)
/// survive a save/open on another machine. `session.json` is working-copy-only otherwise, so without it
/// the archive opens with empty `view_settings` and the viewport resets to default. Exports to an archive
/// and reopens via `open_from_archive`, asserting the PTZ round-trips.
#[tokio::test]
async fn gdd_archive_round_trips_view_settings() {
	use document_graph_storage::attr::session::doc;

	let byte_store = HashMapResourceStorage::new();
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into())
		.await
		.expect("create_in");

	// Stage a distinctive PTZ into the working copy's `view_settings`, as `commit_storage_snapshot` does.
	let mut ptz = crate::messages::portfolio::document::utility_types::misc::PTZ::default();
	ptz.pan = glam::DVec2::new(-960., -540.);
	ptz.set_zoom(0.459);
	let view_settings = std::collections::BTreeMap::from([(doc::PTZ.to_string(), serde_json::to_value(ptz).unwrap())]);
	gdd.set_view_settings(view_settings).expect("set_view_settings");

	// Export to an in-memory archive, then reopen it from bytes into a fresh container.
	let archive = gdd
		.export_to_bytes(document_format::ExportFormat::Zip, document_format::ExportOptions::default(), &byte_store, None)
		.await
		.expect("export_to_bytes");

	let reopened = GddV1::open_from_archive(&archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout)
		.await
		.expect("open_from_archive");

	assert_eq!(
		reopened.view_settings().get(doc::PTZ),
		Some(&serde_json::to_value(ptz).unwrap()),
		"the document PTZ must survive a .gdd archive export/open (session.json travels in the archive)"
	);
}

/// Per-network node-graph navigation is per-peer view state: it must round-trip through `session.json`
/// (keyed by stable `NetworkId`), not the registry/history. Sets a distinctive node-graph PTZ on the root
/// network, persists + reopens through a `Gdd`, and asserts the nav is restored on the rebuilt interface
/// while being absent from the stored registry's attributes.
#[tokio::test]
async fn per_network_navigation_round_trips_via_session_not_registry() {
	use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{apply_network_view_settings, collect_network_view_settings, network_ids_from_entries};
	use document_graph_storage::attr::session::network;

	let byte_store = HashMapResourceStorage::new();

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	// Set a distinctive node-graph PTZ on the root network (path `[]`).
	let ptz = editor.active_document_mut().network_interface.node_graph_ptz_mut(&[]).expect("root network ptz");
	ptz.pan = glam::DVec2::new(-321., 654.);
	ptz.set_zoom(0.75);
	let expected_pan = editor.active_document().network_interface.node_graph_ptz(&[]).unwrap().pan;

	// Commit the document into a fresh `Gdd`, collecting the per-network view state the editor persists.
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0xABCD, "test".into(), "test".into())
		.await
		.expect("create_in");
	let document = editor.active_document();
	let network = document.network_interface.document_network().clone();
	let view = StorageMetadataView::new(&document.network_interface);
	gdd.commit_from_runtime(&network, &view, &document.resources.registry, &byte_store).expect("commit_from_runtime");

	let network_ids = gdd.network_ids(&network, &view).expect("network_ids");
	let network_view_settings = collect_network_view_settings(&document.network_interface, &network_ids);
	assert!(!network_view_settings.is_empty(), "the root network's non-default nav should be collected");
	gdd.set_network_view_settings(network_view_settings).expect("set_network_view_settings");

	// The registry must NOT carry the node-graph nav (it's per-peer, not document content).
	let root_network = gdd.registry().networks.get(&document_graph_storage::ROOT_NETWORK).expect("root network in registry");
	assert!(!root_network.attributes.contains_key(network::NAV_PTZ), "node-graph nav must not be stored in the registry attributes");

	// Reopen, rebuild the interface, and apply the persisted per-network view state.
	let (working, layout) = gdd.into_storage();
	let reopened = GddV1::open_in(working, layout).await.expect("open_in");
	let declarations = reopened.declarations(&byte_store).await;
	let (rebuilt_network, node_entries, network_entries) = reopened.registry().to_runtime_with_full_metadata(&declarations).expect("to_runtime");
	let mut rebuilt = build_interface_from_storage(rebuilt_network, node_entries, network_entries.clone()).expect("build_interface_from_storage");

	let reopened_ids = network_ids_from_entries(&network_entries);
	apply_network_view_settings(&mut rebuilt, &reopened_ids, reopened.network_view_settings());

	assert_eq!(
		rebuilt.node_graph_ptz(&[]).map(|ptz| ptz.pan),
		Some(expected_pan),
		"the node-graph PTZ must be restored from session.json on reopen"
	);
}

/// Mirror the real "new document, draw a rect, press undo" flow: mount storage and capture the
/// new-document base as the mount-time snapshot (as `DocumentStorageMounted` does), then draw a rect
/// (one `CommitTransaction` interaction) and undo. The shadow must reproduce the legacy-restored interface.
#[tokio::test]
async fn live_undo_new_document_draw_rect() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let byte_store = mount_in_memory_storage(&mut editor).await;
	// Mount-time snapshot: capture the new-document graph as the base interaction.
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);

	let before_rect = editor.active_document().network_interface.document_network().clone();
	editor.draw_rect(0., 0., 100., 100.).await;
	assert_ne!(&before_rect, editor.active_document().network_interface.document_network(), "drawing a rect should change the network");

	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(editor.active_document().network_interface.document_network(), &before_rect, "undo should restore the pre-rect network");
}

/// Pasting an image is one user action and must be one undo step: the paste handler brackets the layer add,
/// name set, reparent, and transform in a single transaction. Were the name set to open its own nested
/// transaction (a historical wart), the first undo would revert only the name and leave the layer behind, so
/// this asserts the layer count returns to its pre-paste value after exactly one undo.
#[tokio::test]
async fn paste_image_with_name_is_one_undo_step() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let layers_before = editor.active_document().metadata().all_layers().count();

	// Paste with a name so the handler emits the `SetDisplayName` sub-step that historically opened its
	// own transaction. `create_raster_image` passes `name: None` and so wouldn't exercise this path.
	let image = Image::new(2, 2, Color::WHITE);
	editor
		.handle_message(PortfolioMessage::InsertImage {
			name: Some("pasted".into()),
			image,
			mouse: None,
			parent_and_insert_index: None,
		})
		.await;
	assert_eq!(
		editor.active_document().metadata().all_layers().count(),
		layers_before + 1,
		"pasting an image should add exactly one layer"
	);

	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(
		editor.active_document().metadata().all_layers().count(),
		layers_before,
		"one undo should remove the whole pasted layer, not just its name"
	);
}

/// Undoing an image paste reverts the interaction's `AddResource` in the `Gdd` cursor while the runtime keeps
/// the resource alive for legacy redo, so the cursor legitimately holds fewer resources than a fresh
/// `from_runtime`. Drives the relaxed comparison: every resource the current network references must be in
/// the cursor, and any extra the runtime carries must be history-only.
#[tokio::test]
async fn undo_image_paste_resources_subset_of_runtime() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let byte_store = mount_in_memory_storage(&mut editor).await;
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);

	let image = Image::new(2, 2, Color::WHITE);
	editor
		.handle_message(PortfolioMessage::InsertImage {
			name: Some("pasted".into()),
			image,
			mouse: None,
			parent_and_insert_index: None,
		})
		.await;

	editor.handle_message(DocumentMessage::Undo).await;

	let document = editor.active_document();
	let storage = document.storage().expect("storage mounted");
	let stored: std::collections::BTreeSet<_> = storage.registry().resources.keys().copied().collect();

	// Every resource the restored network still references must be in the cursor.
	let current: std::collections::BTreeSet<_> = document.used_resources(false).iter().copied().collect();
	assert!(
		current.is_subset(&stored),
		"cursor is missing a resource the restored network references: current={current:?} stored={stored:?}"
	);

	// The runtime, which retains the undone paste's resource for redo, may carry strictly more.
	let runtime: std::collections::BTreeSet<_> = document.resources.registry.ids().collect();
	assert!(stored.is_subset(&runtime), "cursor holds a resource the runtime dropped: stored={stored:?} runtime={runtime:?}");
}

/// Two edits (draw, then paste) on an opened document are two undo steps; undoing twice must step the `Gdd`
/// cursor back two interactions in lockstep with the legacy path. The loaded base is not itself an undo
/// step, so the mount-time base must not become an undoable `Gdd` interaction, or the cursor lags the legacy
/// path by one once undo reaches the base. Reproduces the live "open, draw, paste image, undo twice" divergence.
#[tokio::test]
async fn undo_twice_steps_cursor_two_interactions() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let byte_store = mount_in_memory_storage(&mut editor).await;
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);
	let base = editor.active_document().network_interface.document_network().clone();

	editor.draw_rect(0., 0., 100., 100.).await;
	let after_rect = editor.active_document().network_interface.document_network().clone();

	let image = Image::new(2, 2, Color::WHITE);
	editor
		.handle_message(PortfolioMessage::InsertImage {
			name: Some("pasted".into()),
			image,
			mouse: None,
			parent_and_insert_index: None,
		})
		.await;

	// First undo: removes the paste, back to the post-rectangle network. Cursor and legacy must agree.
	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(
		editor.active_document().network_interface.document_network(),
		&after_rect,
		"first undo should restore the post-rectangle network (paste removed)"
	);
	assert_cursor_matches_runtime(editor.active_document(), "after first undo");

	// Re-stage after the undo (the live editor re-evaluates the graph). The pasted image's resource is still
	// cached for redo though its node is gone, so a naive snapshot would re-detect it as a new `AddResource`
	// and retire a phantom interaction, knocking the cursor out of lockstep. Scoping the snapshot to
	// network-referenced resources must keep this a no-op.
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);

	// Second undo: removes the rectangle, back to the loaded base. The cursor must step a second interaction
	// and land on the base, not one short.
	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(
		editor.active_document().network_interface.document_network(),
		&base,
		"second undo should restore the loaded base network"
	);
	assert_cursor_matches_runtime(editor.active_document(), "after second undo");

	// Back at the loaded base, neither path can undo further: the load is not an undo step. Had the `Gdd`
	// retired the mount base as its own interaction, the cursor could still undo here, the off-by-one.
	let storage = editor.active_document().storage().expect("storage mounted");
	assert!(!storage.can_undo(), "cursor must not undo past the loaded base (the load is not an undo step)");
}

/// Assert the `Gdd` cursor's stored registry matches a fresh `from_runtime` of the current interface (node
/// set, network set, per-network export wiring), mirroring the live `verify_cursor_matches_runtime` check.
/// The runtime's resource set may be a superset (it keeps undone resources alive for redo), but every
/// resource the current network references must be present.
fn assert_cursor_matches_runtime(document: &DocumentMessageHandler, at: &str) {
	let storage = document.storage().expect("storage mounted");
	let peer = storage.session().peer();

	let network = document.network_interface.document_network().clone();
	let view = StorageMetadataView::new(&document.network_interface);
	let target = document_graph_storage::Registry::convert_from_runtime(&network, &view, &document.resources.registry, peer).expect("from_runtime");

	let stored = storage.registry();

	let stored_nodes: std::collections::BTreeSet<_> = stored.node_instances.keys().copied().collect();
	let target_nodes: std::collections::BTreeSet<_> = target.registry.node_instances.keys().copied().collect();
	assert_eq!(stored_nodes, target_nodes, "cursor node set diverged from the restored interface {at}");

	let stored_networks: std::collections::BTreeSet<_> = stored.networks.keys().copied().collect();
	let target_networks: std::collections::BTreeSet<_> = target.registry.networks.keys().copied().collect();
	assert_eq!(stored_networks, target_networks, "cursor network set diverged from the restored interface {at}");

	// Compare export wiring per network: the paste rewires the document export, and undo must revert it.
	for (id, stored_network) in &stored.networks {
		let target_network = target.registry.networks.get(id).expect("network present in both (checked above)");
		let stored_targets: Vec<_> = stored_network.exports.iter().map(|export| export.target.clone()).collect();
		let target_targets: Vec<_> = target_network.exports.iter().map(|export| export.target.clone()).collect();
		assert_eq!(stored_targets, target_targets, "cursor export wiring diverged for network {id} {at}");
	}
}

/// Mount a fresh in-memory `Gdd` onto the active document so `commit_storage_snapshot` (the real
/// autosave path) runs against it. Returns the byte store the document's resources resolve through.
async fn mount_in_memory_storage(editor: &mut EditorTestUtils) -> HashMapResourceStorage {
	let gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 0x5EED, "test".into(), "test".into())
		.await
		.expect("create_in");
	editor.active_document_mut().set_storage(Some(gdd));
	HashMapResourceStorage::new()
}

/// Open a real demo artwork, mount storage, edit it, and trigger autosave. The autosave runs
/// `verify_storage_round_trip`, which panics in tests on any conversion or round-trip drift, so a clean run
/// is itself the assertion that the whole pipeline holds on real document data. Also checks that committing
/// edits grows the retired-delta history and that undo reverts the modification.
#[tokio::test]
async fn demo_artwork_edit_autosaves_and_round_trips() {
	let mut editor = EditorTestUtils::create();

	// Open a real demo artwork through the normal open path and let it render.
	let content = std::fs::read_to_string("../demo-artwork/changing-seasons.graphite").expect("read demo artwork");
	editor
		.handle_message(PortfolioMessage::OpenFile {
			path: "changing-seasons.graphite".into(),
			content: content.bytes().collect(),
		})
		.await;

	let byte_store = mount_in_memory_storage(&mut editor).await;

	// First autosave: captures the loaded document. `verify_storage_round_trip` panics on drift.
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);
	let history_after_open = editor.active_document().storage().unwrap().session().history().count();

	// Snapshot the network, then make a real modification.
	let before_edit = editor.active_document().network_interface.document_network().clone();
	editor.draw_rect(64., 64., 192., 192.).await;
	let after_edit = editor.active_document().network_interface.document_network().clone();
	assert_ne!(before_edit, after_edit, "drawing a rectangle should change the document network");

	// Second autosave: again verifies the round-trip, and the edit must produce new retired history.
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);
	let history_after_edit = editor.active_document().storage().unwrap().session().history().count();
	assert!(
		history_after_edit > history_after_open,
		"committing an edit should append retired deltas: {history_after_open} -> {history_after_edit}"
	);

	// The editor's undo stack reverts the modification on a real document.
	editor.handle_message(DocumentMessage::Undo).await;
	let after_undo = editor.active_document().network_interface.document_network().clone();
	assert_eq!(after_undo, before_edit, "undo should restore the pre-edit network");

	// Autosaving the undone state still round-trips cleanly (no drift panic).
	editor.active_document_mut().commit_storage_snapshot(&byte_store, true);
}

/// The document's single Fill node, as `(network_path, node_id)`.
fn find_fill_node(document: &DocumentMessageHandler) -> (Vec<graph_craft::document::NodeId>, graph_craft::document::NodeId) {
	node_paths(&document.network_interface)
		.into_iter()
		.find(|(network_path, node_id)| {
			let Some(network) = document.network_interface.nested_network(network_path) else { return false };
			network.nodes[node_id].implementation == graph_craft::document::DocumentNodeImplementation::ProtoNode(graphene_std::vector_nodes::fill::IDENTIFIER)
		})
		.expect("the document should contain a Fill node")
}

/// The stored paint value of the document's single Fill node.
fn fill_paint_value(document: &DocumentMessageHandler) -> graph_craft::document::value::TaggedValue {
	use graphene_std::NodeInputDecleration as _;

	let (network_path, node_id) = find_fill_node(document);
	let network = document.network_interface.nested_network(&network_path).expect("the found network path should resolve");
	let input = network.nodes[&node_id]
		.inputs
		.get(graphene_std::vector::fill::FillInput::<graphene_std::list::List<graphene_std::Graphic>>::INDEX)
		.expect("Fill should have a paint input");
	input.as_value().expect("the paint input should hold a value").clone()
}

#[tokio::test]
async fn none_fill_survives_document_reopen() {
	use graphene_std::NodeInputDecleration as _;

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

	// Pick the red-slash "none" paint, stored the same way as the Fill widget's None choice
	let (_, fill_node_id) = find_fill_node(editor.active_document());
	editor
		.handle_message(NodeGraphMessage::SetInputValue {
			node_id: fill_node_id,
			input_index: graphene_std::vector::fill::FillInput::<graphene_std::list::List<graphene_std::Graphic>>::INDEX,
			value: graph_craft::document::value::TaggedValue::no_paint(),
		})
		.await;
	assert!(fill_paint_value(editor.active_document()).is_no_paint(), "the None pick should store as no_paint");

	// Reopen through the editor's real open path, which runs the document migrations
	let serialized = editor.active_document().serialize_document();
	editor
		.handle_message(PortfolioMessage::OpenDocumentFile {
			document_name: None,
			document_path: None,
			document_serialized_content: serialized,
		})
		.await;

	let reopened_paint = fill_paint_value(editor.active_document());
	assert!(reopened_paint.is_no_paint(), "a none fill should survive reopening, but the stored paint became {reopened_paint:?}");
}

#[tokio::test]
async fn legacy_four_input_fill_migrates_to_the_split_transform_shape() {
	use graph_craft::document::value::TaggedValue;
	use graphene_std::NodeInputDecleration as _;

	// A minimal master-era document: a 4-input Fill (content, fill: wired, backup color, backup gradient) fed by another node
	const LEGACY_DOCUMENT: &str = r#"{"network_interface":{"network":{"exports":[{"Node":{"node_id":1,"output_index":0,"lambda":false}}],"nodes":[[1,{"inputs":[{"Value":{"tagged_value":{"GraphicGroup":{"instance":[],"transform":[],"alpha_blending":[],"source_node_id":[]}},"exposed":true}},{"Node":{"node_id":2,"output_index":0,"lambda":false}},{"Value":{"tagged_value":{"OptionalColor":null},"exposed":false}},{"Value":{"tagged_value":{"Gradient":{"stops":[[0.0,{"red":0.0,"green":0.0,"blue":0.0,"alpha":1.0}],[1.0,{"red":1.0,"green":1.0,"blue":1.0,"alpha":1.0}]],"gradient_type":"Linear","start":[0.0,0.5],"end":[1.0,0.5],"transform":[1.0,0.0,0.0,1.0,0.0,0.0]}},"exposed":false}}],"manual_composition":{"Concrete":{"name":"core::option::Option<alloc::sync::Arc<graphene_core::context::OwnedContextImpl>>","alias":null}},"implementation":{"ProtoNode":{"name":"graphene_core::vector::FillNode"}},"visible":true,"skip_deduplication":false}],[2,{"inputs":[{"Value":{"tagged_value":"None","exposed":false}},{"Value":{"tagged_value":{"GradientStops":[[0.0,{"red":0.0,"green":0.0,"blue":0.0,"alpha":1.0}],[1.0,{"red":1.0,"green":1.0,"blue":1.0,"alpha":1.0}]]},"exposed":false}},{"Value":{"tagged_value":{"F64":0.5},"exposed":false}}],"manual_composition":{"Concrete":{"name":"core::option::Option<alloc::sync::Arc<graphene_core::context::OwnedContextImpl>>","alias":null}},"implementation":{"ProtoNode":{"name":"graphene_core::ops::SampleGradientNode"}},"visible":true,"skip_deduplication":false}]],"scope_injections":[]},"network_metadata":{"persistent_metadata":{"node_metadata":[[1,{"persistent_metadata":{"reference":"Fill","display_name":"","input_properties":[{"input_data":{"input_name":"Vector Data"},"widget_override":null},{"input_data":{"input_name":"Fill"},"widget_override":null},{"input_data":{"input_name":"Backup Color"},"widget_override":null},{"input_data":{"input_name":"Backup Gradient"},"widget_override":null}],"output_names":["Future<Instances<VectorData>>"],"has_primary_output":true,"locked":false,"pinned":false,"node_type_metadata":{"Node":{"position":{"Absolute":[0,0]}}},"network_metadata":null}}],[2,{"persistent_metadata":{"reference":"Sample Gradient","display_name":"","input_properties":[{"input_data":{"input_name":"Primary"},"widget_override":null},{"input_data":{"input_name":"Gradient"},"widget_override":null},{"input_data":{"input_name":"Position"},"widget_override":null}],"output_names":["Future<Color>"],"has_primary_output":true,"locked":false,"pinned":false,"node_type_metadata":{"Node":{"position":{"Absolute":[-20,0]}}},"network_metadata":null}}]],"previewing":"No","navigation_metadata":{"node_graph_ptz":{"pan":[0.0,0.0],"tilt":0.0,"zoom":1.0,"flip":false},"node_graph_to_viewport":[1.0,0.0,0.0,1.0,0.0,0.0],"node_graph_top_right":[0.0,0.0]},"selection_undo_history":[],"selection_redo_history":[]}}},"collapsed":[],"name":"legacy_fill.graphite","commit_hash":"0000000000000000000000000000000000000000","document_ptz":{"pan":[0.0,0.0],"tilt":0.0,"zoom":1.0,"flip":false},"document_mode":"DesignMode","view_mode":"Normal","overlays_visibility_settings":{"all":true,"artboard_name":true,"compass_rose":true,"quick_measurement":true,"transform_measurement":true,"transform_cage":true,"hover_outline":true,"selection_outline":true,"pivot":true,"path":true,"anchors":true,"handles":true},"rulers_visible":true,"snapping_state":{"snapping_enabled":true,"grid_snapping":false,"artboards":true,"tolerance":8.0,"bounding_box":{"center_point":true,"corner_point":true,"edge_midpoint":true,"align_with_edges":true,"distribute_evenly":true},"path":{"anchor_point":true,"line_midpoint":true,"along_path":true,"normal_to_path":true,"tangent_to_path":true,"path_intersection_point":true,"align_with_anchor_point":true,"perpendicular_from_endpoint":true},"grid":{"origin":[0.0,0.0],"grid_type":{"Rectangular":{"spacing":[1.0,1.0]}},"grid_color":{"red":0.6,"green":0.6,"blue":0.6,"alpha":1.0},"dot_display":false}},"graph_view_overlay_open":false,"graph_fade_artwork_percentage":80.0}"#;

	// Deserializing alone must succeed, so a failure below is attributable to the migrations
	DocumentMessageHandler::deserialize_document(LEGACY_DOCUMENT).expect("the legacy document should deserialize");

	let mut editor = EditorTestUtils::create();
	editor
		.handle_message(PortfolioMessage::OpenDocumentFile {
			document_name: None,
			document_path: None,
			document_serialized_content: LEGACY_DOCUMENT.to_string(),
		})
		.await;

	let document = editor.active_document();
	let (network_path, node_id) = find_fill_node(document);
	let network = document.network_interface.nested_network(&network_path).expect("the found network path should resolve");
	let inputs = &network.nodes[&node_id].inputs;

	assert_eq!(inputs.len(), 8, "the legacy Fill should upgrade to the 8-input shape");
	let paint = &inputs[graphene_std::vector::fill::FillInput::<graphene_std::list::List<graphene_std::Graphic>>::INDEX];
	assert!(
		matches!(paint, graph_craft::document::NodeInput::Node { .. }),
		"the wired legacy fill should keep its connection, but became {paint:?}"
	);
	let has_transform = inputs[graphene_std::vector::fill::HasTransformInput::INDEX].as_value();
	assert!(
		matches!(has_transform, Some(TaggedValue::Bool(_))),
		"the has-transform input should hold a bool, but became {has_transform:?}"
	);
	let transform = inputs[graphene_std::vector::fill::TransformInput::INDEX].as_value();
	assert!(
		matches!(transform, Some(TaggedValue::DAffine2(_))),
		"the transform input should hold a matrix, but became {transform:?}"
	);
}
