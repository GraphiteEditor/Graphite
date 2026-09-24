use super::{InputConnector, OutputConnector, Previewing, RootNode, TransactionStatus};
use crate::messages::portfolio::document::node_graph::utility_types::Direction;
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

	// This test covers the interface's structural invariants. The storage dual-write is out of its scope and asserted by the storage round-trip tests.
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
	network_interface.set_input(&InputConnector::node_at_index(parent, 1), NodeInput::node(child, 0), &[]);
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
	network_interface.set_input(&InputConnector::node_at_index(parent, 1), NodeInput::node(shared_child, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(sibling, 1), NodeInput::node(shared_child, 0), &[]);
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
	network_interface.set_input(&InputConnector::node_at_index(a, 1), NodeInput::node(b, 0), &[]);

	// Attempt to complete a cycle inside a transaction: the edit must be rejected without marking the transaction as modified
	network_interface.start_transaction();
	let input_before = network_interface.input_from_connector(&InputConnector::node_at_index(b, 1), &[]).cloned();
	network_interface.set_input(&InputConnector::node_at_index(b, 1), NodeInput::node(a, 0), &[]);

	let input_after = network_interface.input_from_connector(&InputConnector::node_at_index(b, 1), &[]).cloned();
	assert_eq!(input_before, input_after, "A rejected cyclic connection should leave the input unchanged");
	assert_eq!(
		network_interface.transaction_status(),
		TransactionStatus::Started,
		"A rejected cyclic connection should not mark the transaction as modified"
	);
	network_interface.finish_transaction();

	assert_invariants(&editor, "after rejecting a cyclic connection");
}

/// The check answers for the network with the proposed input substituted, so it has to see a loop that
/// closes through intermediate nodes rather than only one that connects two neighbors.
#[tokio::test]
async fn a_cycle_closing_through_a_chain_is_rejected() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let a = editor.create_node_by_name(rectangle_definition()).await;
	let b = editor.create_node_by_name(rectangle_definition()).await;
	let c = editor.create_node_by_name(rectangle_definition()).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_input(&InputConnector::node_at_index(a, 1), NodeInput::node(b, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(b, 1), NodeInput::node(c, 0), &[]);

	// `a` already reaches `c` through `b`, so feeding `a` back into `c` closes a three-node loop
	let input_before = network_interface.input_from_connector(&InputConnector::node_at_index(c, 1), &[]).cloned();
	network_interface.set_input(&InputConnector::node_at_index(c, 1), NodeInput::node(a, 0), &[]);

	let input_after = network_interface.input_from_connector(&InputConnector::node_at_index(c, 1), &[]).cloned();
	assert_eq!(input_before, input_after, "A cycle closing through an intermediate node should be rejected");

	assert_invariants(&editor, "after rejecting a chained cyclic connection");
}

/// An export is not something a node takes as input, so connecting one cannot close a loop and must
/// not be refused.
#[tokio::test]
async fn connecting_an_export_is_never_cyclic() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let node = editor.create_node_by_name(rectangle_definition()).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_input(&InputConnector::Export(0), NodeInput::node(node, 0), &[]);

	assert_eq!(
		network_interface.input_from_connector(&InputConnector::Export(0), &[]).cloned(),
		Some(NodeInput::node(node, 0)),
		"Connecting a node to the export should be accepted"
	);

	assert_invariants(&editor, "after connecting the export");
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

#[tokio::test]
async fn layer_stacking_follows_wiring() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let upper = editor.create_node_by_name_at(merge_definition(), 20, 10).await;
	let lower = editor.create_node_by_name_at(merge_definition(), 20, 16).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&upper, &[], true);
	network_interface.set_to_node_or_layer(&lower, &[], true);
	assert!(network_interface.is_layer(&upper, &[]) && network_interface.is_layer(&lower, &[]));
	assert!(network_interface.is_absolute(&lower, &[]));

	// Wiring a layer into the bottom input of another layer converts it to stack positioning at its current visual spot
	let lower_position_before = network_interface.position(&lower, &[]).expect("Lower layer should have a position");
	network_interface.create_wire(&OutputConnector::primary_output(lower), &InputConnector::primary_input(upper), &[]);
	assert!(network_interface.is_stack(&lower, &[]), "A layer feeding the bottom of a layer should be stack positioned");
	let stacked_position = network_interface.position(&lower, &[]).expect("Stacked layer should have a position");
	assert_eq!(stacked_position.y, lower_position_before.y, "Stacking should preserve the layer's vertical position");

	// Disconnecting converts the layer back to absolute positioning without moving it
	network_interface.disconnect_input(&InputConnector::primary_input(upper), &[]);
	assert!(network_interface.is_absolute(&lower, &[]), "A disconnected stack layer should return to absolute positioning");
	assert_eq!(network_interface.position(&lower, &[]), Some(stacked_position), "Unstacking should not move the layer");

	assert_invariants(&editor, "after stacking and unstacking a layer");
}

