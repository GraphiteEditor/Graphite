use super::DocumentNodePersistentMetadata;
use super::editor_delta::{EditorDelta, NetworkMetadataChange, NodeMetadataChange, construct_batch, ui_attribute_writes};
use super::storage_metadata::StorageMetadataView;
use super::{InputConnector, NodeTemplate};
use crate::test_utils::test_prelude::*;
use document_graph_storage::delta::compute_deltas;
use document_graph_storage::{PeerId, Registry, RegistryDelta, Session};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeInput, OwnedNetworkLocation};
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

/// The deltas a commit would stage, after settling the definitions table as a commit does.
fn committed_deltas(editor: &mut EditorTestUtils) -> Vec<EditorDelta> {
	let interface = &mut editor.active_document_mut().network_interface;
	interface.settle_definitions();
	interface.take_deltas()
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
		network: OwnedNetworkLocation::ROOT,
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
		network: OwnedNetworkLocation::ROOT,
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
		network: OwnedNetworkLocation::ROOT,
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
		network: OwnedNetworkLocation::ROOT,
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
			network: OwnedNetworkLocation::ROOT,
			node_id,
			node: Box::new(document_node),
		}),
		EditorDelta::NodeMetadataSnapshot {
			network: OwnedNetworkLocation::ROOT,
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
			network: OwnedNetworkLocation::ROOT,
			node_id: node,
			visible: false,
		}),
		// Pinning a node also appends it to the network's display order, which is network state.
		EditorDelta::NetworkMetadata {
			network: OwnedNetworkLocation::ROOT,
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

	let emitted = committed_deltas(&mut editor);
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

	let emitted = committed_deltas(&mut editor);
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

	let emitted = committed_deltas(&mut editor);
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "emitted node insertion");

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();
	editor.active_document_mut().network_interface.delete_nodes(vec![node_id], true, &[]);

	let emitted = committed_deltas(&mut editor);
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
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Definition(_)))
		.map(|(id, _)| *id)
		.expect("the group should name a table entry");

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor
		.active_document_mut()
		.network_interface
		.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);

	let emitted = committed_deltas(&mut editor);
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
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Definition(_)))
		.map(|(id, _)| *id)
		.expect("the group should name a table entry");

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor.active_document_mut().network_interface.set_display_name(&group, "Renamed".to_string(), &[]);
	editor
		.active_document_mut()
		.network_interface
		.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);

	let emitted = committed_deltas(&mut editor);
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

	let edited = &editor.active_document().network_interface;
	assert_eq!(
		replayed.document_network(),
		edited.document_network(),
		"replaying {} recorded deltas did not reproduce the graph",
		recorded.len()
	);
	assert_eq!(
		without_selection_history(&replayed.network_metadata.persistent_metadata),
		without_selection_history(&edited.network_metadata.persistent_metadata),
		"replaying {} recorded deltas did not reproduce the metadata",
		recorded.len()
	);
}

/// The metadata without this peer's selection history, which deltas never carry.
fn without_selection_history(metadata: &super::NodeNetworkPersistentMetadata) -> super::NodeNetworkPersistentMetadata {
	let clear = |network: &mut super::NodeNetworkPersistentMetadata| {
		network.selection_undo_history.clear();
		network.selection_redo_history.clear();
	};

	let mut metadata = metadata.clone();
	clear(&mut metadata);
	for definition in metadata.definitions.values_mut() {
		clear(&mut definition.persistent_metadata);
	}
	metadata
}

