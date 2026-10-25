use super::{
	ChainInsertionPoint, FlowType, GlideOffsets, ImportOrExport, InputConnector, NodeNetworkInterface, NodeNetworkTemplate, NodeTemplate, NodeTemplateImplementation, NodeTypePersistentMetadata,
	OutputConnector, Previewing, RootNode, TransactionStatus,
};
use crate::consts::GRID_SIZE;
use crate::messages::input_mapper::utility_types::pointer::EditorPointerState;
use crate::messages::portfolio::document::node_graph::utility_types::{ContextMenuData, Direction};
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::test_utils::test_prelude::*;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNodeImplementation, NodeInput};
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
async fn a_dot_on_a_stack_wire_leaves_the_layer_beneath_it_where_it_was() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let upper = editor.create_node_by_name_at(merge_definition(), 20, 10).await;
	let lower = editor.create_node_by_name_at(merge_definition(), 20, 16).await;
	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&upper, &[], true);
	network_interface.set_to_node_or_layer(&lower, &[], true);
	network_interface.create_wire(&OutputConnector::primary_output(lower), &InputConnector::primary_input(upper), &[]);
	assert!(network_interface.is_stack(&lower, &[]), "the lower layer should be stacked: fixture is wrong");
	assert!(network_interface.can_bend_wire_into(&InputConnector::primary_input(upper), &[]), "a stack wire should take a dot");
	let lower_position = network_interface.position(&lower, &[]);

	let dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: dot,
			input_connector: InputConnector::primary_input(upper),
			x: 22,
			y: 15,
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(feeding(&editor, upper), Some(OutputConnector::node(dot, 0)), "the dot should feed the upper layer");
	assert_eq!(feeding(&editor, dot), Some(OutputConnector::node(lower, 0)), "the lower layer should feed the dot");
	assert!(network_interface.is_absolute(&lower, &[]), "a layer feeding a dot rather than a layer is no longer stacked");
	assert_eq!(network_interface.position(&lower, &[]), lower_position, "the lower layer shouldn't move");
	assert_invariants(&editor, "after putting a dot on a stack wire");
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

fn fill_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::fill::IDENTIFIER)
}

/// Builds a layer fed by the chain `near <- far <- source` (a generator fixed at the upstream end), with an outside node also wired into the near node's second input.
async fn layer_with_chain(editor: &mut EditorTestUtils) -> (NodeId, NodeId, NodeId, NodeId, NodeId) {
	let layer = editor.create_node_by_name_at(merge_definition(), 30, 10).await;
	let near = editor.create_node_by_name_at(fill_definition(), 23, 10).await;
	let far = editor.create_node_by_name_at(fill_definition(), 16, 10).await;
	let source = editor.create_node_by_name_at(rectangle_definition(), 9, 10).await;
	let outside = editor.create_node_by_name_at(rectangle_definition(), 16, 20).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&layer, &[], true);
	network_interface.create_wire(&OutputConnector::primary_output(near), &InputConnector::layer_secondary_input(layer), &[]);
	network_interface.create_wire(&OutputConnector::primary_output(far), &InputConnector::primary_input(near), &[]);
	network_interface.create_wire(&OutputConnector::primary_output(source), &InputConnector::primary_input(far), &[]);
	network_interface.create_wire(&OutputConnector::primary_output(outside), &InputConnector::node_at_index(near, 1), &[]);

	(layer, near, far, source, outside)
}

/// The layer from `layer_with_chain` placed in an artboard, making it part of the layer structure that the Layers panel and clipboard act on.
async fn layer_with_chain_in_artboard(editor: &mut EditorTestUtils) -> (NodeId, NodeId, NodeId, NodeId, NodeId) {
	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let nodes = layer_with_chain(editor).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	let artboard_layer = LayerNodeIdentifier::new(artboard, network_interface);
	network_interface.move_layer_to_stack(LayerNodeIdentifier::new(nodes.0, network_interface), artboard_layer, 0, &[]);

	nodes
}

fn chain_of(network_interface: &NodeNetworkInterface, layer: NodeId) -> Vec<NodeId> {
	network_interface.upstream_flow_back_from_nodes(vec![layer], &[], FlowType::HorizontalFlow).skip(1).collect()
}

#[tokio::test]
async fn reordering_chain_nodes_keeps_the_source_at_the_upstream_end() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, _) = layer_with_chain(&mut editor).await;

	// Gap 2 is upstream of both reorderable nodes, beside the fixed source
	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.reorder_chain_nodes(&[near], 2, &[]);
	assert_eq!(chain_of(network_interface, layer), vec![far, near, source]);

	assert_invariants(&editor, "after reordering a chain node");
}

#[tokio::test]
async fn duplicating_chain_nodes_inserts_copies_that_keep_their_other_wires() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, outside) = layer_with_chain(&mut editor).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.duplicate_chain_nodes(&[near], Some((layer, 2)), &[]);

	let chain = chain_of(network_interface, layer);
	assert_eq!(chain.len(), 4, "The copy should join the chain");
	assert_eq!(
		(chain[0], chain[1], chain[3]),
		(near, far, source),
		"The originals should keep their places, with the source still at the end"
	);

	let copy = chain[2];
	assert!(network_interface.is_chain(&copy, &[]), "The copy should be positioned as a chain node");
	assert_eq!(
		network_interface.input_from_connector(&InputConnector::node_at_index(copy, 1), &[]),
		Some(&NodeInput::node(outside, 0)),
		"The copy should keep its original's other wires"
	);

	assert_invariants(&editor, "after duplicating a chain node");
}

#[tokio::test]
async fn duplicating_chain_nodes_in_place_one_at_a_time_interleaves_the_copies() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, _) = layer_with_chain(&mut editor).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	let near_copy = network_interface.duplicate_chain_nodes(&[near], None, &[]);
	let far_copy = network_interface.duplicate_chain_nodes(&[far], None, &[]);
	assert_eq!((near_copy.len(), far_copy.len()), (1, 1));

	// Each copy lands just downstream of its own original
	assert_eq!(chain_of(network_interface, layer), vec![near_copy[0], near, far_copy[0], far, source]);

	assert_invariants(&editor, "after duplicating chain nodes in place");
}

#[tokio::test]
async fn pasting_a_chain_node_at_another_layers_chain_end_keeps_only_wires_that_exist() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (_, near, _, _, outside) = layer_with_chain(&mut editor).await;
	let other_layer = editor.create_node_by_name_at(merge_definition(), 30, 30).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&other_layer, &[], true);
	let copy = network_interface.copy_chain_node(&near, &[]).expect("The chain node should be copyable");

	// Pasted while the outside node exists, the copy stays wired to it
	let pasted = network_interface.paste_into_chain(ChainInsertionPoint::BesideLayer(other_layer), vec![copy.clone()], &[]);
	assert_eq!(chain_of(network_interface, other_layer), pasted, "The copy should form the other layer's chain");
	assert_eq!(
		network_interface.input_from_connector(&InputConnector::node_at_index(pasted[0], 1), &[]),
		Some(&NodeInput::node(outside, 0))
	);

	// Pasted where the outside node is missing, the copy takes the value it had
	network_interface.delete_nodes(vec![outside], false, &[]);
	let pasted_again = network_interface.paste_into_chain(ChainInsertionPoint::BesideLayer(other_layer), vec![copy], &[]);
	assert_eq!(
		chain_of(network_interface, other_layer),
		vec![pasted_again[0], pasted[0]],
		"Pasting at the chain's end should go beside the layer"
	);
	assert!(
		matches!(
			network_interface.input_from_connector(&InputConnector::node_at_index(pasted_again[0], 1), &[]),
			Some(NodeInput::Value { .. })
		),
		"A wire to a missing node should become its fallback value"
	);

	assert_invariants(&editor, "after pasting chain nodes");
}

#[tokio::test]
async fn cutting_a_chain_node_and_pasting_it_back_keeps_its_wires() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, outside) = layer_with_chain_in_artboard(&mut editor).await;

	// Cutting removes just the node, keeping the node that fed its second input
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![near] }).await;
	let clipboard = editor
		.handle_message(ClipboardMessage::CutLayers)
		.await
		.into_iter()
		.find_map(|message| match message {
			FrontendMessage::TriggerClipboardWrite { content } => Some(content),
			_ => None,
		})
		.expect("Cutting a chain node should write it to the clipboard");
	let network_interface = &editor.active_document().network_interface;
	assert_eq!(chain_of(network_interface, layer), vec![far, source], "Cutting should close the chain around the node");
	assert!(network_interface.document_node(&outside, &[]).is_some(), "Cutting should keep what fed the node's other inputs");

	// Pasting with the layer selected puts the node back beside the layer, wired as before
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer] }).await;
	editor
		.handle_message(ClipboardMessage::ReadClipboard {
			content: ClipboardContentRaw::Text(clipboard),
		})
		.await;
	let network_interface = &editor.active_document().network_interface;
	let chain = chain_of(network_interface, layer);
	assert_eq!(chain.len(), 3, "The pasted node should join the chain");
	assert_eq!(chain[1..], [far, source], "The pasted node should go beside the layer");
	assert_eq!(
		network_interface.input_from_connector(&InputConnector::node_at_index(chain[0], 1), &[]),
		Some(&NodeInput::node(outside, 0)),
		"The pasted node should be wired to what fed it before"
	);
	assert_eq!(
		network_interface.selected_nodes().selected_nodes().copied().collect::<Vec<_>>(),
		vec![chain[0]],
		"The pasted node should be selected"
	);

	assert_invariants(&editor, "after cutting and pasting a chain node");
}

#[tokio::test]
async fn pasted_chain_nodes_go_beside_their_originals_or_pair_with_a_new_selection() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, _) = layer_with_chain_in_artboard(&mut editor).await;
	let copy = editor.active_document().network_interface.copy_chain_node(&near, &[]).expect("The chain node should be copyable");

	// Over the same selection that was copied, the copy goes beside its original
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![near] }).await;
	editor.handle_message(DocumentMessage::PasteChainNodes { copies: vec![copy.clone()] }).await;
	let chain = chain_of(&editor.active_document().network_interface, layer);
	let first_copy = chain[0];
	assert_eq!(chain, vec![first_copy, near, far, source], "The copy should go beside the node it was copied from");

	// Over a different selection of the same size, the copy pairs with that node instead
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![far] }).await;
	editor.handle_message(DocumentMessage::PasteChainNodes { copies: vec![copy] }).await;
	let chain = chain_of(&editor.active_document().network_interface, layer);
	let second_copy = chain[2];
	assert_eq!(chain, vec![first_copy, near, second_copy, far, source], "The copy should go beside the newly selected node");
	assert_eq!(
		editor.active_document().network_interface.selected_nodes().selected_nodes().copied().collect::<Vec<_>>(),
		vec![second_copy],
		"The pasted copy should become the selection"
	);

	assert_invariants(&editor, "after pasting chain nodes over a selection");
}

#[tokio::test]
async fn moving_a_chain_node_to_another_layer_closes_the_gap_it_leaves() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, _) = layer_with_chain(&mut editor).await;
	let other_layer = editor.create_node_by_name_at(merge_definition(), 30, 30).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&other_layer, &[], true);
	network_interface.move_chain_nodes(&[near], other_layer, 0, &[]);

	assert_eq!(chain_of(network_interface, layer), vec![far, source], "The source chain should close up around the moved node");
	assert_eq!(chain_of(network_interface, other_layer), vec![near], "The moved node should form the other layer's chain");

	assert_invariants(&editor, "after moving a chain node between layers");
}

#[tokio::test]
async fn a_chain_node_reading_from_a_layer_joins_its_chain_only_as_an_unwired_copy() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, _) = layer_with_chain(&mut editor).await;
	let other_layer = editor.create_node_by_name_at(merge_definition(), 30, 30).await;

	// The other layer feeds the near node's second input, so its chain taking the near node with that wire would close a cycle
	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&other_layer, &[], true);
	network_interface.create_wire(&OutputConnector::primary_output(other_layer), &InputConnector::node_at_index(near, 1), &[]);

	network_interface.move_chain_nodes(&[near], other_layer, 0, &[]);
	assert_eq!(chain_of(network_interface, layer), vec![near, far, source], "A refused move should leave the node in its own chain");
	assert!(chain_of(network_interface, other_layer).is_empty(), "A refused move should leave the other chain as it was");

	let copies = network_interface.duplicate_chain_nodes(&[near], Some((other_layer, 0)), &[]);
	assert_eq!(chain_of(network_interface, other_layer), copies, "The copy should join the other chain");
	assert!(
		matches!(network_interface.input_from_connector(&InputConnector::node_at_index(copies[0], 1), &[]), Some(NodeInput::Value { .. })),
		"The copy's wire from the layer it joined should fall back to a value"
	);

	assert_invariants(&editor, "after chain edits that would close a cycle");
}

#[tokio::test]
async fn duplicating_a_mixed_graph_selection_stacks_the_layers_and_offsets_the_other_nodes() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let (layer, near, _, _, _) = layer_with_chain(&mut editor).await;
	let loose = editor.create_node_by_name_at(rectangle_definition(), 40, 40).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	let artboard_layer = LayerNodeIdentifier::new(artboard, network_interface);
	network_interface.move_layer_to_stack(LayerNodeIdentifier::new(layer, network_interface), artboard_layer, 0, &[]);

	// The selected chain node comes along with its layer's copy, so only the loose node gets an offset copy of its own
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer, near, loose] }).await;
	editor.handle_message(NodeGraphMessage::DuplicateSelectedNodes).await;

	let network_interface = &editor.active_document().network_interface;
	let children = artboard_layer.children(network_interface.document_metadata()).map(|child| child.to_node()).collect::<Vec<_>>();
	assert_eq!(children.len(), 2, "The layer's copy should join the stack beside it");
	let layer_copy = *children.iter().find(|child| **child != layer).unwrap();
	assert_eq!(chain_of(network_interface, layer_copy).len(), 3, "The layer's copy should bring its own chain");

	let selected = network_interface.selected_nodes().selected_nodes().copied().collect::<Vec<_>>();
	assert_eq!(selected.len(), 2, "Only the layer's copy and the loose node's copy should be selected");
	assert!(selected.contains(&layer_copy), "The layer's copy should be selected");
	let loose_copy = *selected.iter().find(|node| **node != layer_copy).unwrap();
	assert_eq!(
		network_interface.position(&loose_copy, &[]),
		network_interface.position(&loose, &[]).map(|position| position + IVec2::new(2, 2))
	);

	assert_invariants(&editor, "after duplicating a mixed selection of layers and nodes");
}

