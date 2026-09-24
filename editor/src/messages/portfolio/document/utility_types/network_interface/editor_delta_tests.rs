use super::DocumentNodePersistentMetadata;
use super::InputConnector;
use super::editor_delta::{EditorDelta, NetworkMetadataChange, NodeMetadataChange, construct_batch};
use super::storage_metadata::StorageMetadataView;
use crate::test_utils::test_prelude::*;
use document_graph_storage::delta::compute_deltas;
use document_graph_storage::{PeerId, Registry, RegistryDelta, Session};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graph_craft::runtime_delta::RuntimeDelta;
use graphene_std::uuid::NodeId;

const PEER: PeerId = PeerId(7);

fn rectangle_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
}

fn convert(editor: &EditorTestUtils) -> Registry {
	let document = editor.active_document();
	Registry::convert_from_runtime(
		document.network_interface.document_network(),
		&StorageMetadataView::new(&document.network_interface),
		&document.resources.registry,
		PEER,
	)
	.expect("conversion should succeed")
	.registry
}

fn construct(editor: &EditorTestUtils, deltas: &[EditorDelta], working: &Registry) -> Vec<RegistryDelta> {
	let document = editor.active_document();
	construct_batch(deltas, working, &document.resources.registry, &StorageMetadataView::new(&document.network_interface), PEER)
		.expect("construction should succeed")
		.ops
}

fn node_metadata(editor: &EditorTestUtils, node_id: NodeId) -> DocumentNodePersistentMetadata {
	editor
		.active_document()
		.network_interface
		.node_metadata(&node_id, &[])
		.expect("node metadata should exist")
		.persistent_metadata
		.clone()
}

fn metadata_change(node_id: NodeId, change: NodeMetadataChange) -> EditorDelta {
	EditorDelta::NodeMetadata {
		network_path: Vec::new(),
		node_id,
		change,
	}
}

fn assert_same_stored_effect(working: &Registry, constructed: Vec<RegistryDelta>, diffed: Vec<RegistryDelta>, at: &str) {
	let baseline = compute_deltas(&Registry::default(), working);

	let mut from_construction = Session::with_peer(PEER);
	from_construction.stage_computed_ops(baseline.clone()).expect("baseline should stage");
	from_construction.stage_computed_ops(constructed).expect("constructed ops should stage");

	let mut from_diff = Session::with_peer(PEER);
	from_diff.stage_computed_ops(baseline).expect("baseline should stage");
	from_diff.stage_computed_ops(diffed).expect("diffed ops should stage");

	assert!(
		from_construction.registry().value_equal(from_diff.registry()),
		"Constructed ops must produce the same stored state as the whole-document diff: {at}\nresidual: {:#?}",
		compute_deltas(from_construction.registry(), from_diff.registry())
	);
}

#[tokio::test]
async fn set_input_value_constructs_the_exact_diff_op() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name(rectangle_definition()).await;

	let working = convert(&editor);
	let input = NodeInput::value(TaggedValue::Number(42.), false);
	editor.active_document_mut().network_interface.set_input(&InputConnector::node_at_index(node, 1), input.clone(), &[]);

	let delta = EditorDelta::Graph(RuntimeDelta::SetInput {
		network_path: Vec::new(),
		node_id: node,
		input_index: 1,
		input,
	});

	let constructed = construct(&editor, std::slice::from_ref(&delta), &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_eq!(constructed, diffed, "A value edit should construct exactly the diff's op");
}

