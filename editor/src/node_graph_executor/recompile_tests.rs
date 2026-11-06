use crate::messages::portfolio::document::utility_types::network_interface::FlowType;
use crate::test_utils::test_prelude::*;
use glam::UVec2;
use graph_craft::document::NodeId;
use graphene_std::application_io::TimingInformation;
use std::collections::HashSet;

async fn compile_and_run(editor: &mut EditorTestUtils) {
	let portfolio = editor.portfolio_message_handler_mut();
	let document_id = portfolio.active_document_id.unwrap();
	let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
	let document = documents.get_mut(&document_id).unwrap();
	executor
		.submit_node_graph_evaluation(document, document_id, UVec2::ONE, 1., TimingInformation::default(), Vec::new(), true, DVec2::ZERO)
		.unwrap();
	editor.runtime.update();
	editor.runtime.run().await;
	editor.dispatch(Message::Wake);
}

/// The proto node IDs compiled for the document node paths that pass `keep`. The executor keeps the nodes an update drops
/// for one more update, so a first update clears those of the test harness's instrumented graph.
async fn compiled_ids(editor: &mut EditorTestUtils, keep: impl Fn(&[NodeId]) -> bool) -> HashSet<NodeId> {
	compile_and_run(editor).await;
	compile_and_run(editor).await;
	let source_map = editor.runtime.executor.tree().source_map();
	source_map.iter().filter(|(path, _)| keep(path)).map(|(_, (id, _))| *id).collect()
}

#[tokio::test]
async fn recompiling_keeps_the_ids_of_an_image_chain() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.create_raster_image(Image::new(16, 16, Color::BLUE), Some((0., 0.))).await;
	let layer = editor.get_selected_layer().await.expect("the dropped image should be selected").to_node();

	// The nodes feeding the image layer's content, which the resource node feeds
	let network_interface = &editor.active_document().network_interface;
	let chain: Vec<_> = network_interface
		.upstream_flow_back_from_nodes(vec![layer], &[], FlowType::HorizontalFlow)
		.filter(|&node| node != layer)
		.collect();
	let in_chain = |path: &[NodeId]| path.iter().any(|node| chain.contains(node));

	let all = compiled_ids(&mut editor, |_| true).await;
	assert_eq!(compiled_ids(&mut editor, |_| true).await, all, "compiling the same document again changed its proto node IDs");
	let image_chain = compiled_ids(&mut editor, in_chain).await;
	assert!(!image_chain.is_empty(), "the image layer has no compiled content");

	// The new layer above takes the image layer as its input, a change downstream of the image
	editor.draw_rect(0., 0., 10., 10.).await;
	assert_eq!(
		compiled_ids(&mut editor, in_chain).await,
		image_chain,
		"an edit downstream of the image changed the image chain's proto node IDs"
	);
}