/// A generator feeding three nodes in a row through their primary inputs, outside any layer.
async fn floating_flow(editor: &mut EditorTestUtils) -> (NodeId, NodeId, NodeId, NodeId) {
	let generator = editor.create_node_by_name_at(rectangle_definition(), 0, 40).await;
	let first = editor.create_node_by_name_at(fill_definition(), 7, 40).await;
	let middle = editor.create_node_by_name_at(fill_definition(), 14, 40).await;
	let last = editor.create_node_by_name_at(fill_definition(), 21, 40).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.create_wire(&OutputConnector::primary_output(generator), &InputConnector::primary_input(first), &[]);
	network_interface.create_wire(&OutputConnector::primary_output(first), &InputConnector::primary_input(middle), &[]);
	network_interface.create_wire(&OutputConnector::primary_output(middle), &InputConnector::primary_input(last), &[]);

	(generator, first, middle, last)
}

async fn duplicate_selection(editor: &mut EditorTestUtils, nodes: Vec<NodeId>) -> Vec<NodeId> {
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes }).await;
	editor.handle_message(DocumentMessage::DuplicateSelectedLayers).await;
	editor.active_document().network_interface.selected_nodes().selected_nodes().copied().collect()
}

fn primary_input_of(network_interface: &NodeNetworkInterface, node_id: NodeId) -> Option<NodeInput> {
	network_interface.input_from_connector(&InputConnector::primary_input(node_id), &[]).cloned()
}

#[tokio::test]
async fn duplicating_a_node_inside_a_flow_puts_its_copy_right_after_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (_, _, middle, last) = floating_flow(&mut editor).await;

	let copies = duplicate_selection(&mut editor, vec![middle]).await;
	assert_eq!(copies.len(), 1, "The copy should become the selection");

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(
		primary_input_of(network_interface, copies[0]),
		Some(NodeInput::node(middle, 0)),
		"The copy should be fed by the original"
	);
	assert_eq!(
		primary_input_of(network_interface, last),
		Some(NodeInput::node(copies[0], 0)),
		"The copy should feed what the original fed"
	);

	assert_invariants(&editor, "after duplicating a node inside a flow");
}

#[tokio::test]
async fn duplicating_the_end_of_a_flow_extends_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (_, _, _, last) = floating_flow(&mut editor).await;

	let copies = duplicate_selection(&mut editor, vec![last]).await;
	assert_eq!(copies.len(), 1, "The copy should become the selection");
	assert_eq!(
		primary_input_of(&editor.active_document().network_interface, copies[0]),
		Some(NodeInput::node(last, 0)),
		"The copy should extend the flow from the original"
	);

	assert_invariants(&editor, "after duplicating the end of a flow");
}

#[tokio::test]
async fn duplicating_a_solo_node_or_a_generator_makes_an_offset_copy_outside_the_flow() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (generator, first, _, _) = floating_flow(&mut editor).await;
	let solo = editor.create_node_by_name_at(fill_definition(), 0, 60).await;

	let copies = duplicate_selection(&mut editor, vec![generator, solo]).await;
	assert_eq!(copies.len(), 2, "Both copies should become the selection");

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(
		primary_input_of(network_interface, first),
		Some(NodeInput::node(generator, 0)),
		"The generator should still feed the flow"
	);
	for copy in copies {
		let original = if network_interface.reference(&copy, &[]) == network_interface.reference(&generator, &[]) {
			generator
		} else {
			solo
		};
		assert_eq!(
			network_interface.position(&copy, &[]),
			network_interface.position(&original, &[]).map(|position| position + IVec2::new(2, 2)),
			"A copy outside the flow should sit offset from its original"
		);
		assert!(
			!matches!(primary_input_of(network_interface, copy), Some(NodeInput::Node { .. })),
			"A copy outside the flow should not be wired into it"
		);
	}

	assert_invariants(&editor, "after duplicating nodes outside any flow");
}

#[tokio::test]
async fn duplicating_a_floating_layer_in_a_stack_puts_its_copy_above_it_with_its_content() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, _, _, _, _) = layer_with_chain(&mut editor).await;
	let layer_above = editor.create_node_by_name_at(merge_definition(), 30, 0).await;

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_to_node_or_layer(&layer_above, &[], true);
	network_interface.create_wire(&OutputConnector::primary_output(layer), &InputConnector::primary_input(layer_above), &[]);

	let copies = duplicate_selection(&mut editor, vec![layer]).await;
	assert_eq!(copies.len(), 1, "The copy should become the selection");

	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.is_layer(&copies[0], &[]), "The copy should be a layer");
	assert_eq!(
		primary_input_of(network_interface, copies[0]),
		Some(NodeInput::node(layer, 0)),
		"The copy should be stacked on the original"
	);
	assert_eq!(
		primary_input_of(network_interface, layer_above),
		Some(NodeInput::node(copies[0], 0)),
		"The layer above should now be stacked on the copy"
	);
	assert_eq!(chain_of(network_interface, copies[0]).len(), 3, "The copy should bring a copy of the layer's chain");

	assert_invariants(&editor, "after duplicating a floating layer in a stack");
}

#[tokio::test]
async fn duplicating_a_solo_floating_layer_makes_an_offset_copy_with_its_content() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, _, _, _, _) = layer_with_chain(&mut editor).await;

	let copies = duplicate_selection(&mut editor, vec![layer]).await;
	assert_eq!(copies.len(), 1, "The copy should become the selection");

	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.is_layer(&copies[0], &[]), "The copy should be a layer");
	assert_eq!(chain_of(network_interface, copies[0]).len(), 3, "The copy should bring a copy of the layer's chain");
	assert_eq!(
		network_interface.position(&copies[0], &[]),
		network_interface.position(&layer, &[]).map(|position| position + IVec2::new(2, 2)),
		"The copy should sit offset from the original"
	);

	assert_invariants(&editor, "after duplicating a solo floating layer");
}

async fn copy_in_graph(editor: &mut EditorTestUtils, nodes: Vec<NodeId>) -> String {
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes }).await;
	editor
		.handle_message(NodeGraphMessage::Copy)
		.await
		.into_iter()
		.find_map(|message| match message {
			FrontendMessage::TriggerClipboardWrite { content } => Some(content),
			_ => None,
		})
		.expect("Copying nodes should write them to the clipboard")
}

async fn paste(editor: &mut EditorTestUtils, clipboard: String) -> Vec<NodeId> {
	editor
		.handle_message(ClipboardMessage::ReadClipboard {
			content: ClipboardContentRaw::Text(clipboard),
		})
		.await;
	editor.active_document().network_interface.selected_nodes().selected_nodes().copied().collect()
}

#[tokio::test]
async fn pasting_graph_nodes_over_their_copied_selection_duplicates_them_into_the_flow() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (_, _, middle, last) = floating_flow(&mut editor).await;

	let clipboard = copy_in_graph(&mut editor, vec![middle]).await;
	let pasted = paste(&mut editor, clipboard).await;
	assert_eq!(pasted.len(), 1, "The pasted copy should become the selection");

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(
		primary_input_of(network_interface, pasted[0]),
		Some(NodeInput::node(middle, 0)),
		"The copy should be fed by the original"
	);
	assert_eq!(
		primary_input_of(network_interface, last),
		Some(NodeInput::node(pasted[0], 0)),
		"The copy should feed what the original fed"
	);

	assert_invariants(&editor, "after pasting graph nodes over their copied selection");
}

#[tokio::test]
async fn pasting_graph_nodes_over_an_empty_selection_puts_loose_copies_in_the_graph() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (_, _, middle, last) = floating_flow(&mut editor).await;

	let clipboard = copy_in_graph(&mut editor, vec![middle]).await;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() }).await;
	let pasted = paste(&mut editor, clipboard).await;
	assert_eq!(pasted.len(), 1, "The pasted copy should become the selection");

	let document = editor.active_document();
	assert!(
		!matches!(primary_input_of(&document.network_interface, pasted[0]), Some(NodeInput::Node { .. })),
		"The pasted copy should be outside any flow"
	);
	assert_eq!(
		primary_input_of(&document.network_interface, last),
		Some(NodeInput::node(middle, 0)),
		"The flow should be left as it was"
	);
	assert!(document.graph_view_overlay_open(), "The graph should open to show the pasted copy");

	assert_invariants(&editor, "after pasting graph nodes over an empty selection");
}

#[tokio::test]
async fn pasting_chain_nodes_over_an_empty_selection_puts_loose_copies_in_the_graph() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, near, far, source, _) = layer_with_chain_in_artboard(&mut editor).await;
	let copy = editor.active_document().network_interface.copy_chain_node(&near, &[]).expect("The chain node should be copyable");

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() }).await;
	editor.handle_message(DocumentMessage::PasteChainNodes { copies: vec![copy] }).await;

	let document = editor.active_document();
	let pasted = document.network_interface.selected_nodes().selected_nodes().copied().collect::<Vec<_>>();
	assert_eq!(pasted.len(), 1, "The pasted copy should become the selection");
	assert!(
		!matches!(primary_input_of(&document.network_interface, pasted[0]), Some(NodeInput::Node { .. })),
		"The pasted copy should be outside any flow"
	);
	assert_eq!(chain_of(&document.network_interface, layer), vec![near, far, source], "The chain should be left as it was");
	assert!(document.graph_view_overlay_open(), "The graph should open to show the pasted copy");

	assert_invariants(&editor, "after pasting chain nodes over an empty selection");
}

#[tokio::test]
async fn a_copied_layer_pastes_the_same_whether_copied_in_the_graph_or_not() {
	for copied_in_graph in [false, true] {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (layer, _, _, _, _) = layer_with_chain_in_artboard(&mut editor).await;

		editor.handle_message(DocumentMessage::GraphViewOverlay { open: copied_in_graph }).await;
		let clipboard = copy_in_graph(&mut editor, vec![layer]).await;
		editor.handle_message(DocumentMessage::GraphViewOverlay { open: false }).await;
		let pasted = paste(&mut editor, clipboard).await;
		assert_eq!(pasted.len(), 1, "Only the pasted layer should be selected (copied in the graph: {copied_in_graph})");

		// The copy goes on top of the stack its original is in, bringing its chain along
		let network_interface = &editor.active_document().network_interface;
		let metadata = network_interface.document_metadata();
		let parent = LayerNodeIdentifier::new(layer, network_interface).parent(metadata);
		assert!(parent.is_some(), "The original layer should be in the layer structure");
		assert_eq!(
			parent.and_then(|parent| parent.children(metadata).next()).map(|child| child.to_node()),
			Some(pasted[0]),
			"The pasted layer should go on top of the stack (copied in the graph: {copied_in_graph})"
		);
		assert_eq!(
			chain_of(network_interface, pasted[0]).len(),
			3,
			"The pasted layer should bring its chain (copied in the graph: {copied_in_graph})"
		);

		assert_invariants(&editor, "after pasting a copied layer");
	}
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
			glide: false,
		})
		.await;
	let network_interface = &editor.active_document().network_interface;
	let after = network_interface.position(&third, &[]).expect("Surviving layer should have a position");
	assert_eq!(after.y - before.y, 1, "A single nudge should move the layer exactly one unit");

	assert_invariants(&editor, "after stack pushes, a delete collapse, and a nudge");
}

#[tokio::test]
async fn each_arrow_key_nudge_is_its_own_undo_step() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	let start = editor.active_document().network_interface.position(&node, &[]).expect("The node should have a position");

	editor.handle_message(NodeGraphMessage::NudgeSelectedNodes { direction: Direction::Right }).await;
	editor.handle_message(NodeGraphMessage::NudgeSelectedNodes { direction: Direction::Right }).await;

	// Undoing takes back only the last nudge, not the node's creation before them
	editor.handle_message(DocumentMessage::Undo).await;
	let network_interface = &editor.active_document().network_interface;
	let position = network_interface.position(&node, &[]).expect("Undo should keep the node created before the nudges");
	assert_eq!(position.x - start.x, 1, "Undo should take back only the last nudge");

	assert_invariants(&editor, "after nudging and undoing");
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

#[tokio::test]
async fn reordering_an_import_to_the_front_makes_it_the_exposed_primary() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];

	// Dropping an import or export just below itself leaves it where it was
	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.reorder_import(1, 2, &path);
	assert!(
		network_interface.reference(&merge, &[]).is_some(),
		"dropping an import back where it was should leave the subgraph matching its definition"
	);
	network_interface.reorder_export(0, 1, &path);
	assert!(
		network_interface.reference(&merge, &[]).is_some(),
		"dropping an export back where it was should leave the subgraph matching its definition"
	);

	network_interface.add_import(TaggedValue::Number(1.), false, -1, "Parameter", "", &path);
	let parameter_index = network_interface.number_of_imports(&path) - 1;
	let previous_primary_name = network_interface.displayed_import_name(0, &path);

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor
		.handle_message(NodeGraphMessage::ReorderImport {
			start_index: parameter_index,
			end_index: 0,
		})
		.await;
	let network_interface = &editor.active_document().network_interface;
	let new_primary = network_interface
		.document_node(&merge, &[])
		.and_then(|node| node.inputs.first())
		.expect("Merge should have a primary input");
	assert!(new_primary.is_exposed(), "a parameter moved to the front should be exposed as the primary import");
	assert_eq!(network_interface.displayed_import_name(0, &path), "Parameter");
	assert_eq!(
		network_interface.displayed_import_name(1, &path),
		previous_primary_name,
		"the previous primary should become the first secondary import"
	);

	assert_invariants(&editor, "after reordering a parameter to be the primary import");
}