#[tokio::test]
async fn wiring_and_export_edits_construct_the_exact_diff_ops() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let a = editor.create_node_by_name(rectangle_definition()).await;
	let b = editor.create_node_by_name(rectangle_definition()).await;

	let working = convert(&editor);
	let wire = NodeInput::node(b, 0);
	editor.active_document_mut().network_interface.set_input(&InputConnector::node_at_index(a, 1), wire.clone(), &[]);
	let delta = EditorDelta::Graph(RuntimeDelta::SetInput {
		network_path: Vec::new(),
		node_id: a,
		input_index: 1,
		input: wire,
	});
	let constructed = construct(&editor, std::slice::from_ref(&delta), &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_eq!(constructed, diffed, "A wiring edit should construct exactly the diff's op");

	let working = convert(&editor);
	let export = NodeInput::node(a, 0);
	editor.active_document_mut().network_interface.set_input(&InputConnector::Export(0), export.clone(), &[]);
	let delta = EditorDelta::Graph(RuntimeDelta::SetExport {
		network_path: Vec::new(),
		export_index: 0,
		input: Some(export),
	});
	let constructed = construct(&editor, std::slice::from_ref(&delta), &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_eq!(constructed, diffed, "An export edit should construct exactly the diff's op");
}

#[tokio::test]
async fn adding_a_node_as_structure_plus_metadata_matches_the_diff() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let working = convert(&editor);
	let template = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(&rectangle_definition())
		.expect("rectangle definition")
		.default_node_template();
	let node_id = NodeId(0xDE17A);
	let (document_node, metadata) = template.clone().into_parts();
	editor.active_document_mut().network_interface.insert_node(node_id, template, &[]);

	let deltas = [
		EditorDelta::Graph(RuntimeDelta::AddNode {
			network_path: Vec::new(),
			node_id,
			node: Box::new(document_node),
		}),
		EditorDelta::NodeMetadataSnapshot {
			network_path: Vec::new(),
			node_id,
			metadata: Box::new(metadata),
		},
	];

	let constructed = construct(&editor, &deltas, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "node addition");
}

#[tokio::test]
async fn per_field_metadata_edits_match_the_diff() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name(rectangle_definition()).await;

	let working = convert(&editor);
	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&node, "Renamed".to_string(), &[]);
		network_interface.set_visibility(&node, &[], false);
		network_interface.set_locked(&node, &[], true);
		network_interface.set_pinned(&node, &[], true);
		network_interface.shift_node(&node, glam::IVec2::new(3, 5), &[]);
	}

	let metadata = node_metadata(&editor, node);
	let deltas = [
		metadata_change(node, NodeMetadataChange::DisplayName(metadata.display_name.clone())),
		metadata_change(node, NodeMetadataChange::Locked(metadata.locked)),
		metadata_change(node, NodeMetadataChange::Pinned(metadata.pinned)),
		metadata_change(node, NodeMetadataChange::NodeType(metadata.node_type_metadata.clone())),
		EditorDelta::Graph(RuntimeDelta::SetVisibility {
			network_path: Vec::new(),
			node_id: node,
			visible: false,
		}),
		// Pinning a node also appends it to the network's display order, which is network state.
		EditorDelta::NetworkMetadata {
			network_path: Vec::new(),
			change: NetworkMetadataChange::PinnedOrder(vec![node]),
		},
	];
	let constructed = construct(&editor, &deltas, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "per-field metadata edits");
}

/// The encoding spells "unset" as an absent attribute, so a per-field delta that sets a field back to
/// its unset value must clear the key rather than write a sentinel.
#[tokio::test]
async fn clearing_a_metadata_field_removes_the_attribute() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name(rectangle_definition()).await;

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&node, "Renamed".to_string(), &[]);
		network_interface.set_locked(&node, &[], true);
	}

	let working = convert(&editor);
	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&node, String::new(), &[]);
		network_interface.set_locked(&node, &[], false);
	}

	let metadata = node_metadata(&editor, node);
	let deltas = [
		metadata_change(node, NodeMetadataChange::DisplayName(metadata.display_name.clone())),
		metadata_change(node, NodeMetadataChange::Locked(metadata.locked)),
	];
	let constructed = construct(&editor, &deltas, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "cleared metadata fields");
}

/// The deltas the interface emits for a real edit must store what a whole-document conversion of the
/// result would. Taking them from the interface rather than writing them by hand is what catches a
/// setter that forgets to emit, or emits the wrong field.
#[tokio::test]
async fn emitted_deltas_reproduce_the_diff() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name(rectangle_definition()).await;

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&node, "Renamed".to_string(), &[]);
		network_interface.set_locked(&node, &[], true);
		network_interface.set_pinned(&node, &[], true);
		network_interface.set_visibility(&node, &[], false);
		network_interface.shift_node(&node, glam::IVec2::new(3, 5), &[]);
	}

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	assert!(!emitted.is_empty(), "the edits should have emitted deltas");

	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "emitted deltas");
}

/// A write that changes nothing is not a write, so it must not emit. Otherwise a redundant setter call
/// would stamp a fresh timestamp and win against a concurrent peer that did change the field.
#[tokio::test]
async fn unchanged_writes_emit_nothing() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name(rectangle_definition()).await;

	editor.active_document_mut().network_interface.set_display_name(&node, "Named".to_string(), &[]);
	editor.active_document_mut().network_interface.discard_deltas();

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&node, "Named".to_string(), &[]);
		network_interface.set_locked(&node, &[], false);
		network_interface.shift_node(&node, glam::IVec2::ZERO, &[]);
	}

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	assert!(emitted.is_empty(), "re-writing the same values should emit nothing, got {emitted:#?}");
}