fn only_group(editor: &EditorTestUtils) -> NodeId {
	editor
		.active_document()
		.network_interface
		.document_network()
		.nodes
		.iter()
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Definition(_)))
		.map(|(id, _)| *id)
		.expect("the group should name a table entry")
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

	let emitted = committed_deltas(&mut editor);
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
		.find(|(_, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Definition(_)))
		.map(|(id, _)| *id)
		.expect("the group should name a table entry");
	let interface = &mut editor.active_document_mut().network_interface;
	interface.delete_nodes(vec![group], true, &[]);
	interface.discard_deltas();
	interface.settle_definitions();
	let entry_removals = interface.take_deltas();

	let network = editor.active_document().network_interface.document_network().clone();
	let mut deltas: Vec<EditorDelta> = before
		.iter()
		.filter(|id| !network.nodes.contains_key(id))
		.map(|id| {
			EditorDelta::Graph(RuntimeDelta::RemoveNode {
				network: OwnedNetworkLocation::ROOT,
				node_id: *id,
			})
		})
		.collect();
	for (index, export) in network.exports.iter().enumerate() {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
			network: OwnedNetworkLocation::ROOT,
			export_index: index,
			input: Some(export.clone()),
		}));
	}
	for (node_id, node) in &network.nodes {
		for (index, input) in node.inputs.iter().enumerate() {
			deltas.push(EditorDelta::Graph(RuntimeDelta::SetInput {
				network: OwnedNetworkLocation::ROOT,
				node_id: *node_id,
				input_index: index,
				input: input.clone(),
			}));
		}
	}
	deltas.extend(entry_removals);

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
	let interface = &mut editor.active_document_mut().network_interface;
	interface.delete_nodes(vec![group], true, &[]);
	interface.settle_definitions();

	let emitted = interface.take_deltas();
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
		cursor.replace_implementation(graph_craft::document::DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER));
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

	// The slots that survive the swap keep the names and descriptions the registry holds for them.
	let Some((id, inputs)) = ops.iter().find_map(|op| match op {
		RegistryDelta::SetNodeInputs { id, inputs } => Some((id, inputs)),
		_ => None,
	}) else {
		panic!("the swap should restate the slots")
	};
	let held: Vec<_> = working.node_instances[id]
		.inputs()
		.iter()
		.take(inputs.len())
		.flat_map(|slot| slot.attributes.live().filter(|(key, _)| key.starts_with("ui::")))
		.collect();
	assert!(!held.is_empty(), "the surviving slots should hold ui attributes for this to check anything");
	for (index, slot) in inputs.iter().enumerate() {
		for (key, value) in working.node_instances[id].inputs()[index].attributes.live().filter(|(key, _)| key.starts_with("ui::")) {
			assert_eq!(slot.attributes.get(key), Some(value), "slot {index} lost {key}");
		}
	}
}

fn conversion(editor: &EditorTestUtils) -> document_graph_storage::RuntimeConversion {
	let document = editor.active_document();
	Registry::convert_from_runtime(
		document.network_interface.document_network(),
		&StorageMetadataView::new(&document.network_interface),
		&document.resources.registry,
		PEER,
	)
	.expect("conversion should succeed")
}

/// Sends the deltas `edit` records to a peer as storage ops, reconciles the touched entities into a pre-edit clone of the
/// interface, and checks that it equals the edited interface and converts back to the peer's registry.
async fn assert_remote_reconcile_reproduces(edit: impl FnOnce(&mut EditorTestUtils)) {
	use document_graph_storage::Touched;

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor
		.handle_message(DocumentMessage::GroupSelectedLayers {
			group_folder_type: crate::messages::portfolio::document::utility_types::misc::GroupFolderType::Layer,
		})
		.await;
	editor.active_document_mut().network_interface.discard_deltas();
	editor.active_document_mut().network_interface.pin_storage_identities(PEER);

	let mut mirror = editor.active_document().network_interface.clone();
	let baseline = conversion(&editor);
	let mut peer = Session::with_peer(PeerId(8));
	peer.stage_computed_ops(compute_deltas(&Registry::default(), &baseline.registry)).expect("the peer takes the baseline");
	let mut declarations = baseline.declarations;

	edit(&mut editor);
	editor.active_document_mut().network_interface.settle_definitions();
	editor.active_document_mut().network_interface.pin_storage_identities(PEER);

	let recorded = editor.active_document_mut().network_interface.take_deltas();
	assert!(!recorded.is_empty(), "the edit should have recorded something to send");
	let constructed = {
		let document = editor.active_document();
		construct_batch(
			&recorded,
			&baseline.registry,
			&document.resources.registry,
			&StorageMetadataView::new(&document.network_interface),
			PEER,
		)
		.expect("construction")
	};
	declarations.extend(constructed.declarations.decoded);

	let mut touched = Touched::default();
	for op in &constructed.ops {
		touched.record(op);
	}
	peer.stage_computed_ops(constructed.ops).expect("the peer applies the batch");

	mirror.reconcile_remote(peer.registry(), &declarations, &touched, PEER).expect("reconcile");

	let edited = &editor.active_document().network_interface;
	assert!(
		mirror.document_network() == edited.document_network(),
		"reconciling {} recorded deltas did not reproduce the edited graph:\n{}",
		recorded.len(),
		crate::messages::portfolio::document::document_diff::diff_networks(edited.document_network(), mirror.document_network())
	);
	let metadata = |interface: &super::NodeNetworkInterface| serde_json::to_value(interface.document_network_metadata()).expect("metadata serializes");
	let (reconciled_metadata, edited_metadata) = (metadata(&mirror), metadata(edited));
	assert!(
		reconciled_metadata == edited_metadata,
		"reconciling {} recorded deltas did not reproduce the edited metadata:\n{}",
		recorded.len(),
		json_diff(&edited_metadata, &reconciled_metadata, "")
	);

	let converted = Registry::convert_from_runtime(mirror.document_network(), &StorageMetadataView::new(&mirror), &editor.active_document().resources.registry, PEER)
		.expect("the reconciled interface converts")
		.registry;
	assert!(
		peer.registry().value_equal(&converted),
		"the reconciled interface must convert back to the registry it was reconciled from\nresidual: {:#?}",
		compute_deltas(&converted, peer.registry())
	);
}

