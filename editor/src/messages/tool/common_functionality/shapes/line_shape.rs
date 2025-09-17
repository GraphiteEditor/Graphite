use super::shape_utility::ShapeToolModifierKey;
use crate::consts::{BOUNDS_SELECT_THRESHOLD, LINE_ROTATE_SNAP_ANGLE};
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
pub use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Clone, PartialEq, Debug, Default)]
pub enum LineEnd {
	#[default]
	Start,
	End,
}

#[derive(Clone, Debug, Default)]
pub struct LineToolData {
	pub drag_start: DVec2,
	pub drag_current: DVec2,
	pub angle: f64,
	pub weight: f64,
	pub selected_layers_with_position: HashMap<LayerNodeIdentifier, [DVec2; 2]>,
	pub editing_layer: Option<LayerNodeIdentifier>,
	pub dragging_endpoint: Option<LineEnd>,
}

#[derive(Default)]
pub struct Line;

impl Line {
	pub fn create_node(document: &DocumentMessageHandler, drag_start: DVec2) -> NodeTemplate {
		let node_type = resolve_document_node_type("Line").expect("Line node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::DVec2(document.metadata().document_to_viewport.transform_point2(drag_start)), false)),
			Some(NodeInput::value(TaggedValue::DVec2(document.metadata().document_to_viewport.transform_point2(drag_start)), false)),
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, snap_angle, lock_angle] = modifier;

		shape_tool_data.line_data.drag_current = ipp.mouse.position;

		let keyboard = &ipp.keyboard;
		let ignore = [layer];
		let snap_data = SnapData::ignore(document, ipp, &ignore);
		let mut document_points = generate_line(shape_tool_data, snap_data, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center));

		if shape_tool_data.line_data.dragging_endpoint == Some(LineEnd::Start) {
			document_points.swap(0, 1);
		}

		let Some(node_id) = graph_modification_utils::get_line_id(layer, &document.network_interface) else {
			return;
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(document_points[0]), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::DVec2(document_points[1]), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(document: &DocumentMessageHandler, shape_tool_data: &mut ShapeToolData, overlay_context: &mut OverlayContext) {
		shape_tool_data.line_data.selected_layers_with_position = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter_map(|layer| {
				let node_inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Line")?;

				let (Some(&TaggedValue::DVec2(start)), Some(&TaggedValue::DVec2(end))) = (node_inputs[1].as_value(), node_inputs[2].as_value()) else {
					return None;
				};

				let [viewport_start, viewport_end] = [start, end].map(|point| document.metadata().transform_to_viewport(layer).transform_point2(point));
				if !start.abs_diff_eq(end, f64::EPSILON * 1000.) {
					overlay_context.line(viewport_start, viewport_end, None, None);
					overlay_context.square(viewport_start, Some(6.), None, None);
					overlay_context.square(viewport_end, Some(6.), None, None);
				}

				Some((layer, [start, end]))
			})
			.collect::<HashMap<LayerNodeIdentifier, [DVec2; 2]>>();
	}
}

fn generate_line(tool_data: &mut ShapeToolData, snap_data: SnapData, lock_angle: bool, snap_angle: bool, center: bool) -> [DVec2; 2] {
	let document_to_viewport = snap_data.document.metadata().document_to_viewport;
	let mut document_points = [tool_data.data.drag_start, document_to_viewport.inverse().transform_point2(tool_data.line_data.drag_current)];

	let mut angle = -(document_points[1] - document_points[0]).angle_to(DVec2::X);
	let mut line_length = (document_points[1] - document_points[0]).length();

	if lock_angle {
		angle = tool_data.line_data.angle;
	} else if snap_angle {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;
	}

	tool_data.line_data.angle = angle;

	if lock_angle {
		let angle_vec = DVec2::new(angle.cos(), angle.sin());
		line_length = (document_points[1] - document_points[0]).dot(angle_vec);
	}

	document_points[1] = document_points[0] + line_length * DVec2::new(angle.cos(), angle.sin());

	let constrained = snap_angle || lock_angle;
	let snap = &mut tool_data.data.snap_manager;

	let near_point = SnapCandidatePoint::handle_neighbors(document_points[1], [tool_data.data.drag_start]);
	let far_point = SnapCandidatePoint::handle_neighbors(2. * document_points[0] - document_points[1], [tool_data.data.drag_start]);
	let config = SnapTypeConfiguration::default();

	if constrained {
		let constraint = SnapConstraint::Line {
			origin: document_points[0],
			direction: document_points[1] - document_points[0],
		};
		if center {
			let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
			let snapped_far = snap.constrained_snap(&snap_data, &far_point, constraint, config);
			let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
			document_points[1] = document_points[0] * 2. - best.snapped_point_document;
			document_points[0] = best.snapped_point_document;
			snap.update_indicator(best);
		} else {
			let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
			document_points[1] = snapped.snapped_point_document;
			snap.update_indicator(snapped);
		}
	} else if center {
		let snapped = snap.free_snap(&snap_data, &near_point, config);
		let snapped_far = snap.free_snap(&snap_data, &far_point, config);
		let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
		document_points[1] = document_points[0] * 2. - best.snapped_point_document;
		document_points[0] = best.snapped_point_document;
		snap.update_indicator(best);
	} else {
		let snapped = snap.free_snap(&snap_data, &near_point, config);
		document_points[1] = snapped.snapped_point_document;
		snap.update_indicator(snapped);
	}

	document_points
}

pub fn clicked_on_line_endpoints(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, shape_tool_data: &mut ShapeToolData) -> bool {
	let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Line") else {
		return false;
	};

