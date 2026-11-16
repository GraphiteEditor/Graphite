use super::template::NodeTemplateImplementation;
use super::{InputConnector, OutputConnector, catalog};
use crate::messages::portfolio::document::node_graph::document_node_definitions::{network_node_identifiers, resolve_network_node_type};
use crate::messages::portfolio::document::storage_tests::test_support::round_trip_through_gdd;
use crate::test_utils::test_prelude::*;
use graph_craft::document::{DocumentNodeImplementation, NetworkId, NodeInput};
use graphene_std::uuid::NodeId;

async fn editor_with_blends(count: usize) -> (EditorTestUtils, Vec<NodeId>) {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let template = resolve_network_node_type("Blend").expect("the catalog should hold Blend").default_node_template();
	let blends: Vec<NodeId> = (0..count).map(|_| NodeId::new()).collect();
	let interface = &mut editor.active_document_mut().network_interface;
	for blend in &blends {
		interface.insert_node(*blend, template.clone(), &[]);
	}
	(editor, blends)
}

fn entry_of(editor: &EditorTestUtils, node_id: NodeId) -> NetworkId {
	let DocumentNodeImplementation::Definition(id) = editor.active_document().network_interface.document_network().nodes[&node_id].implementation else {
		panic!("the node should name a table entry")
	};
	id
}

#[tokio::test]
async fn catalog_nodes_share_the_shipped_entry() {
	let (editor, blends) = editor_with_blends(2).await;

	let shared = entry_of(&editor, blends[0]);
	assert_eq!(entry_of(&editor, blends[1]), shared);
	assert!(catalog::is_protected(shared));
	assert_eq!(
		editor.active_document().network_interface.document_network().definitions[&shared],
		catalog::entry(shared).expect("shipped").network
	);
	assert!(editor.active_document().network_interface.validate_invariants().is_empty());
}

#[tokio::test]
async fn every_catalog_template_inserts_as_its_shipped_entry() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let unrecognized: Vec<_> = network_node_identifiers()
		.filter(|identifier| {
			let node_id = NodeId::new();
			let template = resolve_network_node_type(identifier).expect("listed identifiers resolve").default_node_template();
			// An empty network has nothing to share, so it inserts as a definition of its own
			if !matches!(&template.implementation, NodeTemplateImplementation::Network(network) if !network.nodes.is_empty()) {
				return false;
			}
			let interface = &mut editor.active_document_mut().network_interface;
			interface.insert_node(node_id, template, &[]);
			!matches!(interface.document_network().nodes[&node_id].implementation, DocumentNodeImplementation::Definition(id) if catalog::is_protected(id))
		})
		.collect();
	assert!(unrecognized.is_empty(), "these should insert as their catalog entries: {unrecognized:?}");
}

#[tokio::test]
async fn editing_the_signature_of_one_catalog_node_leaves_its_siblings_wired() {
	let (mut editor, blends) = editor_with_blends(3).await;
	let interface = &mut editor.active_document_mut().network_interface;
	let sibling_input = InputConnector::node_at_index(blends[1], 1);
	interface.create_wire(&OutputConnector::node(blends[2], 0), &sibling_input, &[]);
	let sibling_input_count = interface.document_network().nodes[&blends[1]].inputs.len();

	interface.make_editable(&[blends[0]]);
	interface.remove_import(1, &[blends[0]]);

	let interface = &editor.active_document().network_interface;
	assert!(matches!(interface.input_from_connector(&sibling_input, &[]), Some(NodeInput::Node { .. })));
	assert_eq!(interface.document_network().nodes[&blends[1]].inputs.len(), sibling_input_count);
	assert_eq!(interface.document_network().nodes[&blends[0]].inputs.len(), sibling_input_count - 1);
}

#[tokio::test]
async fn navigating_inside_a_catalog_node_does_not_fork_it() {
	let (mut editor, blends) = editor_with_blends(1).await;
	let shipped = entry_of(&editor, blends[0]);

	let interface = &mut editor.active_document_mut().network_interface;
	interface.set_transform(glam::DAffine2::from_translation((3., 5.).into()), &[blends[0]]);
	interface.set_node_graph_width(400., &[blends[0]]);
	assert_eq!(entry_of(&editor, blends[0]), shipped);
}