/// The paths at which two JSON values differ, one per line.
fn json_diff(expected: &serde_json::Value, actual: &serde_json::Value, path: &str) -> String {
	use serde_json::Value;
	match (expected, actual) {
		(Value::Object(expected), Value::Object(actual)) => {
			let keys: std::collections::BTreeSet<_> = expected.keys().chain(actual.keys()).collect();
			keys.into_iter()
				.map(|key| match (expected.get(key), actual.get(key)) {
					(Some(expected), Some(actual)) => json_diff(expected, actual, &format!("{path}/{key}")),
					(Some(expected), None) => format!("{path}/{key}: missing, expected {expected}\n"),
					(None, Some(actual)) => format!("{path}/{key}: unexpected {actual}\n"),
					(None, None) => String::new(),
				})
				.collect()
		}
		(Value::Array(expected), Value::Array(actual)) if expected.len() == actual.len() => expected
			.iter()
			.zip(actual)
			.enumerate()
			.map(|(index, (expected, actual))| json_diff(expected, actual, &format!("{path}[{index}]")))
			.collect(),
		_ if expected == actual => String::new(),
		_ => format!("{path}: expected {expected}\n{path}: actual   {actual}\n"),
	}
}

fn rectangle_template() -> NodeTemplate {
	crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(&rectangle_definition())
		.expect("rectangle definition")
		.default_node_template()
}

#[tokio::test]
async fn reconciling_metadata_edits_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
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
async fn reconciling_an_arity_change_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		editor
			.active_document_mut()
			.network_interface
			.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);
	})
	.await;
}

#[tokio::test]
async fn reconciling_a_node_insertion_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		editor.active_document_mut().network_interface.insert_node(NodeId(0xDE17A), rectangle_template(), &[]);
	})
	.await;
}

#[tokio::test]
async fn reconciling_a_node_insertion_inside_a_group_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		editor.active_document_mut().network_interface.insert_node(NodeId(0xDE17B), rectangle_template(), &[group]);
	})
	.await;
}

#[tokio::test]
async fn reconciling_a_new_instance_of_a_shared_network_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		insert_shared_copy(editor, group);
	})
	.await;
}

#[tokio::test]
async fn reconciling_an_edit_inside_a_shared_network_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		insert_shared_copy(editor, group);
		editor
			.active_document_mut()
			.network_interface
			.add_import(TaggedValue::Number(7.), true, -1, "Added", "An added import", &[group]);
	})
	.await;
}

#[tokio::test]
async fn reconciling_a_group_removal_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		editor.active_document_mut().network_interface.delete_nodes(vec![group], true, &[]);
	})
	.await;
}

#[tokio::test]
async fn reconciling_an_implementation_swap_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		let interface = &mut editor.active_document_mut().network_interface;
		let Some(mut cursor) = interface.node_mut(super::NodeLocator::new(group, &[])) else {
			panic!("the group should resolve")
		};
		cursor.replace_implementation(graph_craft::document::DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER));
	})
	.await;
}

