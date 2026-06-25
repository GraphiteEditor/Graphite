use super::line_shape::{LineEnd, generate_line};
use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::consts::BOUNDS_SELECT_THRESHOLD;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
pub use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::snapping::SnapData;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Arrow;

impl Arrow {
	pub fn create_node(shaft_width: f64, head_width: f64, head_length: f64) -> NodeTemplate {
		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Arrow node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false)), // arrow_to
			Some(NodeInput::value(TaggedValue::F64(shaft_width), false)),   // shaft_width
			Some(NodeInput::value(TaggedValue::F64(head_width), false)),    // head_width
			Some(NodeInput::value(TaggedValue::F64(head_length), false)),   // head_length
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, snap_angle, lock_angle] = modifier;

		tool_data.line_data.drag_current = input.mouse.position;

		let keyboard = &input.keyboard;
		let ignore = [layer];
		let snap_data = SnapData::ignore(document, input, viewport, &ignore);
		let mut document_points = generate_line(tool_data, snap_data, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center));

		if tool_data.line_data.dragging_endpoint == Some(LineEnd::Start) {
			document_points.swap(0, 1);
		}

		let arrow_to = document_points[1] - document_points[0];

		if arrow_to.length() < 1e-6 {
			return;
		}

		let Some(node_id) = graph_modification_utils::get_arrow_id(layer, &document.network_interface) else {
			return;
		};

		let document_to_viewport = document.metadata().document_to_viewport;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(arrow_to), false),
		});
		let downstream = document.metadata().downstream_transform_to_viewport(layer);
		let scope = downstream.inverse() * document_to_viewport;
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_translation(document_points[0]),
			transform_in: TransformIn::Scope { scope },
			skip_rerender: false,
		});

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(document: &DocumentMessageHandler, shape_tool_data: &mut ShapeToolData, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		let arrow_layers: HashMap<LayerNodeIdentifier, [DVec2; 2]> = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter_map(|layer| {
				let node_inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER))?;
				let Some(&TaggedValue::DVec2(arrow_to)) = node_inputs[1].as_value() else { return None };

				let transform = document.metadata().transform_to_viewport(layer);
				let viewport_start = transform.transform_point2(DVec2::ZERO);
				let viewport_end = transform.transform_point2(arrow_to);

				if !arrow_to.abs_diff_eq(DVec2::ZERO, f64::EPSILON * 1000.) {
					let is_editing = shape_tool_data.line_data.editing_layer == Some(layer);
					for (i, pos) in [viewport_start, viewport_end].into_iter().enumerate() {
						let is_dragged = is_editing && matches!((i, &shape_tool_data.line_data.dragging_endpoint), (0, Some(LineEnd::Start)) | (1, Some(LineEnd::End)));
						if is_dragged || (pos - mouse_position).length_squared() < BOUNDS_SELECT_THRESHOLD.powi(2) {
							overlay_context.hover_manipulator_anchor(pos, is_dragged);
						} else {
							overlay_context.square(pos, Some(6.), None, None);
						}
					}
				}

				Some((layer, [DVec2::ZERO, arrow_to]))
			})
			.collect();

		shape_tool_data.line_data.selected_layers_with_position.extend(arrow_layers);
	}
}

#[cfg(test)]
mod test_arrow {
	use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeType;
	use crate::messages::tool::tool_messages::shape_tool::{ShapeOptionsUpdate, ShapeToolMessage};
	use crate::test_utils::test_prelude::*;
	use graph_craft::document::value::TaggedValue;

	async fn select_arrow(editor: &mut EditorTestUtils) {
		editor.select_tool(ToolType::Shape).await;
		editor
			.handle_message(ToolMessage::Shape(ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Arrow),
			}))
			.await;
	}

	fn get_arrow_to(editor: &EditorTestUtils) -> Option<DVec2> {
		let document = editor.active_document();
		document
			.metadata()
			.all_layers()
			.find_map(|layer| {
				let node_inputs = NodeGraphLayer::new(layer, &document.network_interface)
					.find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER))?;
				let Some(&TaggedValue::DVec2(arrow_to)) = node_inputs[1].as_value() else {
					return None;
				};
				Some(arrow_to)
			})
	}

	#[tokio::test]
	async fn arrow_draw_simple() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		select_arrow(&mut editor).await;
		editor.drag_tool(ToolType::Shape, 0., 0., 100., 0., ModifierKeys::empty()).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);

		let arrow_to = get_arrow_to(&editor).expect("Expected arrow_to value");
		assert!((arrow_to.x - 100.).abs() < 1., "arrow_to.x should be ~100, got {}", arrow_to.x);
		assert!(arrow_to.y.abs() < 1., "arrow_to.y should be ~0, got {}", arrow_to.y);
	}

	#[tokio::test]
	async fn arrow_draw_diagonal() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		select_arrow(&mut editor).await;
		editor.drag_tool(ToolType::Shape, 0., 0., 60., 80., ModifierKeys::empty()).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);

		let arrow_to = get_arrow_to(&editor).expect("Expected arrow_to value");
		let length = arrow_to.length();
		assert!((length - 100.).abs() < 1., "arrow length should be ~100, got {length}");
	}

	#[tokio::test]
	async fn arrow_cancel_rmb() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		select_arrow(&mut editor).await;
		editor.drag_tool_cancel_rmb(ToolType::Shape).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 0, "No layer should be created on RMB cancel");
		assert!(get_arrow_to(&editor).is_none());
	}

	#[tokio::test]
	async fn arrow_snap_angle_shift() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		select_arrow(&mut editor).await;
		editor.drag_tool(ToolType::Shape, 0., 0., 80., 30., ModifierKeys::SHIFT).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);

		let arrow_to = get_arrow_to(&editor).expect("Expected arrow_to value");
		let angle_degrees = arrow_to.angle_to(DVec2::X).to_degrees();
		let nearest_snap = (angle_degrees / 15.).round() * 15.;
		assert!((angle_degrees - nearest_snap).abs() < 1., "Angle should snap to 15° multiple, got {angle_degrees}°");
	}

	#[tokio::test]
	async fn arrow_zero_length_no_layer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		select_arrow(&mut editor).await;
		// Drag start == end: the 1e-6 guard in update_shape should prevent a valid arrow_to
		editor.drag_tool(ToolType::Shape, 50., 50., 50., 50., ModifierKeys::empty()).await;

		// Either no layer created, or arrow_to is zero/near-zero
		if let Some(arrow_to) = get_arrow_to(&editor) {
			assert!(arrow_to.length() < 1e-4, "Zero-length drag should produce no meaningful arrow_to, got {arrow_to:?}");
		}
	}
}