#[tokio::test]
async fn moving_the_primary_import_among_the_secondary_ones_leaves_an_empty_primary() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.add_import(TaggedValue::Number(1.), true, -1, "Parameter", "", &path);
	let primary_name = network_interface.displayed_import_name(0, &path);
	let secondary_name = network_interface.displayed_import_name(1, &path);
	let fed_by_primary = fed_inputs(&editor, &OutputConnector::Import(0), &path);

	// Drop the primary between the first and second secondary imports
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(NodeGraphMessage::ReorderImport { start_index: 0, end_index: 2 }).await;

	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.hidden_primary_import(&path), "the primary's place should be left empty");
	assert_eq!(network_interface.displayed_import_name(1, &path), secondary_name, "the first secondary import should stay secondary");
	assert_eq!(network_interface.displayed_import_name(2, &path), primary_name, "the moved import should land where it was dropped");
	assert_eq!(
		fed_inputs(&editor, &OutputConnector::Import(2), &path),
		fed_by_primary,
		"the moved import should keep feeding what it fed"
	);

	assert_invariants(&editor, "after moving the primary import among the secondary ones");
}

#[tokio::test]
async fn the_primary_import_can_become_the_first_secondary_and_an_empty_primary_can_be_filled() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	let network_interface = &editor.active_document().network_interface;
	let primary_name = network_interface.displayed_import_name(0, &path);
	let secondary_name = network_interface.displayed_import_name(1, &path);
	let import_count = network_interface.number_of_imports(&path);

	// Dropped just below the separator, the primary becomes the first secondary import
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(NodeGraphMessage::ReorderImport { start_index: 0, end_index: 1 }).await;

	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.hidden_primary_import(&path), "the primary's place should be left empty");
	assert_eq!(network_interface.displayed_import_name(1, &path), primary_name, "the moved import should be the first secondary one");
	assert_eq!(network_interface.displayed_import_name(2, &path), secondary_name);

	// Dropping a secondary import into the empty primary fills it, rather than keeping the empty one as a secondary import
	editor.handle_message(NodeGraphMessage::ReorderImport { start_index: 2, end_index: 0 }).await;

	let network_interface = &editor.active_document().network_interface;
	assert!(!network_interface.hidden_primary_import(&path), "the primary should be filled");
	assert_eq!(network_interface.displayed_import_name(0, &path), secondary_name);
	assert_eq!(network_interface.displayed_import_name(1, &path), primary_name);
	assert_eq!(network_interface.number_of_imports(&path), import_count, "the empty primary should be gone rather than pushed down");

	assert_invariants(&editor, "after filling an empty primary import");
}

#[tokio::test]
async fn the_primary_export_can_become_the_first_secondary() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(NodeGraphMessage::AddSecondaryExport).await;
	let network_interface = &editor.active_document().network_interface;
	let primary_source = network_interface.upstream_output_connector(&InputConnector::Export(0), &path);
	assert!(primary_source.is_some(), "Merge's primary export should be wired");

	editor.handle_message(NodeGraphMessage::ReorderExport { start_index: 0, end_index: 1 }).await;

	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.hidden_primary_export(&path), "the primary's place should be left empty");
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::Export(1), &path),
		primary_source,
		"the moved export should be the first secondary one"
	);
	assert_eq!(network_interface.number_of_exports(&path), 3, "the empty primary should be added in front of both exports");

	assert_invariants(&editor, "after making the primary export secondary");
}

/// The inputs fed by `output_connector` in the network at `network_path`, in no particular order.
fn fed_inputs(editor: &EditorTestUtils, output_connector: &OutputConnector, network_path: &[NodeId]) -> HashSet<InputConnector> {
	let network_interface = &editor.active_document().network_interface;
	network_interface
		.with_outward_wires(network_path, |outward_wires| outward_wires.get(output_connector).cloned())
		.flatten()
		.unwrap_or_default()
		.into_iter()
		.collect()
}

#[tokio::test]
async fn dragging_a_dot_out_of_an_import_takes_over_the_wires_the_import_fed() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	// TODO: Remove once staging the recorded edits stores a node wired to an import inside a subgraph as it is, instead of drifting from it
	editor.handle_message(PreferencesMessage::ValidateStorageRoundTrip { enabled: false }).await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	let content_import = OutputConnector::Import(1);
	let fed_before = fed_inputs(&editor, &content_import, &path);
	assert!(!fed_before.is_empty(), "Merge's content import should feed its subgraph: fixture is wrong");

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let import_position = editor
		.active_document()
		.network_interface
		.output_position(&content_import, &path)
		.expect("the import should have a position");
	double_press_drag_in_graph(&mut editor, &path, import_position, import_position + DVec2::new(72., 0.)).await;

	let fed_after = fed_inputs(&editor, &content_import, &path);
	let [InputConnector::Node { node_id: dot, input_index: 0 }] = fed_after.into_iter().collect::<Vec<_>>()[..] else {
		panic!("the import should feed only the dot's input");
	};
	assert!(editor.active_document().network_interface.is_redirect_dot(&dot, &path));
	assert_eq!(fed_inputs(&editor, &OutputConnector::primary_output(dot), &path), fed_before, "the dot should feed what the import fed");

	assert_invariants(&editor, "after dragging a dot out of an import");
}

#[tokio::test]
async fn editing_import_and_export_descriptions_stores_them_on_the_subgraph_node() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor
		.handle_message(NodeGraphMessage::SetImportExportDescription {
			description: "The artwork to add.".to_string(),
			index: ImportOrExport::Import(1),
		})
		.await;
	editor
		.handle_message(NodeGraphMessage::SetImportExportDescription {
			description: "The combined result.".to_string(),
			index: ImportOrExport::Export(0),
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	let import_description = network_interface.persistent_input_metadata(&merge, 1, &[]).map(|metadata| metadata.input_description.as_str());
	assert_eq!(import_description, Some("The artwork to add."));
	assert_eq!(network_interface.output_description(&merge, 0, &[]), "The combined result.");

	assert_invariants(&editor, "after editing import and export descriptions");
}

#[tokio::test]
async fn the_empty_import_connectors_add_the_next_import_or_fill_the_empty_primary() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	let import_count = editor.active_document().network_interface.number_of_imports(&path);

	// With a primary, the only one is past the last import
	let network_interface = &editor.active_document().network_interface;
	let empty_connectors = network_interface.empty_import_export_connectors(true, &path);
	assert_eq!(empty_connectors.iter().map(|(index, _)| *index).collect::<Vec<_>>(), vec![import_count]);

	// The attribute name given to Merge's interior Write Attribute node
	let write_attribute = network_interface
		.nested_network(&path)
		.and_then(|network| {
			network
				.nodes
				.iter()
				.find(|(_, node)| node.implementation == DocumentNodeImplementation::ProtoNode(graphene_std::graphic::write_attribute::IDENTIFIER))
		})
		.map(|(node_id, _)| *node_id)
		.expect("Merge should contain a Write Attribute node");
	let value_input = InputConnector::node_at_index(write_attribute, 1);
	let Some(NodeInput::Value { tagged_value, .. }) = network_interface.input_from_connector(&value_input, &path) else {
		panic!("Merge's Write Attribute node should be given its attribute name as a value");
	};
	let displaced_value = (**tagged_value).clone();

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor
		.handle_message(NodeGraphMessage::AddImportAndConnect {
			import_index: import_count,
			input_connector: value_input,
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(network_interface.number_of_imports(&path), import_count + 1, "the next import should be added");
	assert!(
		fed_inputs(&editor, &OutputConnector::Import(import_count), &path).contains(&value_input),
		"the new import should be wired to the input"
	);
	let network_interface = &editor.active_document().network_interface;
	let new_input = network_interface.input_from_connector(&InputConnector::node_at_index(merge, import_count), &[]);
	assert!(
		matches!(new_input, Some(NodeInput::Value { tagged_value, exposed: true }) if **tagged_value == displaced_value),
		"the new import should take over the value its wire displaced, found {new_input:?}"
	);
	assert_eq!(
		network_interface.displayed_import_name(import_count, &path),
		displaced_value.ty().nested_type().to_string(),
		"an import without a name should be shown by the type it took over"
	);

	// Emptying the primary adds a connector in its place, which fills it again from an input that's already wired
	editor.handle_message(NodeGraphMessage::ExposeEncapsulatingInput { input_index: 0, exposed: false }).await;
	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.empty_import_export_connectors(true, &path).iter().any(|(index, _)| *index == 0));

	// The content given to Merge's interior Write Attribute node by its Into Group node
	let wired_input = InputConnector::node_at_index(write_attribute, 0);
	assert!(
		matches!(network_interface.input_from_connector(&wired_input, &path), Some(NodeInput::Node { .. })),
		"Merge's Write Attribute node should be wired its content from another node"
	);
	editor
		.handle_message(NodeGraphMessage::AddImportAndConnect {
			import_index: 0,
			input_connector: wired_input,
		})
		.await;
	let network_interface = &editor.active_document().network_interface;
	assert!(!network_interface.hidden_primary_import(&path), "the empty primary should be filled");
	assert_eq!(network_interface.number_of_imports(&path), import_count + 1, "filling the empty primary shouldn't add another import");

	let drawn_wire = network_interface
		.network_metadata(&path)
		.and_then(|network_metadata| network_metadata.transient_metadata.wires.borrow().get(&wired_input).cloned())
		.and_then(|wire| wire.wire_path_update);
	let current_wire = network_interface.wire_path_from_input(&wired_input, GraphWireStyle::default(), false, &GlideOffsets::default(), &path);
	assert_eq!(drawn_wire, current_wire, "the input's wire should be redrawn from its new import");

	assert_invariants(&editor, "after adding imports from the empty connectors");
}

#[tokio::test]
async fn showing_a_secondary_import_in_the_inspector_keeps_its_wires_in_the_subgraph() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	let content_import = OutputConnector::Import(1);
	let fed_before = fed_inputs(&editor, &content_import, &path);

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(NodeGraphMessage::ExposeEncapsulatingInput { input_index: 1, exposed: false }).await;

	let network_interface = &editor.active_document().network_interface;
	let content_input = network_interface
		.input_from_connector(&InputConnector::node_at_index(merge, 1), &[])
		.expect("Merge should have a content input");
	assert!(!content_input.is_exposed(), "the subgraph node should show the input as a parameter instead of a connector");
	assert_eq!(
		fed_inputs(&editor, &content_import, &path),
		fed_before,
		"the import should still feed the same inputs within the subgraph"
	);

	assert_invariants(&editor, "after showing a secondary import in the Inspector");
}

#[tokio::test]
async fn toggle_preview_transitions_with_a_connected_export() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let artboard = NodeId::new();
	editor.handle_message(new_artboard_message(artboard)).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;

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

	// Toggling the restore node while previewing ends the preview by restoring it
	network_interface.toggle_preview(node, &[]);
	network_interface.toggle_preview(artboard, &[]);
	assert_eq!(export_node(network_interface), Some(artboard));
	assert_eq!(network_interface.previewing(&[]), Previewing::No);

	// Toggling the node already feeding the export changes nothing
	network_interface.toggle_preview(artboard, &[]);
	assert_eq!(export_node(network_interface), Some(artboard));
	assert_eq!(network_interface.previewing(&[]), Previewing::No);

	assert_invariants(&editor, "after cycling through the preview states");
}

fn assert_interface_invariants(network_interface: &NodeNetworkInterface, context: &str) {
	let violations = network_interface.validate_invariants();
	assert!(violations.is_empty(), "Invariant violations {context}:\n{}", violations.join("\n"));
}

fn value_input() -> NodeInput {
	NodeInput::value(TaggedValue::None, false)
}

/// A source feeding a node that feeds a consumer, with the outward wire cache loaded so every later mutation has to keep it current.
fn wired_interface() -> (NodeNetworkInterface, NodeId, NodeId, NodeId) {
	let (source, node, consumer) = (NodeId(1), NodeId(2), NodeId(3));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(source, plain_node(1), &[]);
	network_interface.insert_node(node, plain_node(2), &[]);
	network_interface.insert_node(consumer, plain_node(1), &[]);
	network_interface.set_input(&InputConnector::node_at_index(node, 0), NodeInput::node(source, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(consumer, 0), NodeInput::node(node, 0), &[]);

	assert!(network_interface.outward_wires(&[]).is_some());
	assert_interface_invariants(&network_interface, "after wiring the fixture");
	(network_interface, source, node, consumer)
}

#[test]
fn replacing_inputs_keeps_the_loaded_wire_cache_exact() {
	let (mut network_interface, source, node, _) = wired_interface();

	let mut template = NodeTemplate {
		inputs: vec![value_input(); 3],
		..Default::default()
	};
	template.normalize_input_metadata();
	network_interface.replace_inputs(&node, &[], &mut template);
	assert_interface_invariants(&network_interface, "after replacing the inputs of a wired node");

	// The wire returns at a new index, as a migration restores the inputs it keeps
	network_interface.set_input(&InputConnector::node_at_index(node, 2), NodeInput::node(source, 0), &[]);
	assert_interface_invariants(&network_interface, "after rewiring the source into the replaced node");
}

// A chain node is placed only by its link into the next node, so it leaves the chain only when replacing inputs drops that link
#[test]
fn replacing_inputs_breaks_a_chain_only_where_its_link_is_dropped() {
	let (layer, chain_node, side_consumer) = (NodeId(1), NodeId(2), NodeId(3));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(
		layer,
		NodeTemplate {
			node_type_metadata: NodeTypePersistentMetadata::layer(IVec2::ZERO),
			..plain_node(2)
		},
		&[],
	);
	let two_exports = NodeNetworkTemplate {
		exports: vec![NodeInput::value(TaggedValue::None, true), value_input()],
		..Default::default()
	};
	network_interface.insert_node(
		chain_node,
		NodeTemplate {
			inputs: vec![value_input()],
			implementation: NodeTemplateImplementation::Network(two_exports),
			output_names: vec![String::new(); 2],
			node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-7, 0)),
			..Default::default()
		},
		&[],
	);
	network_interface.insert_node(
		side_consumer,
		NodeTemplate {
			inputs: vec![value_input(); 2],
			..Default::default()
		},
		&[],
	);

	// Only consumers of the primary output keep a node out of a chain, so one feeding a parameter from its second output still joins
	network_interface.set_input(&InputConnector::node_at_index(side_consumer, 1), NodeInput::node(chain_node, 1), &[]);
	network_interface.set_input(&InputConnector::node_at_index(layer, 1), NodeInput::node(chain_node, 0), &[]);
	assert!(network_interface.is_chain(&chain_node, &[]), "the node should join the layer's chain");
	let chain_position = network_interface.position(&chain_node, &[]);

	let replacement = || {
		let mut template = NodeTemplate {
			inputs: vec![value_input(); 2],
			..Default::default()
		};
		template.normalize_input_metadata();
		template
	};

	network_interface.replace_inputs(&side_consumer, &[], &mut replacement());
	assert!(
		network_interface.is_chain(&chain_node, &[]),
		"dropping the wire from its second output should leave the node in the chain"
	);

	network_interface.replace_inputs(&layer, &[], &mut replacement());
	assert!(!network_interface.is_chain(&chain_node, &[]), "dropping its link into the layer should take the node out of the chain");
	assert_eq!(network_interface.position(&chain_node, &[]), chain_position, "the node should stay where the chain placed it");
	assert_interface_invariants(&network_interface, "after replacing the inputs around a chain node");
}