#[tokio::test]
async fn reconciling_an_input_value_edit_reproduces_the_interface() {
	assert_remote_reconcile_reproduces(|editor| {
		let group = only_group(editor);
		let interface = &mut editor.active_document_mut().network_interface;
		let (inner, index) = interface
			.nested_network(&[group])
			.expect("the group's network")
			.nodes
			.iter()
			.find_map(|(id, node)| Some((*id, node.inputs.iter().position(|input| input.as_value().is_some())?)))
			.expect("a node with a value input inside the group");
		interface.set_input(&InputConnector::node_at_index(inner, index), NodeInput::value(TaggedValue::Number(42.), false), &[group]);
	})
	.await;
}

/// A touched node the registry holds unchanged, as a late-writer-wins loser leaves it, must not disturb the interface.
#[tokio::test]
async fn reconciling_an_untouched_node_changes_nothing() {
	use document_graph_storage::Touched;

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	editor.active_document_mut().network_interface.pin_storage_identities(PEER);

	let baseline = conversion(&editor);
	let mut touched = Touched::default();
	touched.nodes.extend(baseline.registry.node_instances.keys().copied());
	touched.networks.extend(baseline.registry.networks.keys().copied());

	let before = editor.active_document().network_interface.clone();
	let reconciled = editor
		.active_document_mut()
		.network_interface
		.reconcile_remote(&baseline.registry, &baseline.declarations, &touched, PEER)
		.expect("reconcile");

	assert!(reconciled.networks.is_empty(), "nothing differed, so nothing should have been applied: {:?}", reconciled.networks);
	assert_eq!(editor.active_document().network_interface, before);
}

/// A `ui::` key the registry already holds as deleted is not cleared again on every snapshot.
#[test]
fn a_deleted_ui_key_is_not_cleared_again() {
	use document_graph_storage::{AttributeValue, Attributes, TimeStamp, Value};
	let at = TimeStamp { counter: 3, peer: PeerId(1) };
	let current: Attributes = [
		("ui::name".to_string(), AttributeValue::deleted(at)),
		("ui::locked".to_string(), AttributeValue::new(Value::Bool(true), at)),
	]
	.into_iter()
	.collect();
	let writes = ui_attribute_writes(Some(&current), &Attributes::new());
	assert_eq!(writes.iter().map(|write| write.key.as_str()).collect::<Vec<_>>(), ["ui::locked"]);
}

fn merge_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::Network("Merge".to_string())
}

/// A subgraph's output descriptions are addressed by export index, so each has to move with its export as exports are added, reordered, and removed.
#[tokio::test]
async fn output_descriptions_follow_their_exports() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let merge = editor.create_node_by_name(merge_definition()).await;

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.add_export(TaggedValue::None, -1, "", &[merge]);
		network_interface.add_export(TaggedValue::None, -1, "", &[merge]);

		let mut node = network_interface.node_mut(super::NodeLocator::new(merge, &[])).expect("Merge should exist");
		node.set_output_descriptions(["First.", "Second.", "Third."].map(String::from).to_vec());
	}
	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.reorder_export(2, 0, &[merge]);
		network_interface.remove_export(1, &[merge]);
	}

	let network_interface = &editor.active_document().network_interface;
	let descriptions = (0..network_interface.number_of_exports(&[merge]))
		.map(|index| network_interface.output_description(&merge, index, &[]))
		.collect::<Vec<_>>();
	assert_eq!(descriptions, ["Third.", "Second."], "the descriptions should have moved with their exports");

	let emitted = committed_deltas(&mut editor);
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "moved output descriptions");
}

/// A subgraph whose output descriptions are all empty stores none, so adding or removing an export leaves nothing to write for them.
#[tokio::test]
async fn exports_of_a_subgraph_without_descriptions_emit_no_description_write() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let merge = editor.create_node_by_name(merge_definition()).await;
	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		let mut node = network_interface.node_mut(super::NodeLocator::new(merge, &[])).expect("Merge should exist");
		node.set_output_descriptions(Vec::new());
	}
	editor.active_document_mut().network_interface.discard_deltas();

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.add_export(TaggedValue::None, -1, "", &[merge]);
		let last_export = network_interface.number_of_exports(&[merge]) - 1;
		network_interface.remove_export(last_export, &[merge]);
	}

	let emitted = committed_deltas(&mut editor);
	let description_writes = emitted
		.iter()
		.filter(|delta| {
			matches!(
				delta,
				EditorDelta::NodeMetadata {
					change: NodeMetadataChange::OutputDescriptions(_),
					..
				}
			)
		})
		.count();
	assert_eq!(description_writes, 0, "no descriptions are stored, so none should be written: {emitted:#?}");
}