#[tokio::test]
async fn chain_membership_follows_wiring() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let layer = editor.create_node_by_name_at(merge_definition(), 20, 10).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 15, 10).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&layer, &[], true);

	// A node wired into a layer's secondary input from the same row, within chain distance, joins the chain
	network_interface.create_wire(&OutputConnector::primary_output(node), &InputConnector::layer_secondary_input(layer), &[]);
	assert!(
		network_interface.is_chain(&node, &[]),
		"A node feeding a layer's secondary input from chain range should become a chain node"
	);

	// Disconnecting breaks the chain and the node becomes absolute at its chain spot
	let chained_y = network_interface.position(&node, &[]).expect("Chained node should have a position").y;
	network_interface.disconnect_input(&InputConnector::layer_secondary_input(layer), &[]);
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

	let first = editor.create_node_by_name_at(merge_definition(), 0, 30).await;
	let second = editor.create_node_by_name_at(merge_definition(), 0, 40).await;

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
async fn drag_offsets_do_not_outlive_programmatic_shifts() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;

	let first = editor.create_node_by_name_at(merge_definition(), 0, 30).await;
	let second = editor.create_node_by_name_at(merge_definition(), 0, 40).await;
	let third = editor.create_node_by_name_at(merge_definition(), 0, 50).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&first, &[], true);
	network_interface.set_to_node_or_layer(&second, &[], true);
	network_interface.set_to_node_or_layer(&third, &[], true);

	let artboard_layer = LayerNodeIdentifier::new(artboard, network_interface);
	network_interface.move_layer_to_stack(LayerNodeIdentifier::new(first, network_interface), artboard_layer, 0, &[]);
	network_interface.move_layer_to_stack(LayerNodeIdentifier::new(second, network_interface), artboard_layer, 1, &[]);
	network_interface.move_layer_to_stack(LayerNodeIdentifier::new(third, network_interface), artboard_layer, 2, &[]);

	// Making room while inserting into the stack pushes neighbors with drag-offset bookkeeping that must not outlive the operation
	for node_id in [first, second, third] {
		assert_eq!(network_interface.drag_offset(&node_id, &[]), 0, "Inserting into a stack should not leave a drag offset on {node_id}");
	}

	// Deleting the middle layer collapses the stack upward, which must also leave no offsets behind
	network_interface.delete_nodes(vec![second], false, &[]);
	assert_eq!(network_interface.drag_offset(&third, &[]), 0, "Collapsing the deleted layer's space should not leave a drag offset");

	// A later nudge must move the survivor by exactly one unit, without replaying any restore toward its pre-collapse position
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![third] }).await;
	let network_interface = &editor.active_document().network_interface;
	let before = network_interface.position(&third, &[]).expect("Surviving layer should have a position");
	editor
		.handle_message(NodeGraphMessage::ShiftSelectedNodes {
			direction: Direction::Down,
			rubber_band: false,
		})
		.await;
	let network_interface = &editor.active_document().network_interface;
	let after = network_interface.position(&third, &[]).expect("Surviving layer should have a position");
	assert_eq!(after.y - before.y, 1, "A single nudge should move the layer exactly one unit");

	assert_invariants(&editor, "after stack pushes, a delete collapse, and a nudge");
}

#[tokio::test]
async fn signature_edits_keep_parallel_metadata_in_sync() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
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

/// Previewing rewires nothing, so ending one has nothing to restore and cannot disconnect anything.
#[tokio::test]
async fn previewing_leaves_the_export_alone() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	let export_node = |network_interface: &super::NodeNetworkInterface| network_interface.input_from_connector(&InputConnector::Export(0), &[]).and_then(|input| input.as_node());
	assert_eq!(export_node(network_interface), Some(artboard));

	// Previewing a node names it without touching the export
	network_interface.toggle_preview(node, &[]);
	assert_eq!(
		network_interface.previewing(&[]),
		Previewing::Yes {
			previewed: RootNode { node_id: node, output_index: 0 }
		}
	);
	assert_eq!(export_node(network_interface), Some(artboard), "Previewing must not rewire the export");

	// Previewing a different node moves the preview, still without touching the export
	network_interface.toggle_preview(artboard, &[]);
	assert_eq!(
		network_interface.previewing(&[]),
		Previewing::Yes {
			previewed: RootNode { node_id: artboard, output_index: 0 }
		}
	);
	assert_eq!(export_node(network_interface), Some(artboard));

	// Toggling the previewed node again ends the preview, leaving the export as it always was
	network_interface.toggle_preview(artboard, &[]);
	assert_eq!(network_interface.previewing(&[]), Previewing::No);
	assert_eq!(export_node(network_interface), Some(artboard));

	assert_invariants(&editor, "after cycling through the preview states");
}

