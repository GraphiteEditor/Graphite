use super::shape_utility::ShapeToolModifierKey;
use crate::consts::{BOUNDS_SELECT_THRESHOLD, LINE_ROTATE_SNAP_ANGLE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
pub use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::snap_session::{FeatureMap, Locus, PointOptions, SnapSession, SnapView};
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::{DAffine2, DMat2, DVec2};
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
	pub fn create_node() -> NodeTemplate {
		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::line::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Line node can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false))])
	}

	pub fn update_shape(
		view: SnapView,
		snap_session: &mut SnapSession,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let SnapView { document, input: ipp, .. } = view;
		let [center, snap_angle, lock_angle] = modifier;

		shape_tool_data.line_data.drag_current = ipp.mouse.position;

		let keyboard = &ipp.keyboard;
		let mut document_points = generate_line(shape_tool_data, snap_session, view, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center));

		if shape_tool_data.line_data.dragging_endpoint == Some(LineEnd::Start) {
			document_points.swap(0, 1);
		}

		let Some(node_id) = graph_modification_utils::get_line_id(layer, &document.network_interface) else {
			return;
		};

		// Compute line_to in document space
		let line_to = document_points[1] - document_points[0];

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::line::LineToInput),
			input: NodeInput::value(TaggedValue::DVec2(line_to), false),
		});
		let document_to_viewport = document.metadata().document_to_viewport;
		let downstream = document.metadata().downstream_transform_to_viewport(layer);
		let scope = downstream.inverse() * document_to_viewport;
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_translation(document_points[0]),
			transform_in: TransformIn::Scope { scope },
			skip_rerender: false,
		});
	}

	pub fn overlays(document: &DocumentMessageHandler, shape_tool_data: &mut ShapeToolData, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		shape_tool_data.line_data.selected_layers_with_position = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter_map(|layer| {
				let node_inputs =
					NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::line::IDENTIFIER))?;

				let Some(&TaggedValue::DVec2(line_to)) = node_inputs[1].as_value() else {
					return None;
				};

				// Line goes from local origin (0,0) to line_to, positioned by the Transform node
				let transform = document.metadata().transform_to_viewport(layer);
				let viewport_start = transform.transform_point2(DVec2::ZERO);
				let viewport_end = transform.transform_point2(line_to);
				if !line_to.abs_diff_eq(DVec2::ZERO, f64::EPSILON * 1000.) {
					overlay_context.line(viewport_start, viewport_end, None, None);
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

				// Store local-space positions for endpoint editing
				Some((layer, [DVec2::ZERO, line_to]))
			})
			.collect::<HashMap<LayerNodeIdentifier, [DVec2; 2]>>();
	}
}

/// The two ends of the line being drawn, in document space, with the end the pointer carries given last.
pub fn generate_line(tool_data: &mut ShapeToolData, snap_session: &mut SnapSession, view: SnapView, lock_angle: bool, snap_angle: bool, center: bool) -> [DVec2; 2] {
	let document_to_viewport = view.document.metadata().document_to_viewport;
	let start = tool_data.data.drag_start;
	let pointer = document_to_viewport.inverse().transform_point2(tool_data.line_data.drag_current);

	// Locking keeps the angle the line had, and snapping the angle turns it to the nearest step, and either way the end stays on that line
	let direction = if lock_angle {
		Some(DVec2::from_angle(tool_data.line_data.angle))
	} else if snap_angle {
		let step = LINE_ROTATE_SNAP_ANGLE.to_radians();
		Some(DVec2::from_angle(((pointer - start).to_angle() / step).round() * step))
	} else {
		None
	};
	let options = PointOptions {
		previous: Some(start),
		required: direction.and_then(|direction| Locus::line(start, direction)),
		..Default::default()
	};

	let end = if center {
		// The far end mirrors the pointer through where the drag began, and either end may be the one to snap
		let mirrored = FeatureMap {
			matrix: DMat2::from_diagonal(DVec2::splat(-1.)),
			offset: start * 2.,
		};
		snap_session.snap_features(view, &[FeatureMap::IDENTITY, mirrored], 0, pointer, options)
	} else {
		snap_session.snap_point(view, pointer, options)
	};

	if !lock_angle && let Some(drawn) = (end - start).try_normalize() {
		tool_data.line_data.angle = drawn.to_angle();
	}

	if center { [start * 2. - end, end] } else { [start, end] }
}

#[cfg(test)]
mod test_line_tool {
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
	use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graph_craft::document::value::TaggedValue;