/// Structural writes go through the store too, so inserting and deleting a node must be recorded
/// without the caller saying anything about it.
#[tokio::test]
async fn emitted_deltas_reproduce_the_diff_for_structural_edits() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	let template = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(&rectangle_definition())
		.expect("rectangle definition")
		.default_node_template();
	let node_id = NodeId(0xDE17A);
	editor.active_document_mut().network_interface.insert_node(node_id, template, &[]);

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "emitted node insertion");

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();
	editor.active_document_mut().network_interface.delete_nodes(vec![node_id], true, &[]);

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "emitted node deletion");
}

/// Adding an import shifts every later input slot, which no index-addressed op can express. The store
/// restates the whole list instead, so this checks that restatement lands the same as the diff.
#[tokio::test]
async fn emitted_deltas_reproduce_the_diff_for_an_arity_change() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: crate::messages::portfolio::document::utility_types::misc::GroupFolderType::Layer,
		})
		.await;

	let group = editor
		.active_document()
		.network_interface
		.document_network()
		.nodes
		.iter()
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Network(_)))
		.map(|(id, _)| *id)
		.expect("the group should be a network node");

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor
		.active_document_mut()
		.network_interface
		.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	assert!(!emitted.is_empty(), "adding an import should have emitted deltas");

	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "emitted import addition");
}

/// An arity change rebuilds the node's whole record, so anything written to that node earlier in the
/// same batch has to survive it. Renaming first and adding the import second is the order that catches
/// a rebuild sourced from the pre-batch registry rather than from what the batch has done so far.
#[tokio::test]
async fn an_arity_change_keeps_an_earlier_edit_to_the_same_node() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: crate::messages::portfolio::document::utility_types::misc::GroupFolderType::Layer,
		})
		.await;

	let group = editor
		.active_document()
		.network_interface
		.document_network()
		.nodes
		.iter()
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Network(_)))
		.map(|(id, _)| *id)
		.expect("the group should be a network node");

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor.active_document_mut().network_interface.set_display_name(&group, "Renamed".to_string(), &[]);
	editor
		.active_document_mut()
		.network_interface
		.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "rename then arity change");
}

/// What an edit records must be enough to reproduce it: applying the recorded deltas to a clone of the
/// interface taken before the edit must yield the edited interface. A write recorded wrongly or not at
/// all surfaces here rather than as drift on another peer.
async fn assert_replay_reproduces(edit: impl FnOnce(&mut EditorTestUtils)) {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: crate::messages::portfolio::document::utility_types::misc::GroupFolderType::Layer,
		})
		.await;
	editor.active_document_mut().network_interface.discard_deltas();

	let before = editor.active_document().network_interface.clone();
	edit(&mut editor);

	let recorded = editor.active_document_mut().network_interface.take_deltas();
	assert!(!recorded.is_empty(), "the edit should have recorded something to replay");

	let mut replayed = before;
	for delta in &recorded {
		replayed.apply(delta);
	}

	assert_eq!(
		replayed,
		editor.active_document().network_interface,
		"replaying {} recorded deltas did not reproduce the edited interface",
		recorded.len()
	);
}

fn only_group(editor: &EditorTestUtils) -> NodeId {
	editor
		.active_document()
		.network_interface
		.document_network()
		.nodes
		.iter()
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Network(_)))
		.map(|(id, _)| *id)
		.expect("the group should be a network node")
}

#[tokio::test]
async fn replaying_metadata_edits_reproduces_the_interface() {
	assert_replay_reproduces(|editor| {
		let group = only_group(editor);
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&group, "Renamed".to_string(), &[]);
		network_interface.set_locked(&group, &[], true);
		network_interface.set_pinned(&group, &[], true);
		network_interface.set_visibility(&group, &[], false);
		network_interface.shift_node(&group, glam::IVec2::new(3, 5), &[]);
	})
	.await;
}

#[tokio::test]
async fn replaying_an_arity_change_reproduces_the_interface() {
	assert_replay_reproduces(|editor| {
		let group = only_group(editor);
		editor
			.active_document_mut()
			.network_interface
			.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);
	})
	.await;
}