/// Storage keeps a subgraph's whole description list once any entry says something, so an export added past the described ones still changes what's stored.
#[tokio::test]
async fn adding_an_export_past_the_described_ones_matches_the_diff() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let merge = editor.create_node_by_name(merge_definition()).await;

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor.active_document_mut().network_interface.add_export(TaggedValue::None, -1, "", &[merge]);

	let emitted = committed_deltas(&mut editor);
	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "an export added past the described ones");
}

/// Adding or removing an export past the end of a shorter description list leaves the list as it was, so no description write is recorded.
#[tokio::test]
async fn exports_past_the_end_of_the_description_list_emit_no_description_write() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let merge = editor.create_node_by_name(merge_definition()).await;
	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.add_export(TaggedValue::None, -1, "", &[merge]);
		let mut node = network_interface.node_mut(super::NodeLocator::new(merge, &[])).expect("Merge should exist");
		node.set_output_descriptions(vec!["Described.".to_string()]);
	}
	editor.active_document_mut().network_interface.discard_deltas();

	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.add_export(TaggedValue::None, -1, "", &[merge]);
		let last_export = network_interface.number_of_exports(&[merge]) - 1;
		network_interface.remove_export(last_export, &[merge]);
	}

	let emitted = committed_deltas(&mut editor);
	let description_writes = emitted
		.iter()
		.filter(|delta| {
			matches!(
				delta,
				EditorDelta::NodeMetadata {
					change: NodeMetadataChange::OutputDescriptions(_),
					..
				}
			)
		})
		.count();
	assert_eq!(description_writes, 0, "the description list didn't change, so it shouldn't be written: {emitted:#?}");
}

fn table_layer(editor: &EditorTestUtils) -> (NodeId, graph_craft::document::NetworkId) {
	editor
		.active_document()
		.network_interface
		.document_network()
		.nodes
		.iter()
		.find_map(|(id, node)| match node.implementation {
			graph_craft::document::DocumentNodeImplementation::Definition(definition) => Some((*id, definition)),
			_ => None,
		})
		.expect("a committed layer should name a table entry")
}

/// A node inside the table entry `definition`, and the index of one of its value inputs.
fn value_input_inside(editor: &EditorTestUtils, definition: graph_craft::document::NetworkId) -> (NodeId, usize) {
	editor.active_document().network_interface.document_network().definitions[&definition]
		.nodes
		.iter()
		.find_map(|(id, node)| node.inputs.iter().position(|input| matches!(input, NodeInput::Value { .. })).map(|index| (*id, index)))
		.expect("the layer's network should have a node with a value input")
}

/// The network location `deltas` recorded for a write to an input of `node_id`.
fn recorded_input_write(deltas: &[EditorDelta], node_id: NodeId) -> OwnedNetworkLocation {
	deltas
		.iter()
		.find_map(|delta| match delta {
			EditorDelta::Graph(RuntimeDelta::SetInput { network, node_id: written, .. }) if *written == node_id => Some(network.clone()),
			_ => None,
		})
		.expect("the input write should be recorded")
}

/// An inserted network goes into the definitions table right away, and a commit drops the entries no node reaches any more.
#[tokio::test]
async fn inserted_networks_are_lifted_and_unreached_entries_dropped() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let network = editor.active_document().network_interface.document_network();
	let inline_remains = std::iter::once(network)
		.chain(network.definitions.values())
		.flat_map(|network| network.nodes.values())
		.any(|node| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Network(_)));
	assert!(!inline_remains, "the insertion should have lifted every inline network");
	assert!(network.definitions.contains_key(&definition));

	let interface = &mut editor.active_document_mut().network_interface;
	interface.delete_nodes(vec![layer], true, &[]);
	interface.settle_definitions();

	let network = editor.active_document().network_interface.document_network();
	assert!(!network.definitions.contains_key(&definition), "the deleted layer's entry should be gone");
	assert!(editor.active_document().network_interface.validate_invariants().is_empty());
}

