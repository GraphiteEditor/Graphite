use super::{InputConnector, catalog};
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_network_node_type;
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
async fn an_edit_inside_a_catalog_node_forks_only_that_node() {
	let (mut editor, blends) = editor_with_blends(2).await;
	let shipped = entry_of(&editor, blends[0]);

	let interface = &mut editor.active_document_mut().network_interface;
	let (inner, input_index) = interface.document_network().definitions[&shipped]
		.nodes
		.iter()
		.find_map(|(id, node)| node.inputs.iter().position(|input| matches!(input, NodeInput::Value { .. })).map(|index| (*id, index)))
		.expect("Blend should have a value input inside");
	let NodeInput::Value { tagged_value, exposed } = interface.document_network().definitions[&shipped].nodes[&inner].inputs[input_index].clone() else {
		unreachable!("the input was found as a value")
	};
	interface.set_input(&InputConnector::node_at_index(inner, input_index), NodeInput::value((*tagged_value).clone(), !exposed), &[blends[0]]);

	let forked = entry_of(&editor, blends[0]);
	assert_ne!(forked, shipped);
	assert!(!catalog::is_protected(forked));
	assert_eq!(entry_of(&editor, blends[1]), shipped);
	assert_eq!(
		editor.active_document().network_interface.document_network().definitions[&shipped],
		catalog::entry(shipped).expect("shipped").network
	);
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