#[test]
fn replacing_an_implementation_keeps_its_outputs_listed_in_the_wire_cache() {
	let (mut network_interface, _, node, _) = wired_interface();
	let second_output = OutputConnector::node(node, 1);

	// An implementation swap leaves the output names to the open path, so restore them as it does before validating
	let swap_implementation = |network_interface: &mut NodeNetworkInterface, template: &mut NodeTemplate, context: &str| {
		network_interface.replace_implementation(&node, &[], template);
		let document_node = network_interface.document_node(&node, &[]).cloned().unwrap();
		network_interface.validate_output_names(&node, &document_node, &[]);
		assert_interface_invariants(network_interface, context);
	};

	let two_exports = NodeNetworkTemplate {
		exports: vec![value_input(), value_input()],
		..Default::default()
	};
	let mut template = NodeTemplate {
		implementation: NodeTemplateImplementation::Network(two_exports),
		..Default::default()
	};
	swap_implementation(&mut network_interface, &mut template, "after giving a node a second output");
	assert!(
		network_interface.outward_wires(&[]).is_some_and(|wires| wires.contains_key(&second_output)),
		"a gained output should be listed"
	);

	swap_implementation(&mut network_interface, &mut NodeTemplate::default(), "after taking the second output away");
	assert!(
		network_interface.outward_wires(&[]).is_some_and(|wires| !wires.contains_key(&second_output)),
		"a lost output should be delisted"
	);
}

// A vanished output keeps its entry while a consumer is still wired to it, so the entry has to go when that consumer leaves
#[test]
fn rewiring_off_a_vanished_output_delists_it_from_the_wire_cache() {
	let (mut network_interface, source, node, consumer) = wired_interface();
	let second_output = OutputConnector::node(node, 1);

	let restore_output_names = |network_interface: &mut NodeNetworkInterface| {
		let document_node = network_interface.document_node(&node, &[]).cloned().unwrap();
		network_interface.validate_output_names(&node, &document_node, &[]);
	};

	// Give the node a second output and move the consumer onto it
	let two_exports = NodeNetworkTemplate {
		exports: vec![value_input(), value_input()],
		..Default::default()
	};
	let mut template = NodeTemplate {
		implementation: NodeTemplateImplementation::Network(two_exports),
		..Default::default()
	};
	network_interface.replace_implementation(&node, &[], &mut template);
	restore_output_names(&mut network_interface);
	network_interface.set_input(&InputConnector::node_at_index(consumer, 0), NodeInput::node(node, 1), &[]);
	assert_interface_invariants(&network_interface, "after wiring a consumer to the second output");

	// Taking that output away leaves the consumer dangling, which a fresh computation still lists
	network_interface.replace_implementation(&node, &[], &mut plain_node(0));
	restore_output_names(&mut network_interface);
	assert!(
		network_interface.outward_wires(&[]).is_some_and(|wires| wires.contains_key(&second_output)),
		"a vanished output should keep its entry while something is still wired to it"
	);

	network_interface.set_input(&InputConnector::node_at_index(consumer, 0), NodeInput::node(source, 0), &[]);
	assert!(
		network_interface.outward_wires(&[]).is_some_and(|wires| !wires.contains_key(&second_output)),
		"the entry should go once the last consumer leaves a vanished output"
	);
	assert_interface_invariants(&network_interface, "after rewiring off a vanished output");
}

#[test]
fn inserting_and_deleting_nodes_keeps_the_loaded_wire_cache_exact() {
	let (mut network_interface, _, node, _) = wired_interface();

	network_interface.insert_node(NodeId(4), NodeTemplate::default(), &[]);
	assert_interface_invariants(&network_interface, "after inserting a node while the cache is loaded");

	network_interface.delete_nodes(vec![node], false, &[]);
	assert_interface_invariants(&network_interface, "after deleting a wired node");
}

#[test]
fn adding_an_import_ahead_of_a_wired_input_keeps_the_loaded_wire_cache_exact() {
	let (source, node) = (NodeId(1), NodeId(2));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(
		source,
		NodeTemplate {
			inputs: vec![value_input()],
			..Default::default()
		},
		&[],
	);
	let network = NodeNetworkTemplate {
		exports: vec![value_input()],
		..Default::default()
	};
	let template = NodeTemplate {
		implementation: NodeTemplateImplementation::Network(network),
		inputs: vec![value_input()],
		output_names: vec![String::new()],
		..Default::default()
	};
	network_interface.insert_node(node, template, &[]);
	network_interface.set_input(&InputConnector::node_at_index(node, 0), NodeInput::node(source, 0), &[]);
	assert!(network_interface.outward_wires(&[]).is_some());

	// Inserting ahead of the wired input moves that wire to index 1
	network_interface.add_import(TaggedValue::None, true, 0, "", "", &[node]);
	assert_interface_invariants(&network_interface, "after adding an import ahead of a wired input");
	let consumers = network_interface.outward_wires(&[]).and_then(|wires| wires.get(&OutputConnector::node(source, 0)).cloned());
	assert_eq!(consumers.as_deref(), Some([InputConnector::node_at_index(node, 1)].as_slice()));
}

#[tokio::test]
async fn a_subgraph_output_fed_by_a_value_can_produce_its_type() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.add_export(TaggedValue::Number(1.), -1, "", &path);
	network_interface.add_export(TaggedValue::None, -1, "", &path);
	let number_output = OutputConnector::node(merge, network_interface.number_of_exports(&path) - 2);
	let placeholder_output = OutputConnector::node(merge, network_interface.number_of_exports(&path) - 1);

	let number_type = TaggedValue::Number(1.).ty().nested_type().clone();
	assert_eq!(
		network_interface.possible_output_types(&number_output, &[]),
		vec![number_type],
		"an export given a value can only produce its type"
	);
	assert!(
		network_interface.possible_output_types(&placeholder_output, &[]).is_empty(),
		"a new export's empty placeholder shouldn't be listed as a type it can produce"
	);
}

/// Presses at `from` and releases at `to`, both in the graph space of the network at `path`, as the node graph receives a mouse drag.
async fn drag_in_graph(editor: &mut EditorTestUtils, path: &[NodeId], from: DVec2, to: DVec2) {
	let node_graph_to_viewport = editor
		.active_document()
		.network_interface
		.network_metadata(path)
		.expect("the network should have metadata")
		.persistent_metadata
		.navigation_metadata
		.node_graph_to_viewport;
	let (from, to) = (node_graph_to_viewport.transform_point2(from), node_graph_to_viewport.transform_point2(to));

	editor.move_mouse(from.x, from.y, ModifierKeys::empty(), MouseKeys::empty()).await;
	editor
		.handle_message(NodeGraphMessage::PointerDown {
			shift_click: false,
			control_click: false,
			alt_click: false,
			right_click: false,
		})
		.await;
	editor.move_mouse(to.x, to.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
	editor.handle_message(NodeGraphMessage::PointerMove { shift: Key::Shift }).await;
	editor.handle_message(NodeGraphMessage::PointerUp).await;
}

#[tokio::test]
async fn a_wire_dragged_backward_from_an_empty_input_connects_to_the_output_it_is_dropped_on() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let fill_content = InputConnector::node_at_index(fill, 0);

	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.upstream_output_connector(&fill_content, &[]).is_none(), "Fill's content should start out unwired");
	let input_position = network_interface.input_position(&fill_content, &[]).expect("Fill's content should have a position");
	let output_position = network_interface
		.output_position(&OutputConnector::node(rectangle, 0), &[])
		.expect("Rectangle's output should have a position");

	drag_in_graph(&mut editor, &[], input_position, output_position).await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(
		network_interface.upstream_output_connector(&fill_content, &[]),
		Some(OutputConnector::node(rectangle, 0)),
		"the wire dragged back from Fill's content should connect it to Rectangle's output"
	);
}

#[tokio::test]
async fn a_wire_dragged_backward_onto_empty_space_wires_the_node_picked_from_the_menu() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let fill = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let fill_content = InputConnector::node_at_index(fill, 0);
	let input_position = editor
		.active_document()
		.network_interface
		.input_position(&fill_content, &[])
		.expect("Fill's content should have a position");

	drag_in_graph(&mut editor, &[], input_position, input_position - DVec2::new(240., 0.)).await;

	let context_menu = editor.active_document().node_graph_handler.context_menu.clone();
	assert!(
		matches!(context_menu.map(|menu| menu.context_menu_data), Some(ContextMenuData::CreateNode { compatible_type: None })),
		"dropping the wire on empty space should open the node menu without filtering it"
	);

	let rectangle = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(rectangle),
			node_type: rectangle_definition(),
			xy: Some((0, 0)),
			add_transaction: true,
		})
		.await;
	assert_eq!(
		editor.active_document().network_interface.upstream_output_connector(&fill_content, &[]),
		Some(OutputConnector::node(rectangle, 0)),
		"the node picked from the menu should be wired into Fill's content"
	);
}

#[tokio::test]
async fn a_wire_dragged_backward_from_the_dimmed_export_connector_adds_an_export() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;

	let network_interface = &editor.active_document().network_interface;
	let export_count = network_interface.number_of_exports(&path);
	let (new_export_index, empty_export_position) = *network_interface
		.empty_import_export_connectors(false, &path)
		.last()
		.expect("there should be a dimmed connector for adding an export");
	let result_output = network_interface
		.upstream_output_connector(&InputConnector::Export(0), &path)
		.expect("Merge's export should be fed by a node inside it");
	let result_position = network_interface
		.output_position(&result_output, &path)
		.expect("the node feeding Merge's export should have an output position");

	drag_in_graph(&mut editor, &path, empty_export_position, result_position).await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(network_interface.number_of_exports(&path), export_count + 1, "the export should be added");
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::Export(new_export_index), &path),
		Some(result_output),
		"the added export should be wired from the output the wire was dropped on"
	);

	assert_invariants(&editor, "after dragging a wire backward from the dimmed export connector");
}

#[tokio::test]
async fn a_wire_dragged_from_the_dimmed_import_connector_onto_empty_space_wires_a_new_import_into_the_node_picked_from_the_menu() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;

	let network_interface = &editor.active_document().network_interface;
	let import_count = network_interface.number_of_imports(&path);
	let (new_import_index, empty_import_position) = *network_interface
		.empty_import_export_connectors(true, &path)
		.last()
		.expect("there should be a dimmed connector for adding an import");

	drag_in_graph(&mut editor, &path, empty_import_position, empty_import_position + DVec2::new(240., 0.)).await;

	let context_menu = editor.active_document().node_graph_handler.context_menu.clone();
	assert!(
		matches!(context_menu.map(|menu| menu.context_menu_data), Some(ContextMenuData::CreateNode { compatible_type: None })),
		"dropping the wire on empty space should open the node menu without filtering it, as the import has no type yet"
	);

	let fill = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(fill),
			node_type: fill_definition(),
			xy: Some((10, 0)),
			add_transaction: true,
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(network_interface.number_of_imports(&path), import_count + 1, "the import should be added");
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::node_at_index(fill, 0), &path),
		Some(OutputConnector::Import(new_import_index)),
		"the added import should be wired into the node picked from the menu"
	);

	assert_invariants(&editor, "after dragging a wire from the dimmed import connector onto empty space");
}

#[tokio::test]
async fn clicking_an_empty_input_without_dragging_leaves_it_and_opens_no_menu() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let fill = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let fill_content = InputConnector::node_at_index(fill, 0);
	let input_position = editor
		.active_document()
		.network_interface
		.input_position(&fill_content, &[])
		.expect("Fill's content should have a position");

	drag_in_graph(&mut editor, &[], input_position, input_position).await;

	assert!(editor.active_document().node_graph_handler.context_menu.is_none(), "a click shouldn't open the node menu");
	assert!(
		editor.active_document().network_interface.upstream_output_connector(&fill_content, &[]).is_none(),
		"a click shouldn't wire anything into Fill's content"
	);
}

