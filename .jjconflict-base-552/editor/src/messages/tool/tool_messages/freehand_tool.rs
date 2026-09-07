use super::tool_prelude::*;
use crate::consts::SNAP_POINT_TOLERANCE;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_network_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::open_path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{
	DrawingToolState, apply_fill_color_pick, apply_fill_enabled, apply_stroke_color_pick, apply_stroke_enabled, apply_working_colors, reset_colors_on_deactivation, swap_fill_and_stroke,
	sync_drawing_state,
};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::translation_transform_set;
use crate::messages::tool::common_functionality::stroke_options::{StrokeOptionsUpdate, apply_stroke_option, create_stroke_options_popover_widget};
use crate::messages::tool::common_functionality::utility_functions::closest_open_path_endpoint;
use glam::DVec2;
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::vector::VectorModificationType;
use graphene_std::vector::style::FillChoice;
use graphene_std::vector::{PointId, SegmentId};

#[derive(Default, ExtractField)]
pub struct FreehandTool {
	fsm_state: FreehandToolFsmState,
	data: FreehandToolData,
	options: FreehandOptions,
}

pub struct FreehandOptions {
	drawing: DrawingToolState,
}

impl Default for FreehandOptions {
	fn default() -> Self {
		Self {
			drawing: DrawingToolState::new(false),
		}
	}
}

#[impl_message(Message, ToolMessage, Freehand)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum FreehandToolMessage {
	// Standard messages
	Overlays { context: OverlayContext },
	Abort,
	SelectionChanged,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart { append_to_selected: Key },
	DragStop,
	PointerMove,
	UpdateOptions { options: FreehandOptionsUpdate },
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum FreehandOptionsUpdate {
	FillColor(FillChoice),
	FillEnabled(bool),
	StrokeOption(StrokeOptionsUpdate),
	StrokeColor(Option<Color>),
	StrokeEnabled(bool),
	SwapFillAndStroke,
	WorkingColorsChanged,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FreehandToolFsmState {
	#[default]
	Ready,
	Drawing,
}

impl ToolMetadata for FreehandTool {
	fn icon_name(&self) -> String {
		"VectorFreehandTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Freehand Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Freehand
	}
}

impl LayoutHolder for FreehandTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.drawing.fill.create_widgets(
			"Fill:",
			|checkbox: &CheckboxInput| {
				FreehandToolMessage::UpdateOptions {
					options: FreehandOptionsUpdate::FillEnabled(checkbox.checked),
				}
				.into()
			},
			|color: &ColorInput| {
				FreehandToolMessage::UpdateOptions {
					options: FreehandOptionsUpdate::FillColor(FillChoice::from(&color.value)),
				}
				.into()
			},
		);

		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.push(
			IconButton::new("SwapHorizontal", 16)
				.tooltip_label("Swap Fill/Stroke Colors")
				.on_update(|_| {
					FreehandToolMessage::UpdateOptions {
						options: FreehandOptionsUpdate::SwapFillAndStroke,
					}
					.into()
				})
				.widget_instance(),
		);
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

		widgets.append(&mut self.options.drawing.stroke.create_widgets(
			"Stroke:",
			|checkbox: &CheckboxInput| {
				FreehandToolMessage::UpdateOptions {
					options: FreehandOptionsUpdate::StrokeEnabled(checkbox.checked),
				}
				.into()
			},
			|color: &ColorInput| {
				FreehandToolMessage::UpdateOptions {
					options: FreehandOptionsUpdate::StrokeColor(color.value.as_solid().map(Color::from)),
				}
				.into()
			},
		));
		let weight_disabled = self.options.drawing.stroke.enabled == Some(false);
		widgets.push(create_stroke_options_popover_widget(&self.options.drawing, weight_disabled, |update| {
			FreehandToolMessage::UpdateOptions {
				options: FreehandOptionsUpdate::StrokeOption(update),
			}
			.into()
		}));

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for FreehandTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		// On tool deactivation (Abort fires from the dispatcher's tool transition), reset the displayed fill/stroke colors so
		// the next activation starts fresh from the current working colors. The global swap state persists across tool switches.
		// Guarded on `Ready` so Esc-mid-drawing (which also fires Abort) doesn't wipe the user's customized fill/stroke options.
		if matches!(&message, ToolMessage::Freehand(FreehandToolMessage::Abort)) && self.fsm_state == FreehandToolFsmState::Ready {
			reset_colors_on_deactivation(&mut self.options.drawing, context.global_tool_data);
		}

		if matches!(&message, ToolMessage::Freehand(FreehandToolMessage::SelectionChanged)) {
			if self.fsm_state != FreehandToolFsmState::Ready {
				return;
			}
			if sync_drawing_state(&mut self.options.drawing, false, true, context.global_tool_data, context.document) {
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			return;
		}

		let ToolMessage::Freehand(FreehandToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, true);
			return;
		};
		match options {
			FreehandOptionsUpdate::FillColor(fill_choice) => {
				apply_fill_color_pick(&mut self.options.drawing, fill_choice, context.document, responses);
			}
			FreehandOptionsUpdate::FillEnabled(enabled) => {
				apply_fill_enabled(&mut self.options.drawing, enabled, context.global_tool_data, context.document, responses);
			}
			FreehandOptionsUpdate::StrokeOption(update) => {
				apply_stroke_option(&mut self.options.drawing, update, context.document, responses);
			}
			FreehandOptionsUpdate::StrokeColor(color) => {
				apply_stroke_color_pick(&mut self.options.drawing, color, context.document, responses);
			}
			FreehandOptionsUpdate::StrokeEnabled(enabled) => {
				apply_stroke_enabled(&mut self.options.drawing, enabled, context.global_tool_data, context.document, responses);
			}
			FreehandOptionsUpdate::SwapFillAndStroke => {
				swap_fill_and_stroke(&mut self.options.drawing, context.document, responses);
			}
			FreehandOptionsUpdate::WorkingColorsChanged => {
				apply_working_colors(&mut self.options.drawing, context.global_tool_data, context.document);
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			FreehandToolFsmState::Ready => actions!(FreehandToolMessageDiscriminant;
				DragStart,
				DragStop,
				PointerMove,
			),
			FreehandToolFsmState::Drawing => actions!(FreehandToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
			),
		}
	}
}