	let (Some(&TaggedValue::DVec2(document_start)), Some(&TaggedValue::DVec2(document_end))) = (node_inputs[1].as_value(), node_inputs[2].as_value()) else {
		return false;
	};

	let transform = document.metadata().transform_to_viewport(layer);
	let viewport_x = transform.transform_vector2(DVec2::X).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
	let viewport_y = transform.transform_vector2(DVec2::Y).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
	let threshold_x = transform.inverse().transform_vector2(viewport_x).length();
	let threshold_y = transform.inverse().transform_vector2(viewport_y).length();

	let drag_start = input.mouse.position;
	let [start, end] = [document_start, document_end].map(|point| transform.transform_point2(point));

	let start_click = (drag_start.y - start.y).abs() < threshold_y && (drag_start.x - start.x).abs() < threshold_x;
	let end_click = (drag_start.y - end.y).abs() < threshold_y && (drag_start.x - end.x).abs() < threshold_x;

	if start_click || end_click {
		shape_tool_data.line_data.dragging_endpoint = Some(if end_click { LineEnd::End } else { LineEnd::Start });
		shape_tool_data.data.drag_start = if end_click { document_start } else { document_end };
		shape_tool_data.line_data.editing_layer = Some(layer);
		return true;
	}
	false
}

#[cfg(test)]
mod test_line_tool {
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graph_craft::document::value::TaggedValue;

	async fn get_line_node_inputs(editor: &mut EditorTestUtils) -> Option<(DVec2, DVec2)> {
		let document = editor.active_document();
		let network_interface = &document.network_interface;

		network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(network_interface)
			.filter_map(|layer| {
				let node_inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs("Line")?;
				let (Some(&TaggedValue::DVec2(start)), Some(&TaggedValue::DVec2(end))) = (node_inputs[1].as_value(), node_inputs[2].as_value()) else {
					return None;
				};
				Some((start, end))
			})
			.next()
	}