#[tokio::test]
async fn leaving_a_subgraph_ends_the_wire_and_closes_the_node_menu_left_open_in_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;

	let (_, empty_export_position) = *editor
		.active_document()
		.network_interface
		.empty_import_export_connectors(false, &path)
		.last()
		.expect("there should be a dimmed connector for adding an export");
	drag_in_graph(&mut editor, &path, empty_export_position, empty_export_position - DVec2::new(240., 0.)).await;
	assert!(
		editor.active_document().node_graph_handler.context_menu.is_some(),
		"dropping the wire on empty space should open the node menu"
	);

	editor.handle_message(DocumentMessage::ExitNestedNetwork { steps_back: 1 }).await;

	let node_graph_handler = &editor.active_document().node_graph_handler;
	assert!(node_graph_handler.context_menu.is_none(), "the node menu should close with the subgraph it was opened in");
	assert!(node_graph_handler.wire_in_progress_from_connector.is_none(), "the wire should end with the subgraph it was dragged in");
}

#[tokio::test]
async fn clicking_a_connected_input_leaves_it_connected_when_a_later_wire_is_dragged_away() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let other_rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 10).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let fill_content = InputConnector::node_at_index(fill, 0);
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(rectangle, 0),
			input_connector: fill_content,
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	let input_position = network_interface.input_position(&fill_content, &[]).expect("Fill's content should have a position");
	let other_output_position = network_interface
		.output_position(&OutputConnector::node(other_rectangle, 0), &[])
		.expect("the other Rectangle's output should have a position");

	// Click Fill's wired content without dragging, then drag a new wire out of the other Rectangle into empty space
	drag_in_graph(&mut editor, &[], input_position, input_position).await;
	drag_in_graph(&mut editor, &[], other_output_position, other_output_position + DVec2::new(240., 240.)).await;

	assert_eq!(
		editor.active_document().network_interface.upstream_output_connector(&fill_content, &[]),
		Some(OutputConnector::node(rectangle, 0)),
		"Fill's content should still be wired from the first Rectangle"
	);
}

/// An ordinary node with the given number of inputs, since a default template is a Passthrough, which draws as a redirect dot.
fn plain_node(inputs: usize) -> NodeTemplate {
	NodeTemplate {
		implementation: NodeTemplateImplementation::Network(NodeNetworkTemplate {
			exports: vec![NodeInput::value(TaggedValue::None, true)],
			..Default::default()
		}),
		inputs: vec![value_input(); inputs],
		output_names: vec![String::new()],
		..Default::default()
	}
}

/// A redirect dot, which a default template with one input is, being a Passthrough.
fn dot_node() -> NodeTemplate {
	NodeTemplate {
		inputs: vec![value_input()],
		..Default::default()
	}
}

/// A plain source feeding a dot feeding a plain consumer, with the dot as the only thing between them.
fn dotted_wire() -> (NodeNetworkInterface, NodeId, NodeId, NodeId) {
	let (source, dot, consumer) = (NodeId(1), NodeId(2), NodeId(3));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(source, plain_node(1), &[]);
	network_interface.insert_node(dot, dot_node(), &[]);
	network_interface.insert_node(consumer, plain_node(1), &[]);
	network_interface.set_input(&InputConnector::node_at_index(dot, 0), NodeInput::node(source, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(consumer, 0), NodeInput::node(dot, 0), &[]);
	assert!(network_interface.is_redirect_dot(&dot, &[]));
	assert!(!network_interface.is_redirect_dot(&source, &[]));
	(network_interface, source, dot, consumer)
}

#[test]
fn a_dot_on_a_wire_is_kept_and_one_that_lost_its_wire_is_removed() {
	let (mut network_interface, source, dot, consumer) = dotted_wire();
	network_interface.remove_orphaned_dots(&[]);
	assert!(network_interface.document_node(&dot, &[]).is_some(), "a dot with a wire through it stays");

	// Taking the wire away from what the dot fed leaves it redirecting nothing
	network_interface.disconnect_input(&InputConnector::node_at_index(consumer, 0), &[]);
	network_interface.remove_orphaned_dots(&[]);
	assert!(network_interface.document_node(&dot, &[]).is_none(), "a dot that nothing takes is removed");
	assert!(network_interface.document_node(&source, &[]).is_some());
	assert_interface_invariants(&network_interface, "after removing an orphaned dot");
}

#[test]
fn a_dot_that_nothing_feeds_is_removed() {
	let (mut network_interface, _, dot, consumer) = dotted_wire();
	network_interface.disconnect_input(&InputConnector::node_at_index(dot, 0), &[]);
	network_interface.remove_orphaned_dots(&[]);
	assert!(network_interface.document_node(&dot, &[]).is_none(), "a dot with nothing feeding it is removed");
	assert!(
		network_interface.upstream_output_connector(&InputConnector::node_at_index(consumer, 0), &[]).is_none(),
		"what the dot fed is left unwired"
	);
	assert_interface_invariants(&network_interface, "after removing a dot that lost its input");
}

/// A dotted wire with the dot also feeding a second consumer.
fn split_dotted_wire() -> (NodeNetworkInterface, NodeId, NodeId, NodeId) {
	let (mut network_interface, source, dot, consumer) = dotted_wire();
	let split_consumer = NodeId(4);
	network_interface.insert_node(split_consumer, plain_node(1), &[]);
	network_interface.set_input(&InputConnector::node_at_index(split_consumer, 0), NodeInput::node(dot, 0), &[]);
	(network_interface, source, dot, consumer)
}

#[test]
fn a_dot_with_a_remaining_consumer_survives_losing_the_other() {
	let (mut network_interface, source, dot, consumer) = split_dotted_wire();

	network_interface.disconnect_input(&InputConnector::node_at_index(consumer, 0), &[]);
	network_interface.remove_orphaned_dots(&[]);
	assert!(network_interface.document_node(&dot, &[]).is_some(), "a split point still taking a wire stays");
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::node_at_index(dot, 0), &[]),
		Some(OutputConnector::node(source, 0))
	);
}

#[test]
fn picking_up_the_last_wire_through_a_dot_draws_it_from_the_source() {
	let (network_interface, source, dot, _) = dotted_wire();
	assert_eq!(network_interface.output_past_sole_dots(OutputConnector::node(dot, 0), &[]), OutputConnector::node(source, 0));
}

#[test]
fn picking_up_a_wire_through_a_shared_dot_draws_it_from_the_dot() {
	let (network_interface, _, dot, _) = split_dotted_wire();
	assert_eq!(
		network_interface.output_past_sole_dots(OutputConnector::node(dot, 0), &[]),
		OutputConnector::node(dot, 0),
		"a dot with another use is where a wire picked up from it is drawn from"
	);
}

#[test]
fn copying_keeps_a_dot_only_when_it_bridges_copied_nodes() {
	let (network_interface, source, dot, consumer) = dotted_wire();

	let mut bridged = network_interface.selection_with_bridging_dots(vec![source, consumer], &[]);
	bridged.sort();
	assert_eq!(bridged, vec![source, dot, consumer], "the dot between two copied nodes comes along");

	assert!(network_interface.selection_with_bridging_dots(vec![dot], &[]).is_empty(), "a dot can't be copied on its own");
	assert_eq!(
		network_interface.selection_with_bridging_dots(vec![source, dot], &[]),
		vec![source],
		"a dot reaching no copied node is left out"
	);
	assert_eq!(
		network_interface.selection_with_bridging_dots(vec![consumer, dot], &[]),
		vec![consumer],
		"a dot fed by no copied node is left out"
	);
}

#[tokio::test]
async fn copying_a_layer_and_a_node_it_feeds_through_a_dot_leaves_the_dot_behind() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	// Layer -> dot -> loose node, where the layer is copied apart from the loose node and so can't keep the wire between them
	editor.draw_rect(0., 0., 100., 100.).await;
	let layer = editor
		.active_document()
		.network_interface
		.upstream_output_connector(&InputConnector::Export(0), &[])
		.and_then(|output| output.node_id())
		.expect("the drawn layer should feed the export");
	let loose = editor.create_node_by_name_at(fill_definition(), 20, 20).await;
	editor
		.active_document_mut()
		.network_interface
		.create_wire(&OutputConnector::primary_output(layer), &InputConnector::primary_input(loose), &[]);
	let dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: dot,
			input_connector: InputConnector::primary_input(loose),
			x: 15,
			y: 20,
		})
		.await;
	assert_eq!(feeding(&editor, loose), Some(OutputConnector::node(dot, 0)), "the dot should feed the loose node: fixture is wrong");

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer, dot, loose] }).await;
	assert_eq!(editor.active_document().partition_selection().other_nodes, vec![loose]);
}

#[test]
fn a_dot_is_never_offered_as_a_layer() {
	let (network_interface, _, dot, _) = dotted_wire();
	assert!(!network_interface.is_eligible_to_be_layer(&dot, &[]));
}

#[test]
fn both_wires_of_a_dot_meet_at_its_center() {
	let (network_interface, _, dot, _) = dotted_wire();
	let center = network_interface
		.input_position(&InputConnector::node_at_index(dot, 0), &[])
		.expect("the dot's input should have a position");
	assert_eq!(
		network_interface.output_position(&OutputConnector::node(dot, 0), &[]),
		Some(center),
		"both wires meet at the dot's center"
	);
}

#[test]
fn a_dot_in_a_layers_chain_is_left_out_of_what_is_listed_for_it() {
	let (layer, upper, dot, lower) = (NodeId(1), NodeId(2), NodeId(3), NodeId(4));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(layer, plain_node(2), &[]);
	network_interface.set_to_node_or_layer(&layer, &[], true);
	network_interface.insert_node(upper, plain_node(1), &[]);
	network_interface.insert_node(dot, dot_node(), &[]);
	network_interface.insert_node(lower, plain_node(1), &[]);
	network_interface.set_input(&InputConnector::node_at_index(layer, 1), NodeInput::node(upper, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(upper, 0), NodeInput::node(dot, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(dot, 0), NodeInput::node(lower, 0), &[]);
	assert!(network_interface.is_redirect_dot(&dot, &[]));

	assert_eq!(
		network_interface.layer_chain_nodes(layer, &[]),
		vec![upper, lower],
		"the Layers panel lists the nodes on either side of the dot"
	);
	// The node at the upstream end has no exposed primary input, so it is a pinned source rather than reorderable
	assert_eq!(network_interface.reorderable_chain(layer, &[]), vec![upper]);

	let (inspector_chain, stack_layer) = crate::messages::portfolio::document::node_graph::node_inspector::input_chain(layer, 1, &network_interface, &[]).expect("a node feeds the layer");
	assert_eq!(inspector_chain, vec![upper, lower], "the Inspector shows a section for each node but not for the dot");
	assert_eq!(stack_layer, None);
}

#[test]
fn a_node_dragged_over_exactly_one_wire_previews_being_inserted_onto_it() {
	use crate::messages::portfolio::document::node_graph::NodeGraphMessageHandler;

	let (source, consumer, dragged) = (NodeId(1), NodeId(2), NodeId(3));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(source, plain_node(1), &[]);
	network_interface.insert_node(consumer, plain_node(1), &[]);
	let mut dragged_template = plain_node(1);
	dragged_template.inputs = vec![NodeInput::value(TaggedValue::None, true)];
	network_interface.insert_node(dragged, dragged_template, &[]);
	network_interface.set_absolute_position(&source, IVec2::new(0, 0), &[]);
	network_interface.set_absolute_position(&consumer, IVec2::new(20, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(consumer, 0), NodeInput::node(source, 0), &[]);
	network_interface
		.selected_nodes_mut(&[])
		.expect("the document network has a selection")
		.set_selected_nodes(vec![dragged]);

	// Level with the wire between the other two
	network_interface.set_absolute_position(&dragged, IVec2::new(8, 0), &[]);
	assert_eq!(
		NodeGraphMessageHandler::wire_to_insert_on(&mut network_interface, GraphWireStyle::Direct, &[]),
		Some((dragged, 0, InputConnector::node_at_index(consumer, 0))),
		"the wire it overlaps is the one a drop would insert it onto"
	);

	network_interface.set_absolute_position(&dragged, IVec2::new(8, 10), &[]);
	assert_eq!(
		NodeGraphMessageHandler::wire_to_insert_on(&mut network_interface, GraphWireStyle::Direct, &[]),
		None,
		"clear of every wire it previews nothing"
	);

	// With a node already taking its output, a drop would not insert it
	network_interface.set_absolute_position(&dragged, IVec2::new(8, 0), &[]);
	network_interface.set_input(&InputConnector::node_at_index(source, 0), NodeInput::node(dragged, 0), &[]);
	assert_eq!(NodeGraphMessageHandler::wire_to_insert_on(&mut network_interface, GraphWireStyle::Direct, &[]), None);
}

fn passthrough_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER)
}

/// Wires each node into the next one's first exposed input, making a run along the flow.
fn wire_run(editor: &mut EditorTestUtils, nodes: &[NodeId]) {
	let network_interface = &mut editor.active_document_mut().network_interface;
	for pair in nodes.windows(2) {
		let input = network_interface.first_exposed_input(&pair[1], &[]).expect("each node after the first should take a wire");
		network_interface.create_wire(&OutputConnector::primary_output(pair[0]), &input, &[]);
	}
}

/// What feeds the node's first exposed input.
fn feeding(editor: &EditorTestUtils, node_id: NodeId) -> Option<OutputConnector> {
	let network_interface = &editor.active_document().network_interface;
	let input = network_interface.first_exposed_input(&node_id, &[])?;
	network_interface.upstream_output_connector(&input, &[])
}

/// Whether the node is in the document network.
fn exists(editor: &EditorTestUtils, node_id: NodeId) -> bool {
	exists_in(editor, node_id, &[])
}

/// Whether the node is in the network at `network_path`, asked without logging an error when it isn't.
fn exists_in(editor: &EditorTestUtils, node_id: NodeId, network_path: &[NodeId]) -> bool {
	editor
		.active_document()
		.network_interface
		.nested_network(network_path)
		.is_some_and(|network| network.nodes.contains_key(&node_id))
}

/// Selects the nodes, then presses on the first to drag them all, shakes them loose, and releases them far below, clear of every wire.
async fn shake_nodes(editor: &mut EditorTestUtils, nodes: Vec<NodeId>) {
	shake_nodes_in(editor, &[], nodes).await;
}

/// Shakes the nodes loose like [`shake_nodes`], in the network at `network_path`.
async fn shake_nodes_in(editor: &mut EditorTestUtils, network_path: &[NodeId], nodes: Vec<NodeId>) {
	let Some(&pressed) = nodes.first() else { return };
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes }).await;

	// TODO: Remove once aborting a transaction (which shaking does) keeps the edits still uncommitted from before it began, instead of dropping them from the stored history
	editor.handle_message(DocumentMessage::AddTransaction).await;

	let network_interface = &editor.active_document().network_interface;
	let bounds = network_interface.node_bounding_box(&pressed, network_path).expect("the node should have bounds");
	let node_graph_to_viewport = network_interface
		.network_metadata(network_path)
		.expect("the network should have metadata")
		.persistent_metadata
		.navigation_metadata
		.node_graph_to_viewport;
	let center = node_graph_to_viewport.transform_point2((bounds[0] + bounds[1]) / 2.);
	let release = center + node_graph_to_viewport.transform_vector2(DVec2::new(0., 1000.));

	editor.move_mouse(center.x, center.y, ModifierKeys::empty(), MouseKeys::empty()).await;
	editor
		.handle_message(NodeGraphMessage::PointerDown {
			shift_click: false,
			control_click: false,
			alt_click: false,
			right_click: false,
		})
		.await;
	editor.move_mouse(center.x + 30., center.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
	editor.handle_message(NodeGraphMessage::PointerMove { shift: Key::Shift }).await;
	editor.handle_message(NodeGraphMessage::ShakeNode).await;
	editor.move_mouse(release.x, release.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
	editor.handle_message(NodeGraphMessage::PointerMove { shift: Key::Shift }).await;
	editor.handle_message(NodeGraphMessage::PointerUp).await;
}

#[tokio::test]
async fn shaking_a_node_out_of_a_wire_through_a_dot_keeps_the_dot_and_the_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 10, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 20, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 30, 0).await;
	wire_run(&mut editor, &[rectangle, fill, dot, consumer]);
	assert!(
		editor.active_document().network_interface.is_redirect_dot(&dot, &[]),
		"the Passthrough should be a redirect dot: fixture is wrong"
	);

	shake_nodes(&mut editor, vec![fill]).await;

	assert!(exists(&editor, dot), "the dot is part of the wire, so shaking a node that fed it leaves it in place");
	assert_eq!(
		feeding(&editor, dot),
		Some(OutputConnector::node(rectangle, 0)),
		"the dot should now be fed by what fed the shaken node"
	);
	assert_eq!(feeding(&editor, consumer), Some(OutputConnector::node(dot, 0)), "the dot should still feed what it fed");
	assert_eq!(feeding(&editor, fill), None, "the shaken node should be left unwired");
}