impl ToolTransition for FreehandTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|context: OverlayContext| FreehandToolMessage::Overlays { context }.into()),
			tool_abort: Some(FreehandToolMessage::Abort.into()),
			selection_changed: Some(FreehandToolMessage::SelectionChanged.into()),
			graph_changed: Some(FreehandToolMessage::SelectionChanged.into()),
			working_color_changed: Some(FreehandToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct FreehandToolData {
	end_point: Option<(DVec2, PointId)>,
	dragged: bool,
	layer: Option<LayerNodeIdentifier>,
	/// Viewport-space start position for newly created layers, used to compute local-space
	/// positions before the deferred TransformSet has been reflected in metadata.
	new_layer_viewport_start: Option<DVec2>,
}

impl Fsm for FreehandToolFsmState {
	type ToolData = FreehandToolData;
	type ToolOptions = FreehandOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document,
			input,
			viewport,
			shape_editor,
			..
		} = tool_action_data;

		let ToolMessage::Freehand(event) = event else { return self };
		match (self, event) {
			(_, FreehandToolMessage::Overlays { context: mut overlay_context }) => {
				let pointer = (self == FreehandToolFsmState::Ready).then_some(input.mouse.position);
				open_path_endpoint_overlays(document, shape_editor, pointer, &mut overlay_context);
				self
			}
			(FreehandToolFsmState::Ready, FreehandToolMessage::PointerMove) => {
				responses.add(OverlaysMessage::Draw);
				self
			}
			(FreehandToolFsmState::Ready, FreehandToolMessage::DragStart { append_to_selected }) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.dragged = false;
				tool_data.end_point = None;
				tool_data.new_layer_viewport_start = None;

				// Pressing on the endpoint of a selected open path continues that path in place, keeping the layer, its selection, and its style
				let selected_nodes = document.network_interface.selected_nodes();
				let selected_visible_layers = selected_nodes.selected_visible_layers(&document.network_interface);
				if let Some((layer, endpoint, position)) = closest_open_path_endpoint(document, input.mouse.position, SNAP_POINT_TOLERANCE, selected_visible_layers) {
					tool_data.layer = Some(layer);
					tool_data.end_point = Some((position, endpoint));

					return FreehandToolFsmState::Drawing;
				}

				if input.keyboard.key(append_to_selected) {
					let mut appendable_layers = selected_nodes
						.selected_visible_layers(&document.network_interface)
						.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));
					let existing_layer = appendable_layers.next().filter(|_| appendable_layers.next().is_none());
					if let Some(layer) = existing_layer {
						tool_data.layer = Some(layer);

						let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
						let position = transform.inverse().transform_point2(input.mouse.position);

						extend_path_with_next_segment(tool_data, position, false, responses);

						return FreehandToolFsmState::Drawing;
					}
				}

				responses.add(DocumentMessage::DeselectAllLayers);

				let parent = document.new_layer_bounding_artboard(input, viewport);

				let node_type = resolve_network_node_type("Path").expect("Path node does not exist");
				let node = node_type.default_node_template();
				let nodes = vec![(NodeId(0), node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
				tool_options.drawing.fill.apply_fill(layer, responses);
				tool_options.drawing.apply_stroke_to_new_layer(layer, responses);
				tool_options.drawing.apply_stroke_order_to_new_layer(layer, responses);
				tool_data.layer = Some(layer);
				tool_data.new_layer_viewport_start = Some(input.mouse.position);

				// Position the layer at the initial mouse position via Transform
				responses.add(DeferMessage::AfterGraphRun {
					messages: vec![translation_transform_set(document, layer, input.mouse.position), NodeGraphMessage::RunDocumentGraph.into()],
				});

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::PointerMove) => {
				if let Some(layer) = tool_data.layer {
					let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);

					// For newly created layers, the deferred TransformSet may not yet be reflected
					// in the metadata, so compute local position from the known viewport start.
					// Once the metadata catches up (origin maps to start), switch to using it so
					// that mid-stroke pan/tilt/zoom works correctly.
					if let Some(start) = tool_data.new_layer_viewport_start
						&& transform.transform_point2(DVec2::ZERO).abs_diff_eq(start, 1e-5)
					{
						tool_data.new_layer_viewport_start = None;
					}
					let position = if let Some(start) = tool_data.new_layer_viewport_start {
						document.metadata().document_to_viewport.inverse().transform_vector2(input.mouse.position - start)
					} else {
						transform.inverse().transform_point2(input.mouse.position)
					};

					extend_path_with_next_segment(tool_data, position, true, responses);
				}

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::DragStop) => {
				if tool_data.dragged {
					responses.add(DocumentMessage::CommitTransaction);
				} else {
					responses.add(DocumentMessage::EndTransaction);
				}

				tool_data.end_point = None;
				tool_data.layer = None;
				tool_data.new_layer_viewport_start = None;

				FreehandToolFsmState::Ready
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.layer = None;
				tool_data.end_point = None;
				tool_data.new_layer_viewport_start = None;

				FreehandToolFsmState::Ready
			}
			(_, FreehandToolMessage::WorkingColorChanged) => {
				responses.add(FreehandToolMessage::UpdateOptions {
					options: FreehandOptionsUpdate::WorkingColorsChanged,
				});
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FreehandToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polyline"),
				// TODO: Only show this if a single layer is selected and it's of a valid type (e.g. a vector path but not raster or artboard)
				HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
			])]),
			FreehandToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn extend_path_with_next_segment(tool_data: &mut FreehandToolData, position: DVec2, extend: bool, responses: &mut VecDeque<Message>) {
	if !tool_data.end_point.is_none_or(|(last_pos, _)| position != last_pos) || !position.is_finite() {
		return;
	}

	let Some(layer) = tool_data.layer else { return };

	let id = PointId::generate();
	responses.add(GraphOperationMessage::Vector {
		layer,
		modification_type: VectorModificationType::InsertPoint { id, position },
	});

	if extend && let Some((_, previous_position)) = tool_data.end_point {
		let next_id = SegmentId::generate();
		let points = [previous_position, id];

		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: VectorModificationType::InsertSegment {
				id: next_id,
				points,
				handles: [None, None],
			},
		});
	}

	tool_data.dragged = true;
	tool_data.end_point = Some((position, id));
}

