use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct QrCode;

impl QrCode {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::qr_code::IDENTIFIER).expect("QR Code node can't be found");
		node_type.node_template_input_override([
			None,
			None,
			Some(NodeInput::value(TaggedValue::Bool(true), false)),
			Some(NodeInput::value(TaggedValue::Number(1.), false)),
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let center = modifier[0];

		// A QR code is always square, so the aspect ratio is locked regardless of the Shift key. This also anchors the
		// square correctly for reverse drags and when Alt centers it on the drag origin.
		let [start, end] = shape_tool_data.data.calculate_circle_points(document, ipp, viewport, center);
		let Some(node_id) = graph_modification_utils::get_qr_code_id(layer, &document.network_interface) else {
			return;
		};

		let side = ((start - end).abs().x / viewport_zoom(document)).max(1.);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::qr_code::SizeInput),
			input: NodeInput::value(TaggedValue::Number(side), false),
		});

		// The QR geometry's origin is its top-left corner, so align the layer to the top-left of the square.
		let top_left = start.min(end);
		responses.add(window_aligned_transform_set(document, layer, top_left, DVec2::ONE));
	}
}

#[cfg(test)]
mod test_qr_code {
	use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeType;
	use crate::messages::tool::tool_messages::shape_tool::ShapeToolMessage;
	pub use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::generator_nodes::qr_code;

	#[derive(Debug, PartialEq)]
	struct ResolvedQrCode {
		size: f64,
		transform: DAffine2,
	}

	async fn get_qr_codes(editor: &mut EditorTestUtils) -> Vec<ResolvedQrCode> {
		let instrumented = match editor.eval_graph_until_finished().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};

		let document = editor.active_document();
		let layers = document.metadata().all_layers();
		layers
			.filter_map(|layer| {
				let node_graph_layer = NodeGraphLayer::new(layer, &document.network_interface);
				let qr_node = node_graph_layer.upstream_node_id_from_protonode(qr_code::IDENTIFIER)?;
				Some(ResolvedQrCode {
					size: instrumented.grab_ranked_input::<qr_code::SizeInput, f64>(&vec![qr_node], &editor.runtime).unwrap(),
					transform: document.metadata().transform_to_document(layer),
				})
			})
			.collect()
	}

	async fn drag_qr_code(start: (f64, f64), end: (f64, f64), modifiers: ModifierKeys) -> Vec<ResolvedQrCode> {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// The tool has to be active before a shape mode change is accepted.
		editor.select_tool(ToolType::Shape).await;
		editor.handle_message(ShapeToolMessage::SetShape { shape: ShapeType::QrCode }).await;
		editor.drag_tool(ToolType::Shape, start.0, start.1, end.0, end.1, modifiers).await;
		get_qr_codes(&mut editor).await
	}

	#[tokio::test]
	async fn qr_code_draw_simple() {
		let qr_codes = drag_qr_code((10., 10.), (19., 0.), ModifierKeys::empty()).await;

		assert_eq!(qr_codes.len(), 1);
		// The drag is 9 wide and 10 tall, so the square takes the larger dimension.
		assert_eq!(
			qr_codes[0],
			ResolvedQrCode {
				size: 10.,
				transform: DAffine2::from_translation(DVec2::new(10., 0.)) // Uses top-left corner
			}
		);
	}

	#[tokio::test]
	async fn qr_code_draw_reverse_drag_anchors_to_start() {
		// Dragging upward and leftward must still anchor the square's top-left to the drag start corner,
		// so the square spans from (-40, -40) to the origin rather than crossing the start point.
		let qr_codes = drag_qr_code((0., 0.), (-30., -40.), ModifierKeys::empty()).await;

		assert_eq!(qr_codes.len(), 1);
		assert_eq!(
			qr_codes[0],
			ResolvedQrCode {
				size: 40.,
				transform: DAffine2::from_translation(DVec2::new(-40., -40.))
			}
		);
	}

	#[tokio::test]
	async fn qr_code_draw_centered_with_alt() {
		// Alt centers the square on the drag origin, so the drag distance is the half-side.
		let qr_codes = drag_qr_code((0., 0.), (40., 0.), ModifierKeys::ALT).await;

		assert_eq!(qr_codes.len(), 1);
		assert_eq!(
			qr_codes[0],
			ResolvedQrCode {
				size: 80.,
				transform: DAffine2::from_translation(DVec2::new(-40., -40.))
			}
		);
	}

	#[tokio::test]
	async fn probe_mode_survives_a_second_tool_selection() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.select_tool(ToolType::Shape).await;
		editor.handle_message(ShapeToolMessage::SetShape { shape: ShapeType::QrCode }).await;
		// drag_tool_cancel_rmb selects the tool again internally; if that wiped the mode, the cancel test proves nothing
		editor.select_tool(ToolType::Shape).await;
		editor.drag_tool(ToolType::Shape, 10., 10., 60., 60., ModifierKeys::empty()).await;
		assert_eq!(get_qr_codes(&mut editor).await.len(), 1, "PROBE: the mode did not survive a second select_tool");
	}

	#[tokio::test]
	async fn qr_code_draw_cancel() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// The tool has to be active and in this mode before a cancelled drag means anything
		editor.select_tool(ToolType::Shape).await;
		editor.handle_message(ShapeToolMessage::SetShape { shape: ShapeType::QrCode }).await;
		editor.drag_tool_cancel_rmb(ToolType::Shape).await;

		assert!(get_qr_codes(&mut editor).await.is_empty(), "a cancelled drag should leave no QR code behind");
	}
}