#[tokio::test]
async fn shaking_the_node_feeding_a_dot_into_a_layer_keeps_the_dot_and_the_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	// Something -> Node 1 -> Node 2 -> dot -> layer, with the dot inserted on the wire as double-clicking it does
	let something = editor.create_node_by_name_at(rectangle_definition(), 2, 10).await;
	let first = editor.create_node_by_name_at(fill_definition(), 12, 10).await;
	let second = editor.create_node_by_name_at(fill_definition(), 22, 10).await;
	let layer = editor.create_node_by_name_at(merge_definition(), 45, 10).await;
	editor.active_document_mut().network_interface.set_to_node_or_layer(&layer, &[], true);
	wire_run(&mut editor, &[something, first, second]);
	editor
		.active_document_mut()
		.network_interface
		.create_wire(&OutputConnector::primary_output(second), &InputConnector::layer_secondary_input(layer), &[]);
	let dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: dot,
			input_connector: InputConnector::layer_secondary_input(layer),
			x: 38,
			y: 11,
		})
		.await;
	assert_eq!(
		editor.active_document().network_interface.upstream_output_connector(&InputConnector::layer_secondary_input(layer), &[]),
		Some(OutputConnector::node(dot, 0)),
		"the dot should feed the layer: fixture is wrong"
	);

	shake_nodes(&mut editor, vec![second]).await;

	assert!(exists(&editor, dot), "the dot should stay on the wire");
	assert_eq!(feeding(&editor, dot), Some(OutputConnector::node(first, 0)), "Node 1 should now feed the dot");
	assert_eq!(
		editor.active_document().network_interface.upstream_output_connector(&InputConnector::layer_secondary_input(layer), &[]),
		Some(OutputConnector::node(dot, 0)),
		"the dot should still feed the layer"
	);
	assert_eq!(feeding(&editor, second), None, "Node 2 should be left unwired");
}

#[tokio::test]
async fn shaking_a_node_spliced_in_by_a_secondary_input_keeps_the_dot_on_its_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	// Numbers to Vec2 hides its primary input, so it takes the flow by its first exposed input instead
	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let shaken = editor
		.create_node_by_name_at(DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::numbers_to_vec_2::IDENTIFIER), 10, 0)
		.await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 20, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 30, 0).await;
	wire_run(&mut editor, &[source, shaken, dot, consumer]);
	assert_ne!(
		editor.active_document().network_interface.first_exposed_input(&shaken, &[]),
		Some(InputConnector::primary_input(shaken)),
		"the shaken node's primary input should be hidden: fixture is wrong"
	);

	shake_nodes(&mut editor, vec![shaken]).await;

	assert!(exists(&editor, dot));
	assert_eq!(feeding(&editor, dot), Some(OutputConnector::node(source, 0)), "what fed the shaken node should take over its wire");
	assert_eq!(feeding(&editor, consumer), Some(OutputConnector::node(dot, 0)));
}

#[tokio::test]
async fn shaking_a_generator_feeding_a_dot_takes_the_dot_with_its_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let generator = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 10, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	wire_run(&mut editor, &[generator, dot, consumer]);

	shake_nodes(&mut editor, vec![generator]).await;

	assert!(!exists(&editor, dot), "with nothing left to feed it, the dot goes along with its wire");
	assert_eq!(feeding(&editor, consumer), None);
}

#[tokio::test]
async fn shaking_nodes_on_either_side_of_a_dot_keeps_the_wire_between_them() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let upper = editor.create_node_by_name_at(fill_definition(), 10, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 20, 0).await;
	let lower = editor.create_node_by_name_at(fill_definition(), 30, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 40, 0).await;
	wire_run(&mut editor, &[source, upper, dot, lower, consumer]);

	shake_nodes(&mut editor, vec![upper, lower]).await;

	assert_eq!(
		feeding(&editor, consumer),
		Some(OutputConnector::node(source, 0)),
		"the flow should close the gap the shaken nodes leave"
	);
	assert_eq!(
		feeding(&editor, dot),
		Some(OutputConnector::node(upper, 0)),
		"the wire between the shaken nodes should come along whole"
	);
	assert_eq!(feeding(&editor, lower), Some(OutputConnector::node(dot, 0)));
}

#[tokio::test]
async fn shaking_a_node_along_with_its_dot_still_hands_the_dot_its_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let shaken = editor.create_node_by_name_at(fill_definition(), 10, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 20, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 30, 0).await;
	wire_run(&mut editor, &[source, shaken, dot, consumer]);

	shake_nodes(&mut editor, vec![shaken, dot]).await;

	assert_eq!(
		feeding(&editor, dot),
		Some(OutputConnector::node(source, 0)),
		"a dot bends only its own wire, so it stays on it even when selected"
	);
	assert_eq!(feeding(&editor, consumer), Some(OutputConnector::node(dot, 0)));
	assert_eq!(feeding(&editor, shaken), None);
}

#[tokio::test]
async fn hiding_an_input_fed_through_a_dot_takes_the_dot_with_its_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 10, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	wire_run(&mut editor, &[source, dot]);
	let hidden_input = InputConnector::node_at_index(consumer, 1);
	editor.active_document_mut().network_interface.create_wire(&OutputConnector::primary_output(dot), &hidden_input, &[]);

	editor
		.handle_message(NodeGraphMessage::ExposeInput {
			input_connector: hidden_input,
			set_to_exposed: false,
			start_transaction: true,
		})
		.await;

	assert!(!exists(&editor, dot), "the dot should go with the wire the hidden input was fed by");
	assert!(editor.active_document().network_interface.upstream_output_connector(&hidden_input, &[]).is_none());
}

#[tokio::test]
async fn dropping_a_wire_back_on_the_input_it_was_picked_up_from_keeps_its_dot() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 10, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	wire_run(&mut editor, &[source, dot, consumer]);

	let input_position = editor
		.active_document()
		.network_interface
		.input_position(&InputConnector::primary_input(consumer), &[])
		.expect("the input should have a position");
	drag_in_graph(&mut editor, &[], input_position, input_position + DVec2::new(1., 0.)).await;

	assert!(exists(&editor, dot), "a wire put back where it was should keep its dot");
	assert_eq!(feeding(&editor, consumer), Some(OutputConnector::node(dot, 0)));
}

#[test]
fn a_passthrough_holding_a_value_is_removed_only_once_nothing_takes_it() {
	// A Passthrough holding a value rather than a wire, as a migration leaves in place of a removed node
	let (value_holder, consumer, other_consumer, unrelated) = (NodeId(1), NodeId(2), NodeId(3), NodeId(4));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(value_holder, dot_node(), &[]);
	for node_id in [consumer, other_consumer, unrelated] {
		network_interface.insert_node(node_id, plain_node(1), &[]);
	}
	for node_id in [consumer, other_consumer] {
		network_interface.set_input(&InputConnector::node_at_index(node_id, 0), NodeInput::node(value_holder, 0), &[]);
	}
	assert!(network_interface.is_redirect_dot(&value_holder, &[]));

	network_interface.delete_nodes(vec![unrelated], false, &[]);
	assert!(network_interface.document_node(&value_holder, &[]).is_some(), "an edit elsewhere should leave it alone");

	network_interface.disconnect_input(&InputConnector::node_at_index(other_consumer, 0), &[]);
	network_interface.remove_orphaned_dots(&[]);
	assert!(network_interface.document_node(&value_holder, &[]).is_some(), "it should stay while it still feeds a node");
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::node_at_index(consumer, 0), &[]),
		Some(OutputConnector::node(value_holder, 0))
	);

	network_interface.disconnect_input(&InputConnector::node_at_index(consumer, 0), &[]);
	network_interface.remove_orphaned_dots(&[]);
	assert!(network_interface.document_node(&value_holder, &[]).is_none(), "with nothing taking its output, it bends no wire");
	assert_interface_invariants(&network_interface, "after removing a dot holding a value that nothing takes");
}

#[tokio::test]
async fn reordering_a_chain_around_a_dot_takes_the_dot_with_its_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (_, near, far, _, _) = layer_with_chain(&mut editor).await;
	let dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: dot,
			input_connector: InputConnector::primary_input(near),
			x: 21,
			y: 11,
		})
		.await;
	assert_eq!(feeding(&editor, near), Some(OutputConnector::node(dot, 0)), "the dot should feed the near node: fixture is wrong");

	editor.active_document_mut().network_interface.reorder_chain_nodes(&[far], 0, &[]);

	assert!(!exists(&editor, dot), "the wire the dot bent is gone once its ends trade places");
	assert_eq!(feeding(&editor, far), Some(OutputConnector::node(near, 0)), "the chain should be reordered");
	assert_invariants(&editor, "after reordering a chain around a dot");
}

#[tokio::test]
async fn grouping_nodes_on_either_side_of_a_dot_takes_the_dot_inside() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let upper = editor.create_node_by_name_at(fill_definition(), 10, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 20, 0).await;
	let lower = editor.create_node_by_name_at(fill_definition(), 30, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 40, 0).await;
	wire_run(&mut editor, &[source, upper, dot, lower, consumer]);

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![upper, lower] }).await;
	editor.handle_message(NodeGraphMessage::MergeSelectedNodes).await;

	assert!(
		!exists(&editor, upper) && !exists(&editor, lower),
		"the unselected dot between them shouldn't stop the nodes from being grouped"
	);
	assert!(!exists(&editor, dot), "the dot should go inside with the wire it bends");
	let group = feeding(&editor, consumer).and_then(|output| output.node_id()).expect("the group should feed the consumer");
	assert_eq!(feeding(&editor, group), Some(OutputConnector::node(source, 0)));
}

#[tokio::test]
async fn duplicating_nodes_on_either_side_of_a_dot_in_their_flow_adds_no_dot() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let upper = editor.create_node_by_name_at(fill_definition(), 10, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 20, 0).await;
	let lower = editor.create_node_by_name_at(fill_definition(), 30, 0).await;
	wire_run(&mut editor, &[source, upper, dot, lower]);

	let copies = duplicate_selection(&mut editor, vec![upper, lower]).await;
	assert_eq!(copies.len(), 2, "each node should get a copy in the flow, and the dot none");

	let network_interface = &editor.active_document().network_interface;
	let dot_count = network_interface
		.document_network()
		.nodes
		.keys()
		.filter(|node_id| network_interface.is_redirect_dot(node_id, &[]))
		.count();
	assert_eq!(dot_count, 1, "the copies are spliced into the flow, where the one dot already bends the wire");

	let upper_copy = feeding(&editor, dot).and_then(|output| output.node_id()).expect("the dot should still be fed");
	assert!(copies.contains(&upper_copy), "the upper node's copy should be spliced in right after it, ahead of the dot");
	assert_eq!(feeding(&editor, upper_copy), Some(OutputConnector::node(upper, 0)));
}

/// Clicks at `from` and presses there again, then drags that second press of a double-click to `to`, both in the graph space of the network at `path`.
async fn double_press_drag_in_graph(editor: &mut EditorTestUtils, path: &[NodeId], from: DVec2, to: DVec2) {
	drag_in_graph(editor, path, from, from).await;
	drag_in_graph(editor, path, from, to).await;
}