#[cfg(test)]
mod test_freehand {
	use crate::messages::input_mapper::utility_types::pointer::{EditorPointerState, MouseKeys};
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_stroke_width};
	use crate::messages::tool::common_functionality::stroke_options::StrokeOptionsUpdate;
	use crate::messages::tool::tool_messages::freehand_tool::FreehandOptionsUpdate;
	use crate::test_utils::test_prelude::*;
	use glam::{DAffine2, DMat2, DVec2};
	use graphene_std::vector::Vector;

	async fn get_vector_and_transform_list(editor: &mut EditorTestUtils) -> Vec<(Vector, DAffine2)> {
		let document = editor.active_document();
		let layers = document.metadata().all_layers();

		layers
			.filter_map(|layer| {
				let graph_layer = NodeGraphLayer::new(layer, &document.network_interface);
				// Only get layers with path nodes
				let _ = graph_layer.upstream_visible_node_id_from_name_in_layer(&DefinitionIdentifier::Network("Path".into()))?;

				let vector = document.network_interface.compute_modified_vector(layer)?;
				let transform = document.metadata().transform_to_viewport(layer);
				Some((vector, transform))
			})
			.collect()
	}

	fn verify_path_points(vector_and_transform_list: &[(Vector, DAffine2)], expected_captured_points: &[DVec2], tolerance: f64) -> Result<(), String> {
		assert_eq!(vector_and_transform_list.len(), 1, "There should be one item of Vector geometry");

		let (vector, transform) = vector_and_transform_list
			.iter()
			.find(|(data, _)| !data.point_domain.ids().is_empty())
			.ok_or("Could not find path data")?;

		let point_count = vector.point_domain.ids().len();
		let segment_count = vector.segment_domain.ids().len();

		let actual_positions: Vec<DVec2> = vector.point_domain.positions().iter().map(|&position| transform.transform_point2(position)).collect();

		if segment_count != point_count - 1 {
			return Err(format!("Expected segments to be one less than points, got {segment_count} segments for {point_count} points"));
		}

		if point_count != expected_captured_points.len() {
			return Err(format!("Expected {} points, got {}", expected_captured_points.len(), point_count));
		}

		for (i, (&expected, &actual)) in expected_captured_points.iter().zip(actual_positions.iter()).enumerate() {
			let distance = (expected - actual).length();
			if distance >= tolerance {
				return Err(format!("Point {i} position mismatch: expected {expected:?}, got {actual:?} (distance: {distance})"));
			}
		}

		Ok(())
	}

	#[tokio::test]
	async fn test_freehand_transformed_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Artboard, 0., 0., 500., 500., ModifierKeys::empty()).await;

		let metadata = editor.active_document().metadata();
		let artboard = metadata.all_layers().next().unwrap();

		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer: artboard,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1.5, 0.8), 0.3, DVec2::new(10., -5.)),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		editor.select_tool(ToolType::Freehand).await;

		let mouse_points = [DVec2::new(150., 100.), DVec2::new(200., 150.), DVec2::new(250., 130.), DVec2::new(300., 170.)];

		// Expected points that will actually be captured by the tool
		let expected_captured_points = &mouse_points[1..];
		editor.drag_path(&mouse_points, ModifierKeys::empty()).await;

		let vector_and_transform_list = get_vector_and_transform_list(&mut editor).await;
		verify_path_points(&vector_and_transform_list, expected_captured_points, 1.).expect("Path points verification failed");
	}

	#[tokio::test]
	async fn test_append_to_selected_layer_with_shift() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.select_tool(ToolType::Freehand).await;

		let initial_points = [DVec2::new(100., 100.), DVec2::new(200., 200.), DVec2::new(300., 100.)];

		let first_point = initial_points[0];
		editor.move_mouse(first_point.x, first_point.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(first_point.x, first_point.y, ModifierKeys::empty()).await;

		for &point in &initial_points[1..] {
			editor.move_mouse(point.x, point.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		}

		let last_initial_point = initial_points[initial_points.len() - 1];
		editor
			.mouseup(
				EditorPointerState {
					editor_position: last_initial_point,
					mouse_keys: MouseKeys::empty(),
					..Default::default()
				},
				ModifierKeys::empty(),
			)
			.await;

		let initial_vector_and_transform = get_vector_and_transform_list(&mut editor).await;
		assert!(!initial_vector_and_transform.is_empty(), "No vector geometry found after initial drawing");

		let (initial_vector, _) = &initial_vector_and_transform[0];
		let initial_point_count = initial_vector.point_domain.ids().len();
		let initial_segment_count = initial_vector.segment_domain.ids().len();

		let existing_layer_id = {
			let document = editor.active_document();

			document.metadata().all_layers().next().unwrap()
		};

		editor
			.handle_message(NodeGraphMessage::SelectedNodesSet {
				nodes: vec![existing_layer_id.to_node()],
			})
			.await;

		let second_path_points = [DVec2::new(400., 100.), DVec2::new(500., 200.), DVec2::new(600., 100.)];

		let first_second_point = second_path_points[0];
		editor.move_mouse(first_second_point.x, first_second_point.y, ModifierKeys::SHIFT, MouseKeys::empty()).await;

		editor
			.mousedown(
				EditorPointerState {
					editor_position: first_second_point,
					mouse_keys: MouseKeys::LEFT,
					..Default::default()
				},
				ModifierKeys::SHIFT,
			)
			.await;

		for &point in &second_path_points[1..] {
			editor.move_mouse(point.x, point.y, ModifierKeys::SHIFT, MouseKeys::LEFT).await;
		}

		let last_second_point = second_path_points[second_path_points.len() - 1];
		editor
			.mouseup(
				EditorPointerState {
					editor_position: last_second_point,
					mouse_keys: MouseKeys::empty(),
					..Default::default()
				},
				ModifierKeys::SHIFT,
			)
			.await;

		let final_vector_and_transform = get_vector_and_transform_list(&mut editor).await;
		assert!(!final_vector_and_transform.is_empty(), "No vector geometry found after second drawing");

		// Verify we still have only one layer
		let layer_count = {
			let document = editor.active_document();
			document.metadata().all_layers().count()
		};
		assert_eq!(layer_count, 1, "Expected only one layer after drawing with Shift key");

		let (final_vector, _) = &final_vector_and_transform[0];
		let final_point_count = final_vector.point_domain.ids().len();
		let final_segment_count = final_vector.segment_domain.ids().len();

		assert!(
			final_point_count > initial_point_count,
			"Expected more points after appending to layer, initial: {initial_point_count}, after append: {final_point_count}"
		);

		let expected_new_points = second_path_points.len();
		let expected_new_segments = expected_new_points - 1;

		assert_eq!(
			final_point_count,
			initial_point_count + expected_new_points,
			"Expected {} total points after append",
			initial_point_count + expected_new_points
		);

		assert_eq!(
			final_segment_count,
			initial_segment_count + expected_new_segments,
			"Expected {} total segments after append",
			initial_segment_count + expected_new_segments
		);
	}

	#[tokio::test]
	async fn test_line_weight_affects_stroke_width() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.select_tool(ToolType::Freehand).await;

		let custom_line_weight = 5.;
		editor
			.handle_message(ToolMessage::Freehand(FreehandToolMessage::UpdateOptions {
				options: FreehandOptionsUpdate::StrokeOption(StrokeOptionsUpdate::LineWeight(custom_line_weight)),
			}))
			.await;

		let points = [DVec2::new(100., 100.), DVec2::new(200., 200.), DVec2::new(300., 100.)];

		let first_point = points[0];
		editor.move_mouse(first_point.x, first_point.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(first_point.x, first_point.y, ModifierKeys::empty()).await;

		for &point in &points[1..] {
			editor.move_mouse(point.x, point.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		}

		let last_point = points[points.len() - 1];
		editor
			.mouseup(
				EditorPointerState {
					editor_position: last_point,
					mouse_keys: MouseKeys::empty(),
					..Default::default()
				},
				ModifierKeys::empty(),
			)
			.await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let stroke_width = get_stroke_width(layer, &document.network_interface);

		assert!(stroke_width.is_some(), "Stroke width should be available on the created path");

		assert_eq!(
			stroke_width.unwrap(),
			custom_line_weight,
			"Stroke width should match the custom line weight (expected {}, got {})",
			custom_line_weight,
			stroke_width.unwrap()
		);
	}

	/// Draws a Freehand stroke through the given viewport positions and returns the layer it created, which the tool leaves selected.
	async fn draw_freehand_stroke(editor: &mut EditorTestUtils, points: &[DVec2]) -> LayerNodeIdentifier {
		editor.select_tool(ToolType::Freehand).await;
		editor.drag_path(points, ModifierKeys::empty()).await;
		editor.get_selected_layer().await.expect("The Freehand stroke should create and select a layer")
	}

	fn layer_count(editor: &EditorTestUtils) -> usize {
		editor.active_document().metadata().all_layers().count()
	}

	fn point_and_segment_counts(editor: &EditorTestUtils, layer: LayerNodeIdentifier) -> (usize, usize) {
		let vector = editor.active_document().network_interface.compute_modified_vector(layer).expect("Layer should have vector geometry");
		(vector.point_domain.ids().len(), vector.segment_domain.ids().len())
	}

	/// The viewport positions of the layer's anchors, split into the endpoints of its open paths and every other anchor.
	fn endpoint_and_other_anchor_viewport_positions(editor: &EditorTestUtils, layer: LayerNodeIdentifier) -> (Vec<DVec2>, Vec<DVec2>) {
		let document = editor.active_document();
		let vector = document.network_interface.compute_modified_vector(layer).expect("Layer should have vector geometry");
		let transform = document.metadata().transform_to_viewport(layer);

		let endpoints: Vec<_> = vector.anchor_endpoints().collect();
		let viewport_position = |id| vector.point_domain.position_from_id(id).map(|position| transform.transform_point2(position));

		let endpoint_positions = endpoints.iter().filter_map(|&id| viewport_position(id)).collect();
		let other_positions = vector.anchor_points().filter(|id| !endpoints.contains(id)).filter_map(viewport_position).collect();
		(endpoint_positions, other_positions)
	}

	const INITIAL_STROKE: [DVec2; 3] = [DVec2::new(100., 100.), DVec2::new(200., 150.), DVec2::new(300., 100.)];

	#[tokio::test]
	async fn test_extend_open_path_from_endpoint() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let (initial_point_count, initial_segment_count) = point_and_segment_counts(&editor, layer);
		assert_eq!(initial_segment_count, initial_point_count - 1, "The initial stroke should be a single open path");

		// Press just inside the snapping tolerance of an endpoint, then keep drawing
		let (endpoints, _) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let endpoint = *endpoints.last().expect("The stroke should have endpoints");
		let continuation = [endpoint + DVec2::new(2., -2.), DVec2::new(400., 150.), DVec2::new(500., 100.)];
		editor.drag_path(&continuation, ModifierKeys::empty()).await;

		assert_eq!(layer_count(&editor), 1, "Continuing from an endpoint should not create a new layer");
		assert_eq!(editor.get_selected_layer().await, Some(layer), "The selection should be left as it was");

		let (point_count, segment_count) = point_and_segment_counts(&editor, layer);
		assert_eq!(point_count, initial_point_count + continuation.len() - 1, "Each pointer move after the press should add a point");
		assert_eq!(segment_count, point_count - 1, "The continued stroke should form a single open path");
	}

	#[tokio::test]
	async fn test_stroke_away_from_endpoint_creates_new_layer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let initial_counts = point_and_segment_counts(&editor, layer);

		editor.drag_path(&[DVec2::new(100., 300.), DVec2::new(200., 350.), DVec2::new(300., 300.)], ModifierKeys::empty()).await;

		assert_eq!(layer_count(&editor), 2, "A stroke starting away from any endpoint should create a new layer");
		assert_eq!(point_and_segment_counts(&editor, layer), initial_counts, "The existing path should be untouched");
	}

	#[tokio::test]
	async fn test_middle_anchor_does_not_extend() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Four pointer positions capture three points, so the path has an anchor that is not an endpoint
		let layer = draw_freehand_stroke(&mut editor, &[DVec2::new(100., 100.), DVec2::new(200., 150.), DVec2::new(300., 100.), DVec2::new(400., 150.)]).await;
		let initial_counts = point_and_segment_counts(&editor, layer);

		let (_, other_anchors) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let middle_anchor = *other_anchors.first().expect("The stroke should have an anchor between its endpoints");
		editor
			.drag_path(&[middle_anchor, middle_anchor + DVec2::new(0., 100.), middle_anchor + DVec2::new(0., 200.)], ModifierKeys::empty())
			.await;

		assert_eq!(layer_count(&editor), 2, "Only the endpoints of a path should be continued from");
		assert_eq!(point_and_segment_counts(&editor, layer), initial_counts, "The existing path should be untouched");
	}

	#[tokio::test]
	async fn test_closed_path_never_extends() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.draw_rect(100., 100., 300., 300.).await;
		let rectangle = editor.get_selected_layer().await.expect("The rectangle should be selected");
		let (endpoints, corners) = endpoint_and_other_anchor_viewport_positions(&editor, rectangle);
		assert!(endpoints.is_empty(), "A closed path has no endpoints");
		let corner = *corners.first().expect("The rectangle should have corner anchors");

		editor.select_tool(ToolType::Freehand).await;
		editor.drag_path(&[corner, corner + DVec2::new(100., 50.), corner + DVec2::new(200., 0.)], ModifierKeys::empty()).await;

		assert_eq!(layer_count(&editor), 2, "Pressing on a closed path's anchor should start a new layer");
	}

	#[tokio::test]
	async fn test_extend_after_layer_transform() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let (initial_point_count, _) = point_and_segment_counts(&editor, layer);

		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1.5, 0.8), 0.3, DVec2::new(40., -25.)),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		let transform = editor.active_document().metadata().transform_to_viewport(layer);
		assert!(!transform.matrix2.abs_diff_eq(DMat2::IDENTITY, 1e-6), "The layer should be rotated and scaled");

		// Press within tolerance of where an endpoint now sits in the viewport, then keep drawing
		let (endpoints, _) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let endpoint = *endpoints.first().expect("The stroke should have endpoints");
		let continuation = [endpoint + DVec2::new(-2., 2.), endpoint + DVec2::new(80., 60.), endpoint + DVec2::new(160., 20.)];
		editor.drag_path(&continuation, ModifierKeys::empty()).await;

		assert_eq!(layer_count(&editor), 1, "Continuing from a transformed layer's endpoint should not create a new layer");

		let (point_count, segment_count) = point_and_segment_counts(&editor, layer);
		assert_eq!(point_count, initial_point_count + continuation.len() - 1, "Each pointer move after the press should add a point");
		assert_eq!(segment_count, point_count - 1, "The continued stroke should form a single open path");

		// The new points are stored in the layer's local space, so they should sit under the pointer once transformed back to the viewport
		let document = editor.active_document();
		let vector = document.network_interface.compute_modified_vector(layer).expect("Layer should have vector geometry");
		let transform = document.metadata().transform_to_viewport(layer);
		for &pointer_position in &continuation[1..] {
			let under_pointer = vector
				.point_domain
				.positions()
				.iter()
				.any(|&position| transform.transform_point2(position).distance(pointer_position) < 1.);
			assert!(under_pointer, "Expected a point under the pointer at {pointer_position:?}");
		}
	}

	#[tokio::test]
	async fn test_endpoint_takes_priority_over_shift_append() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let (initial_point_count, _) = point_and_segment_counts(&editor, layer);

		let (endpoints, _) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let endpoint = *endpoints.last().expect("The stroke should have endpoints");
		let continuation = [endpoint + DVec2::new(2., 2.), DVec2::new(400., 150.), DVec2::new(500., 100.)];
		editor.drag_path(&continuation, ModifierKeys::SHIFT).await;

		assert_eq!(layer_count(&editor), 1, "Shift should keep drawing on the selected layer");

		// Appending a disconnected subpath would leave the layer with one fewer segment than a single continued path has
		let (point_count, segment_count) = point_and_segment_counts(&editor, layer);
		assert_eq!(point_count, initial_point_count + continuation.len() - 1, "Each pointer move after the press should add a point");
		assert_eq!(segment_count, point_count - 1, "Shift on an endpoint should continue the path rather than append a new subpath");
	}

	#[tokio::test]
	async fn test_press_and_release_on_endpoint_leaves_path_unchanged() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let initial_counts = point_and_segment_counts(&editor, layer);

		let (endpoints, _) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let endpoint = *endpoints.last().expect("The stroke should have endpoints");
		editor.move_mouse(endpoint.x, endpoint.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(endpoint.x, endpoint.y, ModifierKeys::empty()).await;
		editor.left_mouseup(endpoint.x, endpoint.y, ModifierKeys::empty()).await;

		assert_eq!(layer_count(&editor), 1, "Releasing without moving should not create a layer");
		assert_eq!(point_and_segment_counts(&editor, layer), initial_counts, "Releasing without moving should leave the path unchanged");
	}

	#[tokio::test]
	async fn test_hidden_layer_endpoint_is_not_extended() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let initial_counts = point_and_segment_counts(&editor, layer);
		let (endpoints, _) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let endpoint = *endpoints.first().expect("The stroke should have endpoints");

		// Where the hidden layer's endpoint would land without its transform
		let metadata = editor.active_document().metadata();
		let untransformed_endpoint = metadata
			.document_to_viewport
			.transform_point2(metadata.transform_to_viewport(layer).inverse().transform_point2(endpoint));

		editor
			.handle_message(NodeGraphMessage::ToggleVisibility {
				node_id: layer.to_node(),
				network_path: Vec::new(),
			})
			.await;
		assert_eq!(editor.get_selected_layer().await, Some(layer), "Hiding the layer should leave it selected");

		editor
			.drag_path(&[endpoint + DVec2::new(2., -2.), DVec2::new(400., 150.), DVec2::new(500., 100.)], ModifierKeys::empty())
			.await;
		assert_eq!(layer_count(&editor), 2, "A stroke at a hidden layer's endpoint should create a new layer");

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;
		editor
			.drag_path(
				&[untransformed_endpoint, untransformed_endpoint + DVec2::new(100., 50.), untransformed_endpoint + DVec2::new(200., 0.)],
				ModifierKeys::empty(),
			)
			.await;
		assert_eq!(
			layer_count(&editor),
			3,
			"A stroke where the hidden layer's untransformed endpoint would sit should also create a new layer"
		);

		editor
			.handle_message(NodeGraphMessage::ToggleVisibility {
				node_id: layer.to_node(),
				network_path: Vec::new(),
			})
			.await;
		assert_eq!(point_and_segment_counts(&editor, layer), initial_counts, "The hidden layer should be untouched");
	}

	#[tokio::test]
	async fn test_locked_layer_endpoint_is_extended() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let layer = draw_freehand_stroke(&mut editor, &INITIAL_STROKE).await;
		let (initial_point_count, _) = point_and_segment_counts(&editor, layer);
		let (endpoints, _) = endpoint_and_other_anchor_viewport_positions(&editor, layer);
		let endpoint = *endpoints.last().expect("The stroke should have endpoints");

		editor
			.handle_message(NodeGraphMessage::ToggleLocked {
				node_id: layer.to_node(),
				network_path: Vec::new(),
			})
			.await;
		assert_eq!(editor.get_selected_layer().await, Some(layer), "Locking the layer should leave it selected");

		let continuation = [endpoint + DVec2::new(2., -2.), DVec2::new(400., 150.), DVec2::new(500., 100.)];
		editor.drag_path(&continuation, ModifierKeys::empty()).await;

		// Locking only blocks viewport picking by the Select tool, so a selected locked layer stays editable
		assert_eq!(layer_count(&editor), 1, "A selected locked layer's endpoint should still be continued from");

		let (point_count, segment_count) = point_and_segment_counts(&editor, layer);
		assert_eq!(point_count, initial_point_count + continuation.len() - 1, "Each pointer move after the press should add a point");
		assert_eq!(segment_count, point_count - 1, "The continued stroke should form a single open path");
	}
}