#[tokio::test]
async fn an_edit_inside_a_catalog_node_applies_only_once_it_is_made_editable() {
	let (mut editor, blends) = editor_with_blends(2).await;
	let shipped = entry_of(&editor, blends[0]);
	let toggled_value_input = |interface: &super::NodeNetworkInterface, definition: NetworkId| {
		let (inner, input_index) = interface.document_network().definitions[&definition]
			.nodes
			.iter()
			.find_map(|(id, node)| node.inputs.iter().position(|input| matches!(input, NodeInput::Value { .. })).map(|index| (*id, index)))
			.expect("Blend should have a value input inside");
		let NodeInput::Value { tagged_value, exposed } = interface.document_network().definitions[&definition].nodes[&inner].inputs[input_index].clone() else {
			unreachable!("the input was found as a value")
		};
		(InputConnector::node_at_index(inner, input_index), NodeInput::value((*tagged_value).clone(), !exposed))
	};

	let interface = &mut editor.active_document_mut().network_interface;
	let (connector, input) = toggled_value_input(interface, shipped);
	interface.set_input(&connector, input, &[blends[0]]);
	assert_eq!(entry_of(&editor, blends[0]), shipped, "a write into a read-only network should be refused rather than fork it");

	let interface = &mut editor.active_document_mut().network_interface;
	interface.make_editable(&[blends[0]]);
	let forked = entry_of(&editor, blends[0]);
	let interface = &mut editor.active_document_mut().network_interface;
	let (connector, input) = toggled_value_input(interface, forked);
	interface.set_input(&connector, input.clone(), &[blends[0]]);

	let interface = &editor.active_document().network_interface;
	assert_eq!(interface.input_from_connector(&connector, &[blends[0]]), Some(&input));
	assert_ne!(forked, shipped);
	assert!(!catalog::is_protected(forked));
	assert_eq!(entry_of(&editor, blends[1]), shipped);
	assert_eq!(interface.document_network().definitions[&shipped], catalog::entry(shipped).expect("shipped").network);
}

#[tokio::test]
async fn moving_a_catalog_node_does_not_fork_it() {
	let (mut editor, blends) = editor_with_blends(1).await;
	let shipped = entry_of(&editor, blends[0]);

	editor.active_document_mut().network_interface.shift_node(&blends[0], glam::IVec2::new(3, 5), &[]);
	assert_eq!(entry_of(&editor, blends[0]), shipped);
}

#[tokio::test]
async fn repairs_on_open_do_not_fork_catalog_nodes() {
	let (mut editor, blends) = editor_with_blends(1).await;
	let shipped = entry_of(&editor, blends[0]);

	editor.active_document_mut().finalize_storage_load();
	assert_eq!(entry_of(&editor, blends[0]), shipped);
}

#[tokio::test]
async fn an_entry_that_diverged_under_a_catalog_id_gets_a_fresh_id_on_open() {
	let (mut editor, blends) = editor_with_blends(1).await;
	let shipped = entry_of(&editor, blends[0]);

	let interface = &mut editor.active_document_mut().network_interface;
	interface.migrate_graph(|network| {
		let definition = network.definitions.get_mut(&shipped).expect("the shipped entry");
		definition.exports.push(NodeInput::value(graph_craft::document::value::TaggedValue::None, false));
	});
	interface.give_diverged_catalog_entries_new_ids();

	let renamed = entry_of(&editor, blends[0]);
	assert_ne!(renamed, shipped);
	let network = editor.active_document().network_interface.document_network();
	assert!(!network.definitions.contains_key(&shipped));
	assert!(network.definitions[&renamed].exports.len() > catalog::entry(shipped).expect("shipped").network.exports.len());
}