/// An edit inside a table entry stages incrementally, onto the same stored network a whole-document conversion produces.
#[tokio::test]
async fn an_edit_inside_a_table_entry_constructs_the_diff() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let (inner, input_index) = value_input_inside(&editor, definition);

	let working = convert(&editor);
	editor.active_document_mut().network_interface.discard_deltas();

	editor
		.active_document_mut()
		.network_interface
		.set_input(&InputConnector::node_at_index(inner, input_index), NodeInput::value(TaggedValue::None, false), &[layer]);

	let emitted = committed_deltas(&mut editor);

	let constructed = construct(&editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "edit inside a table entry");
}

/// A template of a node that names a table entry names it too, so a copy shares the entry.
#[tokio::test]
async fn a_template_of_a_table_node_names_its_entry() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let template = editor.active_document().network_interface.create_node_template(&layer, &[]).expect("template");
	let (node, _) = template.into_parts();
	assert_eq!(node.implementation, graph_craft::document::DocumentNodeImplementation::Definition(definition));
}

/// An inline copy of `node_id` without its catalog reference, so inserting it lifts a new entry.
fn inline_copy(interface: &super::NodeNetworkInterface, node_id: NodeId) -> super::NodeTemplate {
	let mut node = interface.document_network().nodes[&node_id].clone();
	let mut metadata = interface.network_metadata.persistent_metadata.node_metadata[&node_id].persistent_metadata.clone();
	let graph_craft::document::DocumentNodeImplementation::Definition(id) = node.implementation else {
		panic!("the node should name a table entry")
	};

	let mut network_metadata = interface.network_metadata.persistent_metadata.definitions[&id].clone();
	network_metadata.persistent_metadata.reference = None;
	node.implementation = graph_craft::document::DocumentNodeImplementation::Network(interface.document_network().definitions[&id].clone());
	metadata.network_metadata = Some(network_metadata);
	super::NodeTemplate::from_parts(node, metadata)
}

/// A write inside an instance stays with the network it was made in, even when the instance changes implementation later in the same batch.
#[tokio::test]
async fn a_write_keeps_its_network_when_the_instance_changes_implementation() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let (inner, input_index) = value_input_inside(&editor, definition);
	let interface = &mut editor.active_document_mut().network_interface;
	interface.discard_deltas();

	interface.set_input(&InputConnector::node_at_index(inner, input_index), NodeInput::value(TaggedValue::None, false), &[layer]);
	let graph_craft::document::DocumentNodeImplementation::Definition(written) = interface.document_network().nodes[&layer].implementation else {
		panic!("the layer should name a table entry")
	};
	let mut inline_copy = inline_copy(interface, layer);
	interface.replace_implementation(&layer, &[], &mut inline_copy);

	let deltas = interface.take_deltas();
	assert_eq!(recorded_input_write(&deltas, inner), OwnedNetworkLocation::definition(written));
}

/// A copy of `layer` that names the same table entry, so the two share one network.
fn insert_shared_copy(editor: &mut EditorTestUtils, layer: NodeId) -> NodeId {
	let interface = &mut editor.active_document_mut().network_interface;
	let node = interface.document_network().nodes[&layer].clone();
	let metadata = interface.network_metadata.persistent_metadata.node_metadata[&layer].persistent_metadata.clone();

	let copy = NodeId::new();
	interface.insert_node_entry(super::NodeLocator::new(copy, &[]), super::NodeTemplate::from_parts(node, metadata));
	interface.unload_outward_wires(&[]);
	interface.settle_definitions();
	copy
}

/// Stages what `edit` records against the document before it, and checks the result against a whole-document diff.
fn assert_stages_incrementally(editor: &mut EditorTestUtils, at: &str, edit: impl FnOnce(&mut EditorTestUtils)) -> Vec<EditorDelta> {
	let working = convert(editor);
	editor.active_document_mut().network_interface.discard_deltas();

	edit(editor);
	let emitted = committed_deltas(editor);

	let constructed = construct(editor, &emitted, &working);
	let diffed = compute_deltas(&working, &convert(editor));
	assert_same_stored_effect(&working, constructed, diffed, at);
	let violations = editor.active_document().network_interface.validate_invariants();
	assert!(violations.is_empty(), "{at}: {violations:#?}");
	emitted
}

