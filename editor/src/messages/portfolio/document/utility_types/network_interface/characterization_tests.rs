//! Characterization tests capturing the current behavior of `NodeNetworkInterface` as a safety net for its refactor.
//! Each test also sweeps `validate_invariants` so any desync between the network and its metadata tree fails loudly.

use super::{InputConnector, OutputConnector, Previewing, RootNode, TransactionStatus};
use crate::test_utils::test_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
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
async fn deleting_a_node_keeps_children_shared_with_other_nodes() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let parent = editor.create_node_by_name(rectangle_definition()).await;
	let sibling = editor.create_node_by_name(rectangle_definition()).await;
	let shared_child = editor.create_node_by_name(rectangle_definition()).await;

	let network_interface = &mut editor.active_document_mut().network_interface;

	// Wire the same child into the secondary inputs of both nodes, then delete only the parent along with its children
	network_interface.set_input(&InputConnector::node(parent, 1), NodeInput::node(shared_child, 0), &[]);
	network_interface.set_input(&InputConnector::node(sibling, 1), NodeInput::node(shared_child, 0), &[]);
	network_interface.delete_nodes(vec![parent], true, &[]);

	let nodes = &network_interface.document_network().nodes;
	assert!(!nodes.contains_key(&parent), "The deleted node itself should be gone");
	assert!(nodes.contains_key(&shared_child), "A child shared with another node is not a sole dependent and should survive");
	assert!(nodes.contains_key(&sibling), "The unrelated sibling should survive");

	assert_invariants(&editor, "after deleting a node with a shared child");
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

fn merge_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::Network("Merge".to_string())
}

async fn create_node_at(editor: &mut EditorTestUtils, node_type: DefinitionIdentifier, x: i32, y: i32) -> NodeId {
	let node_id = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(node_id),
			node_type,
			xy: Some((x, y)),
			add_transaction: true,
		})
		.await;
	node_id
}

#[tokio::test]
async fn layer_stacking_follows_wiring() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let upper = create_node_at(&mut editor, merge_definition(), 20, 10).await;
	let lower = create_node_at(&mut editor, merge_definition(), 20, 16).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&upper, &[], true);
	network_interface.set_to_node_or_layer(&lower, &[], true);
	assert!(network_interface.is_layer(&upper, &[]) && network_interface.is_layer(&lower, &[]));
	assert!(network_interface.is_absolute(&lower, &[]));

	// Wiring a layer into the bottom input of another layer converts it to stack positioning at its current visual spot
	let lower_position_before = network_interface.position(&lower, &[]).expect("Lower layer should have a position");
	network_interface.create_wire(&OutputConnector::node(lower, 0), &InputConnector::node(upper, 0), &[]);
	assert!(network_interface.is_stack(&lower, &[]), "A layer feeding the bottom of a layer should be stack positioned");
	let stacked_position = network_interface.position(&lower, &[]).expect("Stacked layer should have a position");
	assert_eq!(stacked_position.y, lower_position_before.y, "Stacking should preserve the layer's vertical position");

	// Disconnecting converts the layer back to absolute positioning without moving it
	network_interface.disconnect_input(&InputConnector::node(upper, 0), &[]);
	assert!(network_interface.is_absolute(&lower, &[]), "A disconnected stack layer should return to absolute positioning");
	assert_eq!(network_interface.position(&lower, &[]), Some(stacked_position), "Unstacking should not move the layer");

	assert_invariants(&editor, "after stacking and unstacking a layer");
}

#[tokio::test]
async fn chain_membership_follows_wiring() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let layer = create_node_at(&mut editor, merge_definition(), 20, 10).await;
	let node = create_node_at(&mut editor, rectangle_definition(), 15, 10).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&layer, &[], true);

	// A node wired into a layer's secondary input from the same row, within chain distance, joins the chain
	network_interface.create_wire(&OutputConnector::node(node, 0), &InputConnector::node(layer, 1), &[]);
	assert!(
		network_interface.is_chain(&node, &[]),
		"A node feeding a layer's secondary input from chain range should become a chain node"
	);

	// Disconnecting breaks the chain and the node becomes absolute at its chain spot
	let chained_y = network_interface.position(&node, &[]).expect("Chained node should have a position").y;
	network_interface.disconnect_input(&InputConnector::node(layer, 1), &[]);
	assert!(!network_interface.is_chain(&node, &[]), "Disconnecting should break the chain");
	assert!(network_interface.is_absolute(&node, &[]));
	assert_eq!(network_interface.position(&node, &[]).map(|position| position.y), Some(chained_y));

	assert_invariants(&editor, "after forming and breaking a chain");
}