	#[tokio::test]
	async fn test_line_tool_basicdraw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 0., 0., 100., 100., ModifierKeys::empty()).await;
		if let Some((start_input, end_input)) = get_line_node_inputs(&mut editor).await {
			match (start_input, end_input) {
				(start_input, end_input) => {
					assert!((start_input - DVec2::ZERO).length() < 1., "Start point should be near (0,0)");
					assert!((end_input - DVec2::new(100., 100.)).length() < 1., "End point should be near (100,100)");
				}
			}
		}
	}

	#[tokio::test]
	async fn test_line_tool_with_transformed_viewport() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;
		editor.handle_message(NavigationMessage::CanvasPan { delta: DVec2::new(100., 50.) }).await;
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 30_f64.to_radians() }).await;
		editor.drag_tool(ToolType::Line, 0., 0., 100., 100., ModifierKeys::empty()).await;
		if let Some((start_input, end_input)) = get_line_node_inputs(&mut editor).await {
			let document = editor.active_document();
			let document_to_viewport = document.metadata().document_to_viewport;
			let viewport_to_document = document_to_viewport.inverse();

			let expected_start = viewport_to_document.transform_point2(DVec2::ZERO);
			let expected_end = viewport_to_document.transform_point2(DVec2::new(100., 100.));

			assert!(
				(start_input - expected_start).length() < 1.,
				"Start point should match expected document coordinates. Got {start_input:?}, expected {expected_start:?}"
			);
			assert!(
				(end_input - expected_end).length() < 1.,
				"End point should match expected document coordinates. Got {end_input:?}, expected {expected_end:?}"
			);
		} else {
			panic!("Line was not created successfully with transformed viewport");
		}
	}

	#[tokio::test]
	async fn test_line_tool_ctrl_anglelock() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 0., 0., 100., 100., ModifierKeys::CONTROL).await;
		if let Some((start_input, end_input)) = get_line_node_inputs(&mut editor).await {
			let line_vec = end_input - start_input;
			let original_angle = line_vec.angle_to(DVec2::X);
			editor.drag_tool(ToolType::Line, 0., 0., 200., 50., ModifierKeys::CONTROL).await;
			if let Some((updated_start, updated_end)) = get_line_node_inputs(&mut editor).await {
				let updated_line_vec = updated_end - updated_start;
				let updated_angle = updated_line_vec.angle_to(DVec2::X);
				print!("{original_angle:?}");
				print!("{updated_angle:?}");
				assert!(
					line_vec.normalize().dot(updated_line_vec.normalize()).abs() - 1. < 1e-6,
					"Line angle should be locked when Ctrl is kept pressed"
				);
				assert!((updated_start - updated_end).length() > 1., "Line should be able to change length when Ctrl is kept pressed");
			}
		}
	}

	#[tokio::test]
	async fn test_line_tool_alt() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 100., 100., 200., 100., ModifierKeys::ALT).await;
		if let Some((start_input, end_input)) = get_line_node_inputs(&mut editor).await {
			let expected_start = DVec2::new(0., 100.);
			let expected_end = DVec2::new(200., 100.);
			assert!((start_input - expected_start).length() < 1., "Start point should be near (0, 100)");
			assert!((end_input - expected_end).length() < 1., "End point should be near (200, 100)");
		}
	}

	#[tokio::test]
	async fn test_line_tool_alt_shift_drag() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 100., 100., 150., 120., ModifierKeys::ALT | ModifierKeys::SHIFT).await;
		if let Some((start_input, end_input)) = get_line_node_inputs(&mut editor).await {
			match (start_input, end_input) {
				(start_input, end_input) => {
					let line_vec = end_input - start_input;
					let angle_radians = line_vec.angle_to(DVec2::X);
					let angle_degrees = angle_radians.to_degrees();
					let nearest_angle = (angle_degrees / 15.).round() * 15.;

					assert!((angle_degrees - nearest_angle).abs() < 1., "Angle should snap to the nearest 15 degrees");
				}
			}
		}
	}

	#[tokio::test]
	async fn test_line_tool_with_transformed_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 0., 0., 200., 200., ModifierKeys::empty()).await;

		let artboard_id = editor.get_selected_layer().await.expect("Should have selected the artboard");

		editor
			.handle_message(GraphOperationMessage::TransformChange {
				layer: artboard_id,
				transform: DAffine2::from_angle(45_f64.to_radians()),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		editor.drag_tool(ToolType::Line, 50., 50., 150., 150., ModifierKeys::empty()).await;

		let (start_input, end_input) = get_line_node_inputs(&mut editor).await.expect("Line was not created successfully within transformed artboard");
		// The line should still be diagonal with equal change in x and y
		let line_vector = end_input - start_input;
		// Verifying the line is approximately 100*sqrt(2) units in length (diagonal of 100x100 square)
		let line_length = line_vector.length();
		assert!(
			(line_length - 141.42).abs() < 1., // 100 * sqrt(2) ~= 141.42
			"Line length should be approximately 141.42 units. Got: {line_length}"
		);
		assert!((line_vector.x - 100.).abs() < 1., "X-component of line vector should be approximately 100. Got: {}", line_vector.x);
		assert!(
			(line_vector.y.abs() - 100.).abs() < 1.,
			"Absolute Y-component of line vector should be approximately 100. Got: {}",
			line_vector.y.abs()
		);
		let angle_degrees = line_vector.angle_to(DVec2::X).to_degrees();
		assert!((angle_degrees - (-45.)).abs() < 1., "Line angle should be close to -45 degrees. Got: {angle_degrees}");
	}
}