/// Clicks twice at a point in the graph space of the document network, with the second press dragged by `second_press_drag`,
/// then double-clicks there, as the double-click arrives after the second release.
async fn double_click_in_graph(editor: &mut EditorTestUtils, point: DVec2, second_press_drag: DVec2) {
	double_press_drag_in_graph(editor, &[], point, point + second_press_drag).await;
	editor.handle_message(NodeGraphMessage::EnterNestedNetwork).await;
}

/// The middle of the longest segment of the wire feeding the input, in document network graph space.
fn wire_middle(network_interface: &NodeNetworkInterface, input: InputConnector) -> DVec2 {
	use kurbo::ParamCurveArclen;

	let (wire, _, _) = network_interface.vector_wire_from_input(&input, GraphWireStyle::Direct, &[]).expect("the wire should have a path");
	let longest_segment = wire
		.segments()
		.max_by(|a, b| a.arclen(kurbo::DEFAULT_ACCURACY).total_cmp(&b.arclen(kurbo::DEFAULT_ACCURACY)))
		.expect("the wire should have a segment");
	let middle = kurbo::ParamCurve::eval(&longest_segment, 0.5);
	DVec2::new(middle.x, middle.y)
}

/// Double-clicks the middle of the wire feeding the input.
async fn double_click_wire(editor: &mut EditorTestUtils, input: InputConnector) {
	let middle = wire_middle(&editor.active_document().network_interface, input);
	double_click_in_graph(editor, middle, DVec2::ZERO).await;
}

/// The grid position of a redirect dot centered on the grid point nearest to the point, as its center sits a grid row below its position.
fn dot_position_centered_near(point: DVec2) -> IVec2 {
	(point / GRID_SIZE as f64).round().as_ivec2() - IVec2::Y
}

#[tokio::test]
async fn double_clicking_a_wire_adds_a_dot_even_after_a_node_was_dragged() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let elsewhere = editor.create_node_by_name_at(fill_definition(), 0, 20).await;
	wire_run(&mut editor, &[source, consumer]);

	// Drag a node unrelated to the wire, which ends the press that moved it
	let bounds = editor.active_document().network_interface.node_bounding_box(&elsewhere, &[]).expect("the node should have bounds");
	let center = (bounds[0] + bounds[1]) / 2.;
	drag_in_graph(&mut editor, &[], center, center + DVec2::new(72., 0.)).await;

	double_click_wire(&mut editor, InputConnector::primary_input(consumer)).await;

	let dot = feeding(&editor, consumer).and_then(|output| output.node_id()).expect("the consumer should be fed");
	assert!(
		editor.active_document().network_interface.is_redirect_dot(&dot, &[]),
		"double-clicking the wire should add a dot to it, however long ago a node was dragged"
	);
	assert_eq!(feeding(&editor, dot), Some(OutputConnector::node(source, 0)));
}

#[tokio::test]
async fn a_dot_over_a_node_is_pressed_rather_than_the_node_beneath() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let beneath = editor.create_node_by_name_at(fill_definition(), 10, 4).await;
	wire_run(&mut editor, &[source, consumer]);

	// Centered over the middle of the node beneath
	let dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: dot,
			input_connector: InputConnector::primary_input(consumer),
			x: 12,
			y: 5,
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	let dot_bounds = network_interface.node_bounding_box(&dot, &[]).expect("the dot should have bounds");
	let dot_center = (dot_bounds[0] + dot_bounds[1]) / 2.;
	let beneath_bounds = network_interface.node_bounding_box(&beneath, &[]).expect("the node should have bounds");
	assert!(
		beneath_bounds[0].cmple(dot_center).all() && dot_center.cmple(beneath_bounds[1]).all(),
		"the dot should sit over the node: fixture is wrong"
	);

	let node_graph_to_viewport = network_interface
		.network_metadata(&[])
		.expect("the network should have metadata")
		.persistent_metadata
		.navigation_metadata
		.node_graph_to_viewport;
	assert_eq!(network_interface.node_from_click(node_graph_to_viewport.transform_point2(dot_center), &[]), Some(dot));
}

#[tokio::test]
async fn double_clicking_a_subgraph_enters_it_unless_that_press_dragged_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	// In its node form, so the double-click lands on the body rather than a layer's name
	let subgraph = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.active_document_mut().network_interface.set_to_node_or_layer(&subgraph, &[], false);

	let double_click_node = async |editor: &mut EditorTestUtils, second_press_drag: DVec2| {
		let bounds = editor.active_document().network_interface.node_bounding_box(&subgraph, &[]).expect("the node should have bounds");
		double_click_in_graph(editor, (bounds[0] + bounds[1]) / 2., second_press_drag).await;
	};

	double_click_node(&mut editor, DVec2::new(72., 0.)).await;
	assert!(
		editor.active_document().selection_network_path().is_empty(),
		"a double-click with a press that moved the node shouldn't enter it"
	);

	double_click_node(&mut editor, DVec2::new(2., 0.)).await;
	assert_eq!(
		editor.active_document().selection_network_path(),
		&[subgraph],
		"a double-click that jitters without moving the node should enter it"
	);
}

#[tokio::test]
async fn double_press_dragging_a_wire_drags_the_dot_it_adds() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	wire_run(&mut editor, &[source, consumer]);

	let middle = wire_middle(&editor.active_document().network_interface, InputConnector::primary_input(consumer));
	double_press_drag_in_graph(&mut editor, &[], middle, middle + DVec2::new(72., 48.)).await;

	let dot = feeding(&editor, consumer).and_then(|output| output.node_id()).expect("the consumer should be fed");
	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.is_redirect_dot(&dot, &[]), "the second press should split the wire with a dot");
	assert_eq!(feeding(&editor, dot), Some(OutputConnector::node(source, 0)));
	assert_eq!(
		network_interface.position(&dot, &[]),
		Some(dot_position_centered_near(middle) + IVec2::new(3, 2)),
		"the press should go on to drag the dot it added"
	);

	assert_invariants(&editor, "after dragging a new dot off a wire");
}

#[tokio::test]
async fn dragging_a_dot_out_of_an_output_fans_its_wires_out_from_the_dot() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let upper = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let lower = editor.create_node_by_name_at(fill_definition(), 20, 10).await;
	let source_output = OutputConnector::node(source, 0);
	for consumer in [upper, lower] {
		editor
			.active_document_mut()
			.network_interface
			.create_wire(&source_output, &InputConnector::primary_input(consumer), &[]);
	}

	let output_position = editor
		.active_document()
		.network_interface
		.output_position(&source_output, &[])
		.expect("the output should have a position");
	double_press_drag_in_graph(&mut editor, &[], output_position, output_position + DVec2::new(96., 0.)).await;

	let [InputConnector::Node { node_id: dot, input_index: 0 }] = fed_inputs(&editor, &source_output, &[]).into_iter().collect::<Vec<_>>()[..] else {
		panic!("the output should feed only the dot's input");
	};
	let network_interface = &editor.active_document().network_interface;
	assert!(network_interface.is_redirect_dot(&dot, &[]));
	assert_eq!(
		fed_inputs(&editor, &OutputConnector::primary_output(dot), &[]),
		HashSet::from([InputConnector::primary_input(upper), InputConnector::primary_input(lower)]),
		"the wires the output fed should fan out from the dot"
	);
	assert_eq!(network_interface.position(&dot, &[]), Some(dot_position_centered_near(output_position) + IVec2::new(4, 0)));

	assert_invariants(&editor, "after dragging a dot out of an output");
}

#[tokio::test]
async fn dragging_a_dot_out_of_an_export_runs_its_wire_through_the_dot() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let path = vec![merge];
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;

	let export = InputConnector::Export(0);
	let network_interface = &editor.active_document().network_interface;
	let upstream = network_interface.upstream_output_connector(&export, &path).expect("Merge's export should be fed by a node inside it");
	let export_position = network_interface.input_position(&export, &path).expect("the export should have a position");
	double_press_drag_in_graph(&mut editor, &path, export_position, export_position - DVec2::new(72., 0.)).await;

	let network_interface = &editor.active_document().network_interface;
	let dot = network_interface
		.upstream_output_connector(&export, &path)
		.and_then(|output| output.node_id())
		.expect("the export should be fed by a node");
	assert!(network_interface.is_redirect_dot(&dot, &path), "a dot should be dragged out of the export");
	assert_eq!(network_interface.upstream_output_connector(&InputConnector::primary_input(dot), &path), Some(upstream));

	assert_invariants(&editor, "after dragging a dot out of an export");
}

#[tokio::test]
async fn double_clicking_a_connector_without_dragging_leaves_its_wires_alone() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	wire_run(&mut editor, &[source, consumer]);
	let node_count = editor.active_document().network_interface.document_network().nodes.len();

	let output_position = editor
		.active_document()
		.network_interface
		.output_position(&OutputConnector::node(source, 0), &[])
		.expect("the output should have a position");
	double_click_in_graph(&mut editor, output_position, DVec2::ZERO).await;

	assert_eq!(editor.active_document().network_interface.document_network().nodes.len(), node_count, "no dot should be added");
	assert_eq!(feeding(&editor, consumer), Some(OutputConnector::node(source, 0)));
}

#[tokio::test]
async fn double_clicking_a_dot_dissolves_it_into_the_wires_it_bent() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 12, 0).await;
	let upper = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let lower = editor.create_node_by_name_at(fill_definition(), 20, 10).await;
	wire_run(&mut editor, &[source, dot, upper]);
	editor
		.active_document_mut()
		.network_interface
		.create_wire(&OutputConnector::primary_output(dot), &InputConnector::primary_input(lower), &[]);

	let bounds = editor.active_document().network_interface.node_bounding_box(&dot, &[]).expect("the dot should have bounds");
	double_click_in_graph(&mut editor, (bounds[0] + bounds[1]) / 2., DVec2::ZERO).await;

	assert!(!exists(&editor, dot), "double-clicking the dot should remove it");
	assert_eq!(
		fed_inputs(&editor, &OutputConnector::node(source, 0), &[]),
		HashSet::from([InputConnector::primary_input(upper), InputConnector::primary_input(lower)]),
		"the wires the dot bent should run straight from what fed it"
	);

	assert_invariants(&editor, "after double-clicking a dot");
}

#[tokio::test]
async fn imports_and_exports_keep_just_a_grid_cell_from_a_dot() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let subgraph = editor.create_node_by_name_at(DefinitionIdentifier::Network("Custom Node".to_string()), 0, 0).await;
	let path = [subgraph];
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: subgraph }).await;

	// The grid columns of the imports and exports
	let import_export_columns = |editor: &EditorTestUtils| {
		let (import, export) = editor
			.active_document()
			.network_interface
			.import_export_position(&path)
			.expect("the imports and exports should have positions");
		(import.x / GRID_SIZE as i32, export.x / GRID_SIZE as i32)
	};
	let (import_column, export_column) = import_export_columns(&editor);

	let beside_import = editor.create_node_by_name_at(passthrough_definition(), import_column + 1, 0).await;
	let beside_export = editor.create_node_by_name_at(passthrough_definition(), export_column - 1, 0).await;
	assert_eq!(
		import_export_columns(&editor),
		(import_column, export_column),
		"dots a grid cell away shouldn't push the imports or exports"
	);

	editor.active_document_mut().network_interface.delete_nodes(vec![beside_import, beside_export], false, &path);
	editor.create_node_by_name_at(passthrough_definition(), import_column, 0).await;
	editor.create_node_by_name_at(passthrough_definition(), export_column, 0).await;
	assert_eq!(
		import_export_columns(&editor),
		(import_column - 1, export_column + 1),
		"dots right at the imports and exports should push them a grid cell away"
	);
}

#[tokio::test]
async fn previewing_keeps_the_dot_it_will_restore_the_export_to_and_outlasts_removing_other_dots() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let root = editor.create_node_by_name_at(fill_definition(), 0, 0).await;
	let previewed = editor.create_node_by_name_at(fill_definition(), 0, 10).await;
	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 20).await;
	let other_dot = editor.create_node_by_name_at(passthrough_definition(), 10, 20).await;
	let consumer = editor.create_node_by_name_at(fill_definition(), 20, 20).await;
	wire_run(&mut editor, &[source, other_dot, consumer]);
	editor
		.active_document_mut()
		.network_interface
		.create_wire(&OutputConnector::primary_output(root), &InputConnector::Export(0), &[]);
	let export_dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: export_dot,
			input_connector: InputConnector::Export(0),
			x: 20,
			y: 1,
		})
		.await;

	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: previewed }).await;
	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.disconnect_input(&InputConnector::primary_input(consumer), &[]);
	network_interface.remove_orphaned_dots(&[]);

	assert!(!exists(&editor, other_dot), "the dot left with nothing to feed should be removed");
	assert!(exists(&editor, export_dot), "the dot the preview will restore the export to should stay");
	assert!(
		matches!(editor.active_document().network_interface.previewing(&[]), Previewing::Yes { .. }),
		"removing a dot outside the preview should leave it running"
	);

	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: previewed }).await;
	assert_eq!(
		editor.active_document().network_interface.upstream_output_connector(&InputConnector::Export(0), &[]),
		Some(OutputConnector::node(export_dot, 0)),
		"ending the preview should restore the export to the dot"
	);
	assert_invariants(&editor, "after previewing past a dot on the export's wire");
}

#[tokio::test]
async fn removing_an_import_removes_the_dot_that_fed_its_input_on_the_subgraph_node() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let merge = editor.create_node_by_name_at(merge_definition(), 20, 0).await;
	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let dot = editor.create_node_by_name_at(passthrough_definition(), 10, 0).await;
	wire_run(&mut editor, &[source, dot]);
	editor
		.active_document_mut()
		.network_interface
		.create_wire(&OutputConnector::primary_output(dot), &InputConnector::node_at_index(merge, 1), &[]);

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(NodeGraphMessage::RemoveImport { import_index: 1 }).await;

	assert!(!exists(&editor, dot), "the dot fed only the removed input, so it should go with it");
	assert_invariants(&editor, "after removing an import fed through a dot");
}

