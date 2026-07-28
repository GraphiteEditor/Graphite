use super::InputConnector;
use super::editor_delta::EditorDelta;
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

fn construct(editor: &EditorTestUtils, delta: &EditorDelta, working: &Registry) -> Vec<RegistryDelta> {
	delta
		.to_registry_deltas(working, &editor.active_document().resources.registry, PEER)
		.expect("construction should succeed")
		.ops
}

fn node_metadata_delta(editor: &EditorTestUtils, node_id: NodeId) -> EditorDelta {
	let metadata = editor
		.active_document()
		.network_interface
		.node_metadata(&node_id, &[])
		.expect("node metadata should exist")
		.persistent_metadata
		.clone();
	EditorDelta::NodeMetadata {
		network_path: Vec::new(),
		node_id,
		metadata: Box::new(metadata),
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
	let input = NodeInput::value(TaggedValue::F64(42.), false);
	editor.active_document_mut().network_interface.set_input(&InputConnector::node(node, 1), input.clone(), &[]);

	let delta = EditorDelta::Graph(RuntimeDelta::SetInput {
		network_path: Vec::new(),
		node_id: node,
		input_index: 1,
		input,
	});

	let constructed = construct(&editor, &delta, &working);
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
	editor.active_document_mut().network_interface.set_input(&InputConnector::node(a, 1), wire.clone(), &[]);
	let delta = EditorDelta::Graph(RuntimeDelta::SetInput {
		network_path: Vec::new(),
		node_id: a,
		input_index: 1,
		input: wire,
	});
	let constructed = construct(&editor, &delta, &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_eq!(constructed, diffed, "A wiring edit should construct exactly the diff's op");

	let working = convert(&editor);
	let export = NodeInput::node(a, 0);
	editor.active_document_mut().network_interface.set_input(&InputConnector::Export(0), export.clone(), &[]);
	let delta = EditorDelta::Graph(RuntimeDelta::SetExport {
		network_path: Vec::new(),
		export_index: 0,
		input: export,
	});
	let constructed = construct(&editor, &delta, &working);
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
		EditorDelta::NodeMetadata {
			network_path: Vec::new(),
			node_id,
			metadata: Box::new(metadata),
		},
	];

	let constructed = deltas.iter().flat_map(|delta| construct(&editor, delta, &working)).collect();
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_same_stored_effect(&working, constructed, diffed, "node addition");
}

#[tokio::test]
async fn metadata_edits_construct_the_exact_diff_ops() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let node = editor.create_node_by_name(rectangle_definition()).await;

	let working = convert(&editor);
	{
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_display_name(&node, "Renamed".to_string(), &[]);
		network_interface.set_locked(&node, &[], true);
		network_interface.set_pinned(&node, &[], true);
		network_interface.shift_node(&node, glam::IVec2::new(3, 5), &[]);
	}

	let constructed = construct(&editor, &node_metadata_delta(&editor, node), &working);
	let diffed = compute_deltas(&working, &convert(&editor));
	assert_eq!(constructed, diffed, "Metadata edits should construct exactly the diff's attribute ops");
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
			input: export.clone(),
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

	let without_resource_ops = |ops: Vec<RegistryDelta>| {
		ops.into_iter()
			.filter(|op| !matches!(op, RegistryDelta::RemoveResource { .. } | RegistryDelta::AddResource { .. }))
			.collect::<Vec<_>>()
	};
	let constructed = without_resource_ops(deltas.iter().flat_map(|delta| construct(&editor, delta, &working)).collect());
	let diffed = without_resource_ops(compute_deltas(&working, &convert(&editor)));
	assert_same_stored_effect(&working, constructed, diffed, "nested network removal");
}