#[tokio::test]
async fn a_reopened_document_keeps_its_catalog_entries() {
	let (editor, blends) = editor_with_blends(1).await;
	let shipped = entry_of(&editor, blends[0]);

	let mut reopened = round_trip_through_gdd(editor.active_document()).await.rebuilt;
	reopened.give_diverged_catalog_entries_new_ids();

	assert!(reopened.document_network().definitions.contains_key(&shipped));
}

#[tokio::test]
async fn an_entered_catalog_node_stays_read_only_until_made_editable() {
	let (mut editor, blends) = editor_with_blends(2).await;
	let shared = entry_of(&editor, blends[0]);
	let path = vec![blends[0]];
	let import_count = editor.active_document().network_interface.number_of_imports(&path);

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: blends[0] }).await;
	assert!(editor.active_document().network_interface.is_read_only(&path));
	editor.handle_message(NodeGraphMessage::AddImport).await;
	assert_eq!(
		editor.active_document().network_interface.number_of_imports(&path),
		import_count,
		"a read-only network should ignore edits"
	);
	assert_eq!(entry_of(&editor, blends[0]), shared, "an ignored edit shouldn't fork the entry");

	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	assert!(!editor.active_document().network_interface.is_read_only(&path));
	assert_ne!(entry_of(&editor, blends[0]), shared, "the entered node should get its own copy");
	assert_eq!(entry_of(&editor, blends[1]), shared, "its sibling should keep the shared entry");
	let definitions = &editor.active_document().network_interface.document_network().definitions;
	let copied_ids = definitions[&entry_of(&editor, blends[0])].nodes.keys();
	assert!(
		copied_ids.into_iter().all(|node_id| !definitions[&shared].nodes.contains_key(node_id)),
		"the copy's nodes should get IDs of their own"
	);

	editor.handle_message(NodeGraphMessage::AddImport).await;
	assert_eq!(editor.active_document().network_interface.number_of_imports(&path), import_count + 1);
	assert!(editor.active_document().network_interface.validate_invariants().is_empty());
}

#[tokio::test]
async fn undoing_the_unlock_of_an_open_catalog_node_shows_it_read_only_again() {
	let (mut editor, blends) = editor_with_blends(1).await;
	let path = vec![blends[0]];
	editor.enter_editable_network(blends[0]).await;
	assert!(!editor.active_document().network_interface.is_read_only(&path));

	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(editor.active_document().breadcrumb_network_path(), path, "the catalog node itself still exists");
	assert!(editor.active_document().network_interface.is_read_only(&path));
}

#[tokio::test]
async fn undoing_the_creation_of_an_open_subgraph_leaves_it() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let subgraph = editor.create_node_by_name(DefinitionIdentifier::Network("Custom Node".to_string())).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: subgraph }).await;
	assert_eq!(editor.active_document().breadcrumb_network_path(), [subgraph]);

	editor.handle_message(DocumentMessage::Undo).await;
	assert!(editor.active_document().breadcrumb_network_path().is_empty());
	assert!(editor.active_document().selection_network_path().is_empty());
}

#[tokio::test]
async fn undoing_and_redoing_the_unlock_of_a_layer_keeps_its_nodes() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 10., 10.).await;
	let layer = editor.get_selected_layer().await.expect("the drawn rectangle should be selected").to_node();
	let path = vec![layer];
	let node_count = |editor: &EditorTestUtils| editor.active_document().network_interface.nested_network(&path).map(|network| network.nodes.len());
	let shipped = node_count(&editor);

	editor.enter_editable_network(layer).await;
	assert_eq!(node_count(&editor), shipped);

	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(node_count(&editor), shipped, "undoing the unlock should bring back the shared contents");
	assert!(editor.active_document().network_interface.is_read_only(&path));

	editor.handle_message(DocumentMessage::Redo).await;
	assert_eq!(node_count(&editor), shipped, "redoing the unlock should bring back the copy");
	assert!(!editor.active_document().network_interface.is_read_only(&path));
	assert!(editor.active_document().network_interface.validate_invariants().is_empty());
}
