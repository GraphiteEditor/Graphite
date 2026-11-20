use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
use crate::test_utils::test_prelude::*;
use graphene_std::graphic::extend;

fn rectangle() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
}

// A layer without an Extend node gets its path written onto its whole output
#[tokio::test]
async fn a_layer_gets_click_targets() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let rectangle = editor.create_node_by_name_at(rectangle(), 0, 0).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(rectangle, 0),
			input_connector: InputConnector::Export(0),
		})
		.await;
	assert!(editor.active_document().metadata().click_targets.is_empty(), "A node not shown as a layer should have no click targets");

	editor.handle_message(NodeGraphMessage::SetToNodeOrLayer { node_id: rectangle, is_layer: true }).await;

	let document = editor.active_document();
	let layer = LayerNodeIdentifier::new(rectangle, &document.network_interface);
	let click_targets = document.metadata().click_targets(layer);
	assert!(
		click_targets.is_some_and(|click_targets| !click_targets.is_empty()),
		"A node shown as a layer should have click targets"
	);
}

// A layer that is an Extend node gets its path written onto what it adds
#[tokio::test]
async fn an_extend_node_layer_gets_click_targets() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let extend = editor.create_node_by_name_at(DefinitionIdentifier::ProtoNode(extend::IDENTIFIER), 0, 0).await;
	let (below, added) = (editor.create_node_by_name_at(rectangle(), -10, 0).await, editor.create_node_by_name_at(rectangle(), -10, 4).await);
	for (rectangle, input) in [(below, InputConnector::node(extend, extend::BaseInput)), (added, InputConnector::node(extend, extend::NewInput))] {
		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(rectangle, 0),
				input_connector: input,
			})
			.await;
	}
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(extend, 0),
			input_connector: InputConnector::Export(0),
		})
		.await;

	editor.handle_message(NodeGraphMessage::SetToNodeOrLayer { node_id: extend, is_layer: true }).await;

	let document = editor.active_document();
	let layer = LayerNodeIdentifier::new(extend, &document.network_interface);
	let click_targets = document.metadata().click_targets(layer);
	assert!(
		click_targets.is_some_and(|click_targets| !click_targets.is_empty()),
		"An Extend node shown as a layer should have click targets for what it adds"
	);
}