/// The graph handed to the compiler renders the previewed node, while the document keeps its own export.
#[tokio::test]
async fn the_evaluated_network_renders_the_previewed_node() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.toggle_preview(node, &[]);

	let evaluated = network_interface.network_to_evaluate();
	assert_eq!(
		evaluated.exports.first().and_then(|export| export.as_node()),
		Some(node),
		"The network being evaluated should export the previewed node"
	);
	assert_eq!(
		network_interface.document_network().exports.first().and_then(|export| export.as_node()),
		Some(artboard),
		"The document itself should be unchanged"
	);
}

/// A document saved by the version that rewired the export still opens, rather than failing the whole
/// document's deserialization.
#[test]
fn a_preview_written_by_the_rewiring_version_still_deserializes() {
	let stored = r#"{"Yes":{"root_node_to_restore":{"node_id":7,"output_index":1}}}"#;

	let previewing: Previewing = serde_json::from_str(stored).expect("a preview written by the rewiring version should deserialize");

	assert_eq!(
		previewing,
		Previewing::LegacyRewired {
			root_node_to_restore: Some(RootNode { node_id: NodeId(7), output_index: 1 })
		},
		"The stored preview should be read as one needing migration"
	);
}

/// The shape this version writes round trips, so accepting the older one has not displaced it.
#[test]
fn the_current_preview_shape_round_trips() {
	let previewing = Previewing::Yes {
		previewed: RootNode { node_id: NodeId(7), output_index: 1 },
	};

	let stored = serde_json::to_string(&previewing).expect("previewing should serialize");

	assert_eq!(serde_json::from_str::<Previewing>(&stored).expect("previewing should deserialize"), previewing);
}

/// Migrating a rewired preview puts the export back and keeps the node the user was looking at as the
/// preview, so the document is no longer rewired but looks the same.
#[tokio::test]
async fn migrating_a_rewired_preview_restores_the_export() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;

	// Stand in for what the rewiring version left on disk: the export moved to the previewed node, and
	// the metadata naming the artboard as what to restore.
	network_interface.create_wire(&OutputConnector::primary_output(node), &InputConnector::Export(0), &[]);
	let Some(mut network) = network_interface.network_mut(&[]) else {
		panic!("the document network should resolve")
	};
	network.set_previewing(Previewing::LegacyRewired {
		root_node_to_restore: Some(RootNode { node_id: artboard, output_index: 0 }),
	});

	network_interface.migrate_rewired_previews();

	assert_eq!(
		network_interface.document_network().exports.first().and_then(|export| export.as_node()),
		Some(artboard),
		"The export should be back to what the document recorded to restore"
	);
	assert_eq!(
		network_interface.previewing(&[]),
		Previewing::Yes {
			previewed: RootNode { node_id: node, output_index: 0 }
		},
		"The node the export had been moved to should become the preview"
	);
	assert_eq!(
		network_interface.network_to_evaluate().exports.first().and_then(|export| export.as_node()),
		Some(node),
		"What is evaluated should still be the previewed node, so the user sees what they saved"
	);
}

/// Toggling a preview must move the hash the executor caches against, or the graph it already sent is
/// reused and the canvas keeps rendering the old export.
#[tokio::test]
async fn toggling_a_preview_changes_the_network_hash() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	let before = network_interface.network_hash();

	network_interface.toggle_preview(node, &[]);
	let previewing = network_interface.network_hash();
	assert_ne!(before, previewing, "Starting a preview should change the hash, since it changes what is evaluated");

	network_interface.toggle_preview(node, &[]);
	assert_eq!(before, network_interface.network_hash(), "Ending the preview should return the hash to what it was");
}

/// The hash must be stable for an unchanged document, or every frame looks like a change.
#[tokio::test]
async fn the_network_hash_is_stable_while_previewing() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.toggle_preview(node, &[]);

	let hash = network_interface.network_hash();
	for _ in 0..8 {
		assert_eq!(hash, network_interface.network_hash(), "Re-reading the hash of an unchanged document should give the same value");
	}
}

/// A preview restored from the session must name a node the document still has: the session outlives the
/// document, and previewing a removed node would redirect the export to nothing.
#[tokio::test]
async fn a_session_preview_of_a_removed_node_is_dropped() {
	use super::storage_metadata::{apply_network_view_settings, collect_network_view_settings};

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.toggle_preview(node, &[]);

	let network_ids = HashMap::from([(Vec::new(), document_graph_storage::NetworkId(0))]);
	let stored = collect_network_view_settings(network_interface, &network_ids);

	// The document moves on without the previewed node, as it would if another peer had removed it
	network_interface.delete_nodes(vec![node], false, &[]);
	apply_network_view_settings(network_interface, &network_ids, &stored);

	assert_eq!(
		network_interface.previewing(&[]),
		Previewing::No,
		"A preview naming a node the document no longer has should be dropped rather than restored"
	);
	assert_eq!(
		network_interface.network_to_evaluate().exports.first().and_then(|export| export.as_node()),
		Some(artboard),
		"The evaluated network should fall back to the document's own export"
	);
}