#[tokio::test]
async fn move_layer_to_stack_builds_the_layer_stack() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;

	let first = create_node_at(&mut editor, merge_definition(), 0, 30).await;
	let second = create_node_at(&mut editor, merge_definition(), 0, 40).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&first, &[], true);
	network_interface.set_to_node_or_layer(&second, &[], true);

	let artboard_layer = LayerNodeIdentifier::new(artboard, network_interface);
	let first_layer = LayerNodeIdentifier::new(first, network_interface);
	let second_layer = LayerNodeIdentifier::new(second, network_interface);

	network_interface.move_layer_to_stack(first_layer, artboard_layer, 0, &[]);
	let children = artboard_layer.children(network_interface.document_metadata()).collect::<Vec<_>>();
	assert_eq!(children, vec![first_layer], "The first moved layer should become the artboard's only child");

	network_interface.move_layer_to_stack(second_layer, artboard_layer, 1, &[]);
	let children = artboard_layer.children(network_interface.document_metadata()).collect::<Vec<_>>();
	assert_eq!(children, vec![first_layer, second_layer], "The second layer should be inserted below the first");
	assert!(network_interface.is_stack(&second, &[]), "A layer below a sibling should be stack positioned");

	assert_invariants(&editor, "after moving two layers into an artboard");
}

#[tokio::test]
async fn signature_edits_keep_parallel_metadata_in_sync() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = create_node_at(&mut editor, merge_definition(), 0, 0).await;
	let path = vec![merge];

	let network_interface = &mut editor.active_document_mut().network_interface;
	let initial_imports = network_interface.number_of_imports(&path);
	let initial_exports = network_interface.number_of_exports(&path);

	network_interface.add_import(TaggedValue::None, true, -1, "Extra import", "", &path);
	assert_eq!(network_interface.number_of_imports(&path), initial_imports + 1);
	assert_invariants(&editor, "after adding an import");

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.add_export(TaggedValue::None, -1, "Extra export", &path);
	assert_eq!(network_interface.number_of_exports(&path), initial_exports + 1);
	assert_invariants(&editor, "after adding an export");

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.reorder_import(initial_imports, 0, &path);
	assert_invariants(&editor, "after reordering an import to the front");

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.remove_import(0, &path);
	assert_eq!(network_interface.number_of_imports(&path), initial_imports);
	assert_invariants(&editor, "after removing the reordered import");

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.remove_export(initial_exports, &path);
	assert_eq!(network_interface.number_of_exports(&path), initial_exports);
	assert_invariants(&editor, "after removing the added export");
}

#[tokio::test]
async fn toggle_preview_transitions_with_a_connected_export() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = create_node_at(&mut editor, rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	let export_node = |network_interface: &super::NodeNetworkInterface| network_interface.input_from_connector(&InputConnector::Export(0), &[]).and_then(|input| input.as_node());
	assert_eq!(export_node(network_interface), Some(artboard));

	// Previewing a node remembers the artboard as the connection to restore
	network_interface.toggle_preview(node, &[]);
	assert_eq!(export_node(network_interface), Some(node));
	assert_eq!(
		network_interface.previewing(&[]),
		Previewing::Yes {
			root_node_to_restore: Some(RootNode { node_id: artboard, output_index: 0 })
		}
	);

	// Toggling the previewed node again restores the artboard connection
	network_interface.toggle_preview(node, &[]);
	assert_eq!(export_node(network_interface), Some(artboard));
	assert_eq!(network_interface.previewing(&[]), Previewing::No);

	// Toggling the restore node while previewing promotes it to the export with nothing left to restore
	network_interface.toggle_preview(node, &[]);
	network_interface.toggle_preview(artboard, &[]);
	assert_eq!(export_node(network_interface), Some(artboard));
	assert_eq!(network_interface.previewing(&[]), Previewing::Yes { root_node_to_restore: None });

	// Toggling it once more ends the preview by disconnecting the export entirely
	network_interface.toggle_preview(artboard, &[]);
	assert_eq!(export_node(network_interface), None);
	assert_eq!(network_interface.previewing(&[]), Previewing::No);

	assert_invariants(&editor, "after cycling through the preview states");
}