/// Inserting a node that brings a nested network stages the new table entry and the node naming it.
#[tokio::test]
async fn inserting_a_nested_network_stages_incrementally() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, _) = table_layer(&editor);
	let inline_copy = inline_copy(&editor.active_document().network_interface, layer);
	let emitted = assert_stages_incrementally(&mut editor, "insert a nested network", |editor| {
		let interface = &mut editor.active_document_mut().network_interface;
		interface.insert_node_entry(super::NodeLocator::new(NodeId::new(), &[]), inline_copy);
		interface.unload_outward_wires(&[]);
	});
	assert!(emitted.iter().any(|delta| matches!(delta, EditorDelta::Graph(RuntimeDelta::AddNetwork { .. }))));
}

/// Deleting one of two instances of a shared network keeps the network for the other.
#[tokio::test]
async fn deleting_one_instance_of_a_shared_network_stages_incrementally() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let copy = insert_shared_copy(&mut editor, layer);
	let emitted = assert_stages_incrementally(&mut editor, "delete one shared instance", |editor| {
		editor.active_document_mut().network_interface.delete_nodes(vec![copy], true, &[]);
	});

	assert!(!emitted.iter().any(|delta| matches!(delta, EditorDelta::Graph(RuntimeDelta::RemoveNetwork { .. }))));
	assert!(editor.active_document().network_interface.document_network().definitions.contains_key(&definition));
}

/// Swapping a network for a proto node moves the interface onto the node, which the stored slots then hold.
#[tokio::test]
async fn swapping_a_network_for_a_proto_node_stages_its_interface() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, _) = table_layer(&editor);
	assert_stages_incrementally(&mut editor, "swap a network for a proto node", |editor| {
		let interface = &mut editor.active_document_mut().network_interface;
		let Some(mut cursor) = interface.node_mut(super::NodeLocator::new(layer, &[])) else {
			panic!("the layer should resolve")
		};
		cursor.replace_implementation(graph_craft::document::DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER));
	});
}

/// Deleting the last instance of a network drops its table entry along with it.
#[tokio::test]
async fn deleting_the_last_instance_drops_the_entry_incrementally() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let emitted = assert_stages_incrementally(&mut editor, "delete the last instance", |editor| {
		editor.active_document_mut().network_interface.delete_nodes(vec![layer], true, &[]);
	});

	assert!(emitted.iter().any(|delta| matches!(delta, EditorDelta::Graph(RuntimeDelta::RemoveNetwork { id }) if *id == definition)));
}

#[tokio::test]
async fn replaying_an_insertion_that_lifts_reproduces_the_interface() {
	assert_replay_reproduces(|editor| {
		let group = only_group(editor);
		let interface = &mut editor.active_document_mut().network_interface;
		let inline_copy = inline_copy(interface, group);
		interface.insert_node_entry(super::NodeLocator::new(NodeId(0x11F7), &[]), inline_copy);
	})
	.await;
}

#[tokio::test]
async fn replaying_the_removal_of_a_table_entry_reproduces_the_interface() {
	assert_replay_reproduces(|editor| {
		let group = only_group(editor);
		let interface = &mut editor.active_document_mut().network_interface;
		interface.delete_nodes(vec![group], true, &[]);
		interface.settle_definitions();
	})
	.await;
}

/// A table entry whose export holds a resource stages that resource with the entry, and drops it with the entry.
#[tokio::test]
async fn a_resource_an_entry_exports_follows_the_entry() {
	use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let resource = ResourceId::new();
	let registry = &mut editor.active_document_mut().resources.registry;
	registry.push_source_back(&resource, DataSource::Embedded);
	registry.resolve(&resource, ResourceHash::from(&b"exported bytes"[..]));

	let (layer, _) = table_layer(&editor);
	let template = inline_copy(&editor.active_document().network_interface, layer);
	let (mut node, metadata) = template.into_parts();
	let graph_craft::document::DocumentNodeImplementation::Network(network) = &mut node.implementation else {
		panic!("the template should carry the layer's network inline")
	};
	network.exports[0] = NodeInput::value(TaggedValue::Resource(resource), false);
	let exporting = super::NodeTemplate::from_parts(node, metadata);

	let copy = NodeId::new();
	let added = assert_stages_incrementally(&mut editor, "add an entry exporting a resource", |editor| {
		let interface = &mut editor.active_document_mut().network_interface;
		interface.insert_node_entry(super::NodeLocator::new(copy, &[]), exporting);
		interface.unload_outward_wires(&[]);
	});
	assert!(added.iter().any(|delta| matches!(delta, EditorDelta::Graph(RuntimeDelta::AddNetwork { .. }))));

	assert_stages_incrementally(&mut editor, "remove the entry exporting a resource", |editor| {
		editor.active_document_mut().network_interface.delete_nodes(vec![copy], true, &[]);
	});
}