#[tokio::test]
async fn a_new_layer_goes_beneath_a_dot_on_the_wire_into_the_export() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	editor.draw_rect(0., 0., 100., 100.).await;
	let top_layer = editor
		.active_document()
		.network_interface
		.upstream_output_connector(&InputConnector::Export(0), &[])
		.and_then(|output| output.node_id())
		.expect("the drawn layer should feed the export");
	assert!(
		editor.active_document().network_interface.can_bend_wire_into(&InputConnector::Export(0), &[]),
		"the wire from the top layer into the export should take a dot"
	);

	let dot = NodeId::new();
	editor
		.handle_message(NodeGraphMessage::InsertDotOnWire {
			node_id: dot,
			input_connector: InputConnector::Export(0),
			x: 20,
			y: -3,
		})
		.await;
	editor.draw_rect(200., 0., 300., 100.).await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::Export(0), &[]),
		Some(OutputConnector::node(dot, 0)),
		"the dot should stay on the wire into the export"
	);
	let new_layer = feeding(&editor, dot).and_then(|output| output.node_id()).expect("the dot should still be fed");
	assert!(new_layer != top_layer && network_interface.is_layer(&new_layer, &[]), "the new layer should go beneath the dot");
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::primary_input(new_layer), &[]),
		Some(OutputConnector::node(top_layer, 0)),
		"the new layer should stack straight onto the old top layer, with no dot on the stack wire between them"
	);
	assert_eq!(LayerNodeIdentifier::ROOT_PARENT.children(network_interface.document_metadata()).count(), 2);

	assert_invariants(&editor, "after adding a layer beneath a dot on the export's wire");
}

#[tokio::test]
async fn chain_nodes_pasted_loose_keep_the_wire_and_dot_between_them() {
	for select_the_dot in [false, true] {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (_, near, far, _, _) = layer_with_chain(&mut editor).await;
		let dot = NodeId::new();
		editor
			.handle_message(NodeGraphMessage::InsertDotOnWire {
				node_id: dot,
				input_connector: InputConnector::primary_input(near),
				x: 20,
				y: 11,
			})
			.await;

		let selection = if select_the_dot { vec![far, dot, near] } else { vec![far, near] };
		let clipboard = copy_in_graph(&mut editor, selection).await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() }).await;
		let pasted = paste(&mut editor, clipboard).await;

		let network_interface = &editor.active_document().network_interface;
		assert_eq!(pasted.len(), 3, "both nodes and the dot between them should be pasted");
		let dot_copy = *pasted.iter().find(|node_id| network_interface.is_redirect_dot(node_id, &[])).expect("the dot should come along");
		let far_copy = feeding(&editor, dot_copy).and_then(|output| output.node_id()).expect("the copied dot should be fed");
		let near_copy = *pasted
			.iter()
			.find(|node_id| feeding(&editor, **node_id) == Some(OutputConnector::node(dot_copy, 0)))
			.expect("the copied dot should feed a copy");
		assert!(pasted.contains(&far_copy) && far_copy != far, "the copied dot should be fed by the far node's copy");
		assert_ne!(near_copy, near);

		assert_invariants(&editor, "after pasting chain nodes loose");
	}
}

#[tokio::test]
async fn a_press_right_after_dragging_a_wire_out_of_an_output_drags_another_wire() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let source = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let upper = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let lower = editor.create_node_by_name_at(fill_definition(), 20, 10).await;
	wire_run(&mut editor, &[source, upper]);

	let network_interface = &editor.active_document().network_interface;
	let output_position = network_interface.output_position(&OutputConnector::node(source, 0), &[]).expect("the output should have a position");
	let lower_input_position = network_interface.input_position(&InputConnector::primary_input(lower), &[]).expect("the input should have a position");

	// The first press drags a wire away to empty space, so it isn't a click that a second press could turn into a double-click
	drag_in_graph(&mut editor, &[], output_position, output_position + DVec2::new(0., 480.)).await;
	drag_in_graph(&mut editor, &[], output_position, lower_input_position).await;

	assert_eq!(feeding(&editor, lower), Some(OutputConnector::node(source, 0)), "the second press should drag a wire, not a dot");
	assert_eq!(feeding(&editor, upper), Some(OutputConnector::node(source, 0)));
}

/// Opens a new subgraph with one node inside, which is all that lies between its import and its export.
async fn subgraph_holding_one_node(editor: &mut EditorTestUtils) -> (NodeId, NodeId) {
	// TODO: Remove once staging the recorded edits stores a node wired to an import inside a subgraph as it is, instead of drifting from it
	editor.handle_message(PreferencesMessage::ValidateStorageRoundTrip { enabled: false }).await;

	let subgraph = editor.create_node_by_name_at(DefinitionIdentifier::Network("Custom Node".to_string()), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: subgraph }).await;
	editor.handle_message(NodeGraphMessage::AddImport).await;
	editor.handle_message(NodeGraphMessage::AddPrimaryExport).await;

	let node = editor.create_node_by_name_at(fill_definition(), 10, 0).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::Import(0),
			input_connector: InputConnector::primary_input(node),
		})
		.await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(node),
			input_connector: InputConnector::Export(0),
		})
		.await;

	let network_interface = &editor.active_document().network_interface;
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::Export(0), &[subgraph]),
		Some(OutputConnector::primary_output(node)),
		"the node should feed the subgraph's export: fixture is wrong"
	);
	assert_eq!(
		network_interface.upstream_output_connector(&InputConnector::primary_input(node), &[subgraph]),
		Some(OutputConnector::Import(0)),
		"the subgraph's import should feed the node: fixture is wrong"
	);

	(subgraph, node)
}

/// The redirect dot carrying the subgraph's import out its export, if that's what joins them.
fn dot_between_import_and_export(editor: &EditorTestUtils, subgraph: NodeId) -> Option<NodeId> {
	let network_interface = &editor.active_document().network_interface;
	let path = [subgraph];
	let dot = network_interface.upstream_output_connector(&InputConnector::Export(0), &path)?.node_id()?;

	let joins_them = network_interface.is_redirect_dot(&dot, &path) && network_interface.upstream_output_connector(&InputConnector::primary_input(dot), &path) == Some(OutputConnector::Import(0));
	joins_them.then_some(dot)
}

#[tokio::test]
async fn deleting_the_only_node_between_an_import_and_export_leaves_a_dot_joining_them() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (subgraph, node) = subgraph_holding_one_node(&mut editor).await;

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;

	assert!(!exists_in(&editor, node, &[subgraph]), "the node should be deleted");
	assert!(
		dot_between_import_and_export(&editor, subgraph).is_some(),
		"deleting should dissolve the node from the flow as shaking does, keeping the import wired out through a dot"
	);
}

#[tokio::test]
async fn deleting_the_dot_between_an_import_and_export_puts_a_new_one_in_its_place() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (subgraph, node) = subgraph_holding_one_node(&mut editor).await;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
	let dot = dot_between_import_and_export(&editor, subgraph).expect("deleting the node should have left a dot");

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![dot] }).await;
	editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;

	assert!(!exists_in(&editor, dot, &[subgraph]), "the dot should be deleted");
	let replacement = dot_between_import_and_export(&editor, subgraph).expect("a network needs a node to carry its import out, so a new dot should take the old one's place");
	assert_ne!(replacement, dot);
}

/// Wires the subgraph's one node into a second export too, so the node carries its import out of both exports.
async fn feed_a_second_export(editor: &mut EditorTestUtils, node: NodeId) {
	editor.handle_message(NodeGraphMessage::AddSecondaryExport).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(node),
			input_connector: InputConnector::Export(1),
		})
		.await;
}

/// The redirect dot carrying the subgraph's import out of both its exports, if one dot fans out to them and it's the only dot there.
fn one_dot_fanning_out_to_both_exports(editor: &EditorTestUtils, subgraph: NodeId) -> Option<NodeId> {
	let dot = dot_between_import_and_export(editor, subgraph)?;
	let network_interface = &editor.active_document().network_interface;
	let path = [subgraph];

	let feeds_both = network_interface.upstream_output_connector(&InputConnector::Export(1), &path) == Some(OutputConnector::node(dot, 0));
	let dot_count = network_interface
		.nested_network(&path)?
		.nodes
		.keys()
		.filter(|node_id| network_interface.is_redirect_dot(node_id, &path))
		.count();
	(feeds_both && dot_count == 1).then_some(dot)
}

#[tokio::test]
async fn deleting_what_joins_an_import_to_two_exports_leaves_one_dot_fanning_out_to_both() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (subgraph, node) = subgraph_holding_one_node(&mut editor).await;
	feed_a_second_export(&mut editor, node).await;

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
	let dot = one_dot_fanning_out_to_both_exports(&editor, subgraph).expect("deleting the node should leave one dot feeding both exports");

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![dot] }).await;
	editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
	let replacement = one_dot_fanning_out_to_both_exports(&editor, subgraph).expect("deleting the dot should put one new dot in its place, still feeding both exports");
	assert_ne!(replacement, dot);
}

#[tokio::test]
async fn shaking_out_what_joins_an_import_to_two_exports_leaves_one_dot_fanning_out_to_both() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (subgraph, node) = subgraph_holding_one_node(&mut editor).await;
	feed_a_second_export(&mut editor, node).await;

	shake_nodes_in(&mut editor, &[subgraph], vec![node]).await;

	assert!(
		one_dot_fanning_out_to_both_exports(&editor, subgraph).is_some(),
		"shaking the node out should leave one dot feeding both exports"
	);
}

#[test]
fn a_wire_is_found_near_its_path_but_not_away_from_it() {
	let (source, consumer) = (NodeId(1), NodeId(2));
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.insert_node(source, plain_node(1), &[]);
	network_interface.insert_node(consumer, plain_node(1), &[]);
	network_interface.set_absolute_position(&source, IVec2::new(0, 0), &[]);
	network_interface.set_absolute_position(&consumer, IVec2::new(12, 6), &[]);
	network_interface.set_input(&InputConnector::node_at_index(consumer, 0), NodeInput::node(source, 0), &[]);

	let input = InputConnector::node_at_index(consumer, 0);
	let on_wire = wire_middle(&network_interface, input);

	assert_eq!(network_interface.wire_input_from_point(on_wire, 6., GraphWireStyle::Direct, &[]), Some(input));
	assert_eq!(network_interface.wire_input_from_point(on_wire + DVec2::new(0., 200.), 6., GraphWireStyle::Direct, &[]), None);
}

#[tokio::test]
async fn a_dragged_layer_keeps_its_wires_where_it_is_drawn_in_the_frame_it_moves() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let (layer, ..) = layer_with_chain(&mut editor).await;
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;

	let left_input = InputConnector::layer_secondary_input(layer);
	let network_interface = &editor.active_document().network_interface;
	let wire_before = network_interface.wire_path_from_input(&left_input, GraphWireStyle::default(), false, &GlideOffsets::default(), &[]);
	let bounds = network_interface.node_bounding_box(&layer, &[]).expect("the layer should have bounds");
	let press = DVec2::new(bounds[1].x - 30., (bounds[0].y + bounds[1].y) / 2.);
	let node_graph_to_viewport = network_interface
		.network_metadata(&[])
		.expect("the network should have metadata")
		.persistent_metadata
		.navigation_metadata
		.node_graph_to_viewport;

	let pressed = node_graph_to_viewport.transform_point2(press);
	editor.move_mouse(pressed.x, pressed.y, ModifierKeys::empty(), MouseKeys::empty()).await;
	editor
		.handle_message(NodeGraphMessage::PointerDown {
			shift_click: false,
			control_click: false,
			alt_click: false,
			right_click: false,
		})
		.await;

	// One grid cell right, through the input preprocessor so the frontend messages of the move itself are returned
	let moved = node_graph_to_viewport.transform_point2(press + DVec2::new(GRID_SIZE as f64, 0.));
	let editor_mouse_state = EditorPointerState {
		editor_position: moved,
		mouse_keys: MouseKeys::LEFT,
		..Default::default()
	};
	let messages = editor
		.handle_message(InputPreprocessorMessage::PointerMove {
			editor_mouse_state,
			modifier_keys: ModifierKeys::empty(),
		})
		.await;

	// Still drawn a whole cell back as it starts gliding, so the last wire sent to its left input is the one from before the move
	let last_wire_sent = messages
		.into_iter()
		.filter_map(|message| match message {
			FrontendMessage::UpdateNodeGraphWires { wires } => Some(wires),
			_ => None,
		})
		.flatten()
		.rfind(|wire| wire.id == layer && wire.input_index == 1)
		.and_then(|wire| wire.wire_path_update);
	assert_eq!(last_wire_sent.map(|wire| wire.path_string), wire_before.map(|wire| wire.path_string));
}

/// Nudges the only node of a new document while its graph is open, so the node starts gliding into the cell it moved to.
async fn gliding_nudged_node(editor: &mut EditorTestUtils) -> NodeId {
	editor.new_document().await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	editor.handle_message(NodeGraphMessage::NudgeSelectedNodes { direction: Direction::Right }).await;
	assert!(
		!editor.active_document().network_interface.glide_offsets(&[]).is_empty(),
		"the nudged node should be gliding: fixture is wrong"
	);
	node
}

#[tokio::test]
async fn redoing_a_nudge_undone_while_it_glides_leaves_nothing_gliding() {
	let mut editor = EditorTestUtils::create();
	// Undone from the history's own snapshots, rather than the storage rebuilt afresh
	editor.editor.handle_message(PreferencesMessage::SaveAsGdd { enabled: false });
	gliding_nudged_node(&mut editor).await;

	editor.handle_message(DocumentMessage::Undo).await;
	editor.handle_message(DocumentMessage::Redo).await;

	assert!(editor.active_document().network_interface.glide_offsets(&[]).is_empty());
}

#[tokio::test]
async fn entering_a_node_while_its_network_glides_settles_it() {
	let mut editor = EditorTestUtils::create();
	gliding_nudged_node(&mut editor).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 20, 0).await;

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;

	assert!(editor.active_document().network_interface.glide_offsets(&[]).is_empty());
}