	/// Get the line's document-space start and end points by reading line_to from the node
	/// and computing the actual positions via the layer's transform.
	async fn get_line_endpoints_document(editor: &mut EditorTestUtils) -> Option<(DVec2, DVec2)> {
		let document = editor.active_document();
		let network_interface = &document.network_interface;

		network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(network_interface)
			.filter_map(|layer| {
				let node_inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::line::IDENTIFIER))?;
				let Some(&TaggedValue::DVec2(line_to)) = node_inputs[1].as_value() else {
					return None;
				};

				let transform_to_doc = document.metadata().transform_to_document(layer);
				let doc_start = transform_to_doc.transform_point2(DVec2::ZERO);
				let doc_end = transform_to_doc.transform_point2(line_to);
				Some((doc_start, doc_end))
			})
			.next()
	}

	#[tokio::test]
	async fn test_line_tool_basicdraw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 0., 0., 100., 100., ModifierKeys::empty()).await;
		if let Some((start, end)) = get_line_endpoints_document(&mut editor).await {
			assert!((start - DVec2::ZERO).length() < 1., "Start point should be near (0,0)");
			assert!((end - DVec2::new(100., 100.)).length() < 1., "End point should be near (100,100)");
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
		if let Some((start, end)) = get_line_endpoints_document(&mut editor).await {
			let document = editor.active_document();
			let document_to_viewport = document.metadata().document_to_viewport;
			let viewport_to_document = document_to_viewport.inverse();

			let expected_start = viewport_to_document.transform_point2(DVec2::ZERO);
			let expected_end = viewport_to_document.transform_point2(DVec2::new(100., 100.));

			assert!(
				(start - expected_start).length() < 1.,
				"Start point should match expected document coordinates. Got {start:?}, expected {expected_start:?}"
			);
			assert!(
				(end - expected_end).length() < 1.,
				"End point should match expected document coordinates. Got {end:?}, expected {expected_end:?}"
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
		if let Some((start, end)) = get_line_endpoints_document(&mut editor).await {
			let line_vec = end - start;
			let original_angle = line_vec.angle_to(DVec2::X);
			editor.drag_tool(ToolType::Line, 0., 0., 200., 50., ModifierKeys::CONTROL).await;
			if let Some((updated_start, updated_end)) = get_line_endpoints_document(&mut editor).await {
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
		if let Some((start, end)) = get_line_endpoints_document(&mut editor).await {
			let expected_start = DVec2::new(0., 100.);
			let expected_end = DVec2::new(200., 100.);
			assert!((start - expected_start).length() < 1., "Start point should be near (0, 100)");
			assert!((end - expected_end).length() < 1., "End point should be near (200, 100)");
		}
	}

	#[tokio::test]
	async fn test_line_tool_alt_shift_drag() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 100., 100., 150., 120., ModifierKeys::ALT | ModifierKeys::SHIFT).await;
		if let Some((start, end)) = get_line_endpoints_document(&mut editor).await {
			let line_vec = end - start;
			let angle_radians = line_vec.angle_to(DVec2::X);
			let angle_degrees = angle_radians.to_degrees();
			let nearest_angle = (angle_degrees / 15.).round() * 15.;

			assert!((angle_degrees - nearest_angle).abs() < 1., "Angle should snap to the nearest 15 degrees");
		}
	}

	#[tokio::test]
	async fn a_line_starts_and_ends_on_the_corners_it_is_brought_near() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 200., 160.).await;
		editor.draw_rect(400., 300., 500., 380.).await;

		// Both the press and the release are a few pixels off a corner
		editor.drag_tool(ToolType::Line, 203., 158., 397., 303., ModifierKeys::empty()).await;

		let (start, end) = get_line_endpoints_document(&mut editor).await.expect("a line should have been drawn");
		assert!(start.abs_diff_eq(DVec2::new(200., 160.), 1e-6), "the line starts at {start}");
		assert!(end.abs_diff_eq(DVec2::new(400., 300.), 1e-6), "the line ends at {end}");
	}

	#[tokio::test]
	async fn a_line_drawn_to_the_side_of_an_ellipse_touches_it_without_crossing() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_ellipse(300., 200., 500., 320.).await;
		let (center, radii) = (DVec2::new(400., 260.), DVec2::new(100., 60.));

		// Where a line from the start would touch a true ellipse, which the drawn one comes within a small fraction of a pixel of
		let start = DVec2::new(60., 40.);
		let toward = (center - start) / radii;
		let angle = toward.to_angle() + (-1. / toward.length()).acos();
		let touch = center + DVec2::from_angle(angle) * radii;

		editor.drag_tool(ToolType::Line, start.x, start.y, touch.x + 3., touch.y - 2., ModifierKeys::empty()).await;

		let (_, end) = get_line_endpoints_document(&mut editor).await.expect("a line should have been drawn");
		assert!(end.distance(touch) < 0.5, "the line ends at {end} and not at {touch}");

		// The line runs square to the way the ellipse faces there, to within a tenth of a degree
		let facing = ((end - center) / (radii * radii)).normalize();
		assert!((end - start).normalize().dot(facing).abs() < 2e-3);
	}

	#[tokio::test]
	async fn a_line_held_to_an_angle_still_stops_where_it_meets_an_edge() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(200., 100., 300., 300.).await;

		// The pointer strays from level, and the line stays level and ends on the rectangle's side
		editor.drag_tool(ToolType::Line, 50., 150., 203., 160., ModifierKeys::SHIFT).await;

		let (start, end) = get_line_endpoints_document(&mut editor).await.expect("a line should have been drawn");
		assert!(start.abs_diff_eq(DVec2::new(50., 150.), 1e-6), "the line starts at {start}");
		assert!(end.abs_diff_eq(DVec2::new(200., 150.), 1e-6), "the line ends at {end}");
	}

	#[tokio::test]
	async fn a_line_drawn_from_its_center_can_snap_at_the_end_away_from_the_pointer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 100., 40., 300.).await;

		// The pointer is out in the open, and the mirrored end comes within reach of the rectangle's side
		editor.drag_tool(ToolType::Line, 200., 200., 357., 230., ModifierKeys::ALT).await;

		let (start, end) = get_line_endpoints_document(&mut editor).await.expect("a line should have been drawn");
		assert!(start.abs_diff_eq(DVec2::new(40., 170.), 1e-6), "the line starts at {start}");
		assert!(end.abs_diff_eq(DVec2::new(360., 230.), 1e-6), "the line ends at {end}");
	}

	#[tokio::test]
	async fn holding_control_over_an_end_of_the_selected_line_shows_where_a_new_line_would_start() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Line, 100., 100., 300., 200., ModifierKeys::empty()).await;
		let (_, end) = get_line_endpoints_document(&mut editor).await.expect("a line should have been drawn");

		// Drawing the overlays finds where the selected line's ends are, and the pointer is brought a couple of pixels to the side of one
		editor.handle_message(ShapeToolMessage::Overlays { context: OverlayContext::default() }).await;
		let beside_end = editor.active_document().metadata().document_to_viewport.transform_point2(end) + DVec2::new(-1., 2.);
		let shown = |editor: &EditorTestUtils| editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown_position();

		// A press here would drag that end, so no start is shown
		editor.move_mouse(beside_end.x, beside_end.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		assert_eq!(shown(&editor), None);

		// With Control, a press here starts a new line instead, so its start is shown on that end
		editor.move_mouse(beside_end.x, beside_end.y, ModifierKeys::CONTROL, MouseKeys::empty()).await;
		let start = shown(&editor).expect("the start of a new line should be shown");
		assert!(start.abs_diff_eq(end, 1e-6), "the new line would start at {start} rather than {end}");
	}

	#[tokio::test]
	async fn holding_control_over_the_cage_of_a_selected_rectangle_shows_where_a_line_would_start() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 200., 180.).await;
		editor.select_tool(ToolType::Line).await;
		let shown = |editor: &EditorTestUtils| editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown_position();

		// Drawing the overlays lays the cage around the rectangle, and a press on its right edge would resize it rather than start a line
		editor.handle_message(ShapeToolMessage::Overlays { context: OverlayContext::default() }).await;
		let on_the_right_edge = DVec2::new(200., 130.);
		editor.move_mouse(on_the_right_edge.x, on_the_right_edge.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		assert_eq!(shown(&editor), None);

		// With Control, a press there draws a line instead, so where it would start is shown
		editor.move_mouse(on_the_right_edge.x, on_the_right_edge.y, ModifierKeys::CONTROL, MouseKeys::empty()).await;
		assert!(shown(&editor).is_some(), "where a new line would start should be shown");
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

		let (start, end) = get_line_endpoints_document(&mut editor).await.expect("Line was not created successfully within transformed artboard");
		// The line should still be diagonal with equal change in x and y
		let line_vector = end - start;
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