#[tokio::test]
async fn replaying_a_node_insertion_reproduces_the_interface() {
	assert_replay_reproduces(|editor| {
		let template = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(&rectangle_definition())
			.expect("rectangle definition")
			.default_node_template();
		editor.active_document_mut().network_interface.insert_node(NodeId(0xDE17A), template, &[]);
	})
	.await;
}

/// A node added and removed inside one batch is never in the registry the batch started from, so the
/// removal has nothing to look up there. The pair has to cancel out rather than leaving the addition
/// staged on its own.
#[tokio::test]
async fn a_node_added_and_removed_in_one_batch_leaves_nothing_behind() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	let template = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(&rectangle_definition())
		.expect("rectangle definition")
		.default_node_template();
	let node_id = NodeId(0xDE17A);
	editor.active_document_mut().network_interface.insert_node(node_id, template, &[]);
	editor.active_document_mut().network_interface.delete_nodes(vec![node_id], true, &[]);

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "add then remove in one batch");
}

#[tokio::test]
async fn removing_a_nested_network_node_matches_the_diff() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: crate::messages::portfolio::document::utility_types::misc::GroupFolderType::Layer,
		})
		.await;

	let before: Vec<NodeId> = editor.active_document().network_interface.document_network().nodes.keys().copied().collect();
	let working = convert(&editor);

	let group = editor
		.active_document()
		.network_interface
		.document_network()
		.nodes
		.iter()
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Network(_)))
		.map(|(id, _)| *id)
		.expect("the group should be a network node");
	editor.active_document_mut().network_interface.delete_nodes(vec![group], true, &[]);

	let network = editor.active_document().network_interface.document_network().clone();
	let mut deltas: Vec<EditorDelta> = before
		.iter()
		.filter(|id| !network.nodes.contains_key(id))
		.map(|id| {
			EditorDelta::Graph(RuntimeDelta::RemoveNode {
				network_path: Vec::new(),
				node_id: *id,
			})
		})
		.collect();
	for (index, export) in network.exports.iter().enumerate() {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
			network_path: Vec::new(),
			export_index: index,
			input: Some(export.clone()),
		}));
	}
	for (node_id, node) in &network.nodes {
		for (index, input) in node.inputs.iter().enumerate() {
			deltas.push(EditorDelta::Graph(RuntimeDelta::SetInput {
				network_path: Vec::new(),
				node_id: *node_id,
				input_index: index,
				input: input.clone(),
			}));
		}
	}

	let constructed = construct(&editor, &deltas, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "nested network removal");
}

/// A group carries a nested network, so adding and removing one inside a batch has to cancel out the
/// network as well as the nodes. The proto-node case above does not reach this, since a proto node has
/// no network of its own.
#[tokio::test]
async fn a_group_added_and_removed_in_one_batch_leaves_nothing_behind() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: crate::messages::portfolio::document::utility_types::misc::GroupFolderType::Layer,
		})
		.await;
	let group = only_group(&editor);
	editor.active_document_mut().network_interface.delete_nodes(vec![group], true, &[]);

	let emitted = editor.active_document_mut().network_interface.take_deltas();
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "group added then removed in one batch");
}

/// Swapping a node's implementation must not restate the node's own metadata: the swap did not write it,
/// and asserting it would overwrite whatever a concurrent peer set for the node's name, lock or pin.
#[tokio::test]
async fn swapping_an_implementation_leaves_the_node_in_place() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;

	let working = convert(&editor);

	let deltas = {
		let interface = &mut editor.active_document_mut().network_interface;
		interface.discard_deltas();
		let Some(mut cursor) = interface.node_mut(super::NodeLocator::new(node, &[])) else {
			panic!("the node should resolve")
		};
		cursor.replace_implementation(graph_craft::document::DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER), None);
		interface.take_deltas()
	};

	let ops = construct(&editor, &deltas, &working);

	assert!(
		ops.iter().any(|op| matches!(op, RegistryDelta::SetNodeImplementation { .. })),
		"the swap should update the implementation in place"
	);
	assert!(
		!ops.iter().any(|op| matches!(op, RegistryDelta::RemoveNode { .. })),
		"the node itself should not be rebuilt, since that would clear the attributes it carries"
	);
	assert!(
		!deltas.iter().any(|delta| matches!(delta, EditorDelta::NodeMetadataSnapshot { node_id, .. } if *node_id == node)),
		"the node's own metadata should not be restated by a swap that did not write it"
	);
}
