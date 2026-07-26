use super::{DeltaJournal, InputConnector, RecordedChange};
use crate::test_utils::test_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::uuid::NodeId;

fn rectangle_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
}

/// A fresh document's journal is `Desynced`; taking it simulates the staging that resets tracking.
fn start_tracking(editor: &mut EditorTestUtils) {
	let network_interface = &mut editor.active_document_mut().network_interface;
	assert!(!network_interface.journal().is_tracking(), "A freshly opened document should start desynced");
	network_interface.take_journal();
	assert!(network_interface.journal().is_tracking(), "Taking the journal should leave it tracking");
}

fn recorded_changes(editor: &mut EditorTestUtils) -> Vec<RecordedChange> {
	match editor.active_document_mut().network_interface.journal() {
		DeltaJournal::Tracking { changes } => changes.clone(),
		DeltaJournal::Desynced { reason } => panic!("Journal unexpectedly desynced: {reason}"),
	}
}

fn node_change(node_id: NodeId) -> RecordedChange {
	RecordedChange::Node { network_path: Vec::new(), node_id }
}

fn input_change(node_id: NodeId, input_index: usize) -> RecordedChange {
	RecordedChange::NodeInput {
		network_path: Vec::new(),
		node_id,
		input_index,
	}
}

#[tokio::test]
async fn creating_a_node_records_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	start_tracking(&mut editor);

	let rectangle = editor.create_node_by_name(rectangle_definition()).await;

	assert!(recorded_changes(&mut editor).contains(&node_change(rectangle)), "Inserting a node should record it");
}

#[tokio::test]
async fn wiring_and_metadata_edits_record_the_touched_node() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let a = editor.create_node_by_name(rectangle_definition()).await;
	let b = editor.create_node_by_name(rectangle_definition()).await;
	start_tracking(&mut editor);

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_input(&InputConnector::node(a, 1), NodeInput::node(b, 0), &[]);
	assert!(recorded_changes(&mut editor).contains(&input_change(a, 1)), "Wiring an input should record the destination slot");

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_visibility(&b, &[], false);
	network_interface.set_display_name(&b, "Renamed".to_string(), &[]);
	let changes = recorded_changes(&mut editor);
	assert!(changes.contains(&node_change(b)), "Metadata edits should record the touched node");

	// The same entity is recorded once no matter how many times it changes
	let recorded_b = changes.iter().filter(|change| **change == node_change(b)).count();
	assert_eq!(recorded_b, 1, "Repeated changes to one entity should deduplicate");
}

#[tokio::test]
async fn deleting_a_node_records_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let rectangle = editor.create_node_by_name(rectangle_definition()).await;
	start_tracking(&mut editor);

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.delete_nodes(vec![rectangle], false, &[]);

	assert!(recorded_changes(&mut editor).contains(&node_change(rectangle)), "Deleting a node should record it");
}

#[tokio::test]
async fn export_changes_record_the_network() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let rectangle = editor.create_node_by_name(rectangle_definition()).await;
	start_tracking(&mut editor);

	let network_interface = &mut editor.active_document_mut().network_interface;
	network_interface.set_input(&InputConnector::Export(0), NodeInput::node(rectangle, 0), &[]);

	let changes = recorded_changes(&mut editor);
	assert!(
		changes.contains(&RecordedChange::Network { network_path: Vec::new() }),
		"Rewiring an export should record the network, recorded: {changes:?}"
	);
}

#[tokio::test]
async fn undo_desyncs_the_journal() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	editor.create_node_by_name(rectangle_definition()).await;
	start_tracking(&mut editor);

	editor.handle_message(DocumentMessage::Undo).await;

	let network_interface = &editor.active_document().network_interface;
	assert!(!network_interface.journal().is_tracking(), "Installing an undo snapshot should desync the journal");
}

#[tokio::test]
async fn raw_network_access_desyncs_the_journal() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	start_tracking(&mut editor);

	let network_interface = &mut editor.active_document_mut().network_interface;
	let _ = network_interface.document_network_mut();

	assert!(!network_interface.journal().is_tracking(), "Raw mutable network access should desync the journal");
}

#[tokio::test]
async fn input_edits_coalesce_per_slot_and_are_subsumed_by_node_records() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let node = editor.create_node_by_name(rectangle_definition()).await;
	start_tracking(&mut editor);

	// A drag-like burst of edits to one slot coalesces into a single record
	let network_interface = &mut editor.active_document_mut().network_interface;
	for width in [1., 2., 3.] {
		network_interface.set_input(&InputConnector::node(node, 1), NodeInput::value(TaggedValue::F64(width), false), &[]);
	}
	let changes = recorded_changes(&mut editor);
	let slot_records = changes.iter().filter(|change| **change == input_change(node, 1)).count();
	assert_eq!(slot_records, 1, "Repeated edits to one slot should coalesce, recorded: {changes:?}");

	// A whole-node change replaces the per-input records for that node
	editor.active_document_mut().network_interface.set_visibility(&node, &[], false);
	let changes = recorded_changes(&mut editor);
	assert!(changes.contains(&node_change(node)), "The node record should be present");
	assert!(
		!changes.iter().any(|change| matches!(change, RecordedChange::NodeInput { .. })),
		"Per-input records should be subsumed, recorded: {changes:?}"
	);

	// Later input edits are subsumed by the existing node record rather than re-recorded
	editor
		.active_document_mut()
		.network_interface
		.set_input(&InputConnector::node(node, 1), NodeInput::value(TaggedValue::F64(4.), false), &[]);
	let changes = recorded_changes(&mut editor);
	assert!(
		!changes.iter().any(|change| matches!(change, RecordedChange::NodeInput { .. })),
		"The node record should subsume new input records"
	);
}
