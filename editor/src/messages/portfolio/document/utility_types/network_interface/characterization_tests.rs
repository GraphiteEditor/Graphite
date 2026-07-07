//! Characterization tests capturing the current behavior of `NodeNetworkInterface` as a safety net for its refactor.
//! Each test also sweeps `validate_invariants` so any desync between the network and its metadata tree fails loudly.

use super::{InputConnector, Previewing, TransactionStatus};
use crate::test_utils::test_prelude::*;
use graph_craft::document::NodeInput;
use graphene_std::uuid::NodeId;

fn assert_invariants(editor: &EditorTestUtils, context: &str) {
	let violations = editor.active_document().network_interface.validate_invariants();
	assert!(violations.is_empty(), "Invariant violations {context}:\n{}", violations.join("\n"));
}

fn rectangle_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
}

fn new_artboard_message(id: NodeId) -> GraphOperationMessage {
	GraphOperationMessage::NewArtboard {
		id,
		location: DVec2::ZERO,
		dimensions: DVec2::new(400., 300.),
		background: Color::WHITE,
		clip: false,
	}
}

#[tokio::test]
async fn invariants_hold_through_basic_editing_flow() {
	let mut editor = EditorTestUtils::create();

	// This test covers the interface's structural invariants; the storage dual-write soak is out of its scope and asserted by the storage round-trip tests
	editor.editor.handle_message(PreferencesMessage::ValidateStorageRoundTrip { enabled: false });

	editor.new_document().await;
	assert_invariants(&editor, "after opening a new document");

	editor.handle_message(new_artboard_message(NodeId::new())).await;
	assert_invariants(&editor, "after creating an artboard");

	let rectangle = editor.create_node_by_name(rectangle_definition()).await;
	assert_invariants(&editor, "after creating a node");

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![rectangle] }).await;
	editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
	assert_invariants(&editor, "after deleting the node");

	editor.handle_message(DocumentMessage::Undo).await;
	assert_invariants(&editor, "after undo");

	editor.handle_message(DocumentMessage::Redo).await;
	assert_invariants(&editor, "after redo");
}

#[tokio::test]
async fn deleting_a_node_with_children_prunes_them_from_the_selection() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let parent = editor.create_node_by_name(rectangle_definition()).await;
	let child = editor.create_node_by_name(rectangle_definition()).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	assert!(network_interface.number_of_inputs(&parent, &[]) >= 2, "Test needs a secondary input to wire the child into");

	// Wire the child into the parent's secondary input so it is a sole dependent, then select both and delete only the parent
	network_interface.set_input(&InputConnector::node(parent, 1), NodeInput::node(child, 0), &[]);
	network_interface.selected_nodes_mut(&[]).unwrap().set_selected_nodes(vec![parent, child]);
	network_interface.delete_nodes(vec![parent], true, &[]);

	assert!(network_interface.document_network().nodes.is_empty(), "Both the parent and its sole-dependent child should be deleted");
	let remaining_selection = network_interface.selected_nodes_mut(&[]).unwrap().selected_nodes().copied().collect::<Vec<_>>();
	assert!(remaining_selection.is_empty(), "Deleted children should be pruned from the selection, found {remaining_selection:?}");

	assert_invariants(&editor, "after deleting a node with children");
}

#[tokio::test]
async fn cyclic_connection_is_rejected_without_side_effects() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let a = editor.create_node_by_name(rectangle_definition()).await;
	let b = editor.create_node_by_name(rectangle_definition()).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_input(&InputConnector::node(a, 1), NodeInput::node(b, 0), &[]);

	// Attempt to complete a cycle inside a transaction: the edit must be rejected without marking the transaction as modified
	network_interface.start_transaction();
	let input_before = network_interface.input_from_connector(&InputConnector::node(b, 1), &[]).cloned();
	network_interface.set_input(&InputConnector::node(b, 1), NodeInput::node(a, 0), &[]);

	let input_after = network_interface.input_from_connector(&InputConnector::node(b, 1), &[]).cloned();
	assert_eq!(input_before, input_after, "A rejected cyclic connection should leave the input unchanged");
	assert_eq!(
		network_interface.transaction_status(),
		TransactionStatus::Started,
		"A rejected cyclic connection should not mark the transaction as modified"
	);
	network_interface.finish_transaction();

	assert_invariants(&editor, "after rejecting a cyclic connection");
}

#[tokio::test]
async fn toggling_preview_on_a_disconnected_export() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let node = editor.create_node_by_name(rectangle_definition()).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.disconnect_input(&InputConnector::Export(0), &[]);

	// Previewing a node while the export is disconnected is a preview with nothing to restore
	network_interface.toggle_preview(node, &[]);
	assert_eq!(network_interface.previewing(&[]), Previewing::Yes { root_node_to_restore: None });
	let export = network_interface.input_from_connector(&InputConnector::Export(0), &[]);
	assert_eq!(export.and_then(|input| input.as_node()), Some(node), "The previewed node should be wired to the export");

	// Ending the preview restores the disconnected export
	network_interface.toggle_preview(node, &[]);
	assert_eq!(network_interface.previewing(&[]), Previewing::No);
	let export = network_interface.input_from_connector(&InputConnector::Export(0), &[]);
	assert!(export.is_some_and(|input| input.as_node().is_none()), "Ending the preview should disconnect the export again");

	assert_invariants(&editor, "after toggling preview twice");
}

#[tokio::test]
async fn artboard_identity_is_independent_of_scene_connectivity() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	assert!(network_interface.is_artboard(&artboard, &[]));
	assert!(!network_interface.all_artboards().is_empty());

	// Disconnecting the artboard from the export keeps its identity but removes it from the scene's artboards
	network_interface.disconnect_input(&InputConnector::Export(0), &[]);
	assert!(network_interface.is_artboard(&artboard, &[]), "Artboard identity should survive disconnection");
	assert!(network_interface.all_artboards().is_empty(), "Disconnected artboards should not count as scene artboards");

	assert_invariants(&editor, "after disconnecting an artboard");
}

#[tokio::test]
async fn selection_history_is_not_serialized() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let node = editor.create_node_by_name(rectangle_definition()).await;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;

	let serialized = editor.active_document().serialize_document();
	assert!(!serialized.contains("selection_undo_history"), "Selection history should not be persisted into saved documents");
	assert!(!serialized.contains("selection_redo_history"), "Selection history should not be persisted into saved documents");
}