/// A resource written into an entry's export goes again when the same batch removes the entry.
#[tokio::test]
async fn a_resource_exported_and_removed_in_one_batch_leaves_nothing_behind() {
	use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let resource = ResourceId::new();
	let registry = &mut editor.active_document_mut().resources.registry;
	registry.push_source_back(&resource, DataSource::Embedded);
	registry.resolve(&resource, ResourceHash::from(&b"exported bytes"[..]));

	let (layer, _) = table_layer(&editor);
	assert_stages_incrementally(&mut editor, "export a resource, then remove the entry", |editor| {
		let interface = &mut editor.active_document_mut().network_interface;
		interface.set_input(&InputConnector::Export(0), NodeInput::value(TaggedValue::Resource(resource), false), &[layer]);
		interface.delete_nodes(vec![layer], true, &[]);
	});
}

/// Making one of two instances unique gives it its own copy of the network, so an edit inside it no longer reaches the other.
#[tokio::test]
async fn making_an_instance_unique_forks_its_network() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, definition) = table_layer(&editor);
	let copy = insert_shared_copy(&mut editor, layer);
	assert!(editor.active_document().network_interface.is_shared(&copy, &[]));

	assert_stages_incrementally(&mut editor, "make an instance unique", |editor| {
		assert!(editor.active_document_mut().network_interface.make_unique(&copy, &[]));
	});

	let interface = &editor.active_document().network_interface;
	let graph_craft::document::DocumentNodeImplementation::Definition(forked) = interface.document_network().nodes[&copy].implementation else {
		panic!("the copy should still name a table entry")
	};
	assert_ne!(forked, definition);
	assert!(!interface.is_shared(&copy, &[]) && !interface.is_shared(&layer, &[]));

	let (inner, input_index) = value_input_inside(&editor, forked);
	let original = editor.active_document().network_interface.document_network().definitions[&definition].clone();
	let interface = &mut editor.active_document_mut().network_interface;
	let NodeInput::Value { tagged_value, exposed } = &interface.document_network().definitions[&forked].nodes[&inner].inputs[input_index] else {
		unreachable!("the input was found as a value")
	};
	let flipped = NodeInput::value((**tagged_value).clone(), !exposed);
	interface.set_input(&InputConnector::node_at_index(inner, input_index), flipped, &[copy]);

	assert_ne!(interface.document_network().definitions[&forked], original, "the edit should change the copy's network");
	assert_eq!(interface.document_network().definitions[&definition], original, "the original's network should be untouched");
}

/// Carried subgraphs that name one another in a cycle are refused, since inserting them would never finish.
#[tokio::test]
async fn a_cyclic_paste_is_refused() {
	use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkId, NodeNetwork};

	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let (first, second) = (NetworkId::new(), NetworkId::new());
	let naming = |id| super::CopiedDefinition {
		id: if id == first { second } else { first },
		network: NodeNetwork {
			nodes: [(
				NodeId(0),
				DocumentNode {
					implementation: DocumentNodeImplementation::Definition(id),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		},
		metadata: Default::default(),
	};

	let interface = &mut editor.active_document_mut().network_interface;
	assert!(interface.carry_definitions(vec![naming(first), naming(second)]).is_none());
}

/// Making an instance unique marks the open transaction as modified, so the transaction commits rather than cancels.
#[tokio::test]
async fn making_an_instance_unique_modifies_the_transaction() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;

	let (layer, _) = table_layer(&editor);
	let copy = insert_shared_copy(&mut editor, layer);

	let interface = &mut editor.active_document_mut().network_interface;
	interface.start_transaction();
	assert!(interface.make_unique(&copy, &[]));
	assert_eq!(interface.transaction_status(), super::TransactionStatus::Modified);
}
