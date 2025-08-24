use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Ellipse;

impl Ellipse {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_document_node_type("Ellipse").expect("Ellipse node can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(0.5), false)), Some(NodeInput::value(TaggedValue::F64(0.5), false))])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, lock_ratio, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, center, lock_ratio) {
			let Some(node_id) = graph_modification_utils::get_ellipse_id(layer, &document.network_interface) else {
				return;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 1),
				input: NodeInput::value(TaggedValue::F64(((start.x - end.x) / 2.).abs()), false),
			});
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(((start.y - end.y) / 2.).abs()), false),
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_translation(start.midpoint(end)),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
	}
}

#[cfg(test)]
mod test_ellipse {
	pub use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::generator_nodes::ellipse;

	#[derive(Debug, PartialEq)]
	struct ResolvedEllipse {
		radius_x: f64,
		radius_y: f64,
		transform: DAffine2,
	}

	async fn get_ellipse(editor: &mut EditorTestUtils) -> Vec<ResolvedEllipse> {
		let instrumented = match editor.eval_graph().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};

		let document = editor.active_document();
		let layers = document.metadata().all_layers();
		layers
			.filter_map(|layer| {
				let node_graph_layer = NodeGraphLayer::new(layer, &document.network_interface);
				let ellipse_node = node_graph_layer.upstream_node_id_from_protonode(ellipse::IDENTIFIER)?;
				Some(ResolvedEllipse {
					radius_x: instrumented.grab_protonode_input::<ellipse::RadiusXInput>(&vec![ellipse_node], &editor.runtime).unwrap(),
					radius_y: instrumented.grab_protonode_input::<ellipse::RadiusYInput>(&vec![ellipse_node], &editor.runtime).unwrap(),
					transform: document.metadata().transform_to_document(layer),
				})
			})
			.collect()
	}

	#[tokio::test]
	async fn ellipse_draw_simple() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Ellipse, 10., 10., 19., 0., ModifierKeys::empty()).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);

		let ellipse = get_ellipse(&mut editor).await;
		assert_eq!(ellipse.len(), 1);
		assert_eq!(
			ellipse[0],
			ResolvedEllipse {
				radius_x: 4.5,
				radius_y: 5.,
				transform: DAffine2::from_translation(DVec2::new(14.5, 5.)) // Uses center
			}
		);
	}

	#[tokio::test]
	async fn ellipse_draw_circle() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Ellipse, 10., 10., -10., 11., ModifierKeys::SHIFT).await;

		let ellipse = get_ellipse(&mut editor).await;
		assert_eq!(ellipse.len(), 1);
		assert_eq!(
			ellipse[0],
			ResolvedEllipse {
				radius_x: 10.,
				radius_y: 10.,
				transform: DAffine2::from_translation(DVec2::new(0., 20.)) // Uses center
			}
		);
	}

	#[tokio::test]
	async fn ellipse_draw_square_rotated() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				// 45 degree rotation of content clockwise
				angle_radians: f64::consts::FRAC_PI_4,
			})
			.await;
		editor.drag_tool(ToolType::Ellipse, 0., 0., 1., 10., ModifierKeys::SHIFT).await; // Viewport coordinates

		let ellipse = get_ellipse(&mut editor).await;
		assert_eq!(ellipse.len(), 1);
		println!("{ellipse:?}");
		assert_eq!(ellipse[0].radius_x, 5.);
		assert_eq!(ellipse[0].radius_y, 5.);

		assert!(
			ellipse[0]
				.transform
				.abs_diff_eq(DAffine2::from_angle_translation(-f64::consts::FRAC_PI_4, DVec2::X * f64::consts::FRAC_1_SQRT_2 * 10.), 0.001)
		);
	}

	#[tokio::test]
	async fn ellipse_draw_center_square_rotated() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				// 45 degree rotation of content clockwise
				angle_radians: f64::consts::FRAC_PI_4,
			})
			.await;
		editor.drag_tool(ToolType::Ellipse, 0., 0., 1., 10., ModifierKeys::SHIFT | ModifierKeys::ALT).await; // Viewport coordinates

		let ellipse = get_ellipse(&mut editor).await;
		assert_eq!(ellipse.len(), 1);
		assert_eq!(ellipse[0].radius_x, 10.);
		assert_eq!(ellipse[0].radius_y, 10.);
		assert!(ellipse[0].transform.abs_diff_eq(DAffine2::from_angle(-f64::consts::FRAC_PI_4), 0.001));
	}

	#[tokio::test]
	async fn ellipse_cancel() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool_cancel_rmb(ToolType::Ellipse).await;

		let ellipse = get_ellipse(&mut editor).await;
		assert_eq!(ellipse.len(), 0);
	}
}
