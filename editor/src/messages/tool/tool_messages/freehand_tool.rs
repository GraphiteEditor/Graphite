use super::tool_prelude::*;
use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::utility_functions::should_extend;
use glam::DVec2;
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::vector::VectorModificationType;
use graphene_std::vector::{PointId, SegmentId};

#[derive(Default, ExtractField)]
pub struct FreehandTool {
	fsm_state: FreehandToolFsmState,
	data: FreehandToolData,
	options: FreehandOptions,
}

pub struct FreehandOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for FreehandOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Freehand)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FreehandToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart { append_to_selected: Key },
	DragStop,
	PointerMove,
	UpdateOptions(FreehandOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FreehandOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
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
	fn tooltip(&self) -> String {
		"Freehand Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Freehand
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(1.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for FreehandTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorInput| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for FreehandTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Freehand(FreehandToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, true);
			return;
		};
		match action {
			FreehandOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			FreehandOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			FreehandOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			FreehandOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			FreehandOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			FreehandOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			FreehandToolFsmState::Ready => actions!(FreehandToolMessageDiscriminant;
				DragStart,
				DragStop,
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
			overlay_provider: Some(|overlay_context: OverlayContext| FreehandToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(FreehandToolMessage::Abort.into()),
			working_color_changed: Some(FreehandToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct FreehandToolData {
	end_point: Option<(DVec2, PointId)>,
	dragged: bool,
	weight: f64,
	layer: Option<LayerNodeIdentifier>,
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
			global_tool_data,
			input,
			shape_editor,
			preferences,
			..
		} = tool_action_data;

		let ToolMessage::Freehand(event) = event else { return self };
		match (self, event) {
			(_, FreehandToolMessage::Overlays(mut overlay_context)) => {
				path_endpoint_overlays(document, shape_editor, &mut overlay_context, tool_action_data.preferences);

				self
			}
			(FreehandToolFsmState::Ready, FreehandToolMessage::DragStart { append_to_selected }) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.dragged = false;
				tool_data.end_point = None;
				tool_data.weight = tool_options.line_weight;

				// Extend an endpoint of the selected path
				let selected_nodes = document.network_interface.selected_nodes();
				let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
				if let Some((layer, point, position)) = should_extend(document, input.mouse.position, tolerance, selected_nodes.selected_layers(document.metadata()), preferences) {
					tool_data.layer = Some(layer);
					tool_data.end_point = Some((position, point));

					extend_path_with_next_segment(tool_data, position, true, responses);

					return FreehandToolFsmState::Drawing;
				}

				if input.keyboard.key(append_to_selected) {
					let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
					let existing_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());
					if let Some(layer) = existing_layer {
						tool_data.layer = Some(layer);

						let transform = document.metadata().transform_to_viewport(layer);
						let position = transform.inverse().transform_point2(input.mouse.position);

						extend_path_with_next_segment(tool_data, position, false, responses);

						return FreehandToolFsmState::Drawing;
					}
				}

				responses.add(DocumentMessage::DeselectAllLayers);

				let parent = document.new_layer_bounding_artboard(input);

				let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
				let node = node_type.default_node_template();
				let nodes = vec![(NodeId(0), node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
				let defered_responses = &mut VecDeque::new();
				tool_options.fill.apply_fill(layer, defered_responses);
				tool_options.stroke.apply_stroke(tool_data.weight, layer, defered_responses);
				responses.add(DeferMessage::AfterGraphRun {
					messages: defered_responses.drain(..).collect(),
				});
				tool_data.layer = Some(layer);

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::PointerMove) => {
				if let Some(layer) = tool_data.layer {
					let transform = document.metadata().transform_to_viewport(layer);
					let position = transform.inverse().transform_point2(input.mouse.position);

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

				FreehandToolFsmState::Ready
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.layer = None;
				tool_data.end_point = None;

				FreehandToolFsmState::Ready
			}
			(_, FreehandToolMessage::WorkingColorChanged) => {
				responses.add(FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
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

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
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

	if extend {
		if let Some((_, previous_position)) = tool_data.end_point {
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
	}

	tool_data.dragged = true;
	tool_data.end_point = Some((position, id));
}

#[cfg(test)]
mod test_freehand {
	use crate::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, MouseKeys, ScrollDelta};
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_stroke_width};
	use crate::messages::tool::tool_messages::freehand_tool::FreehandOptionsUpdate;
	use crate::test_utils::test_prelude::*;
	use glam::{DAffine2, DVec2};
	use graphene_std::vector::Vector;

	async fn get_vector_and_transform_list(editor: &mut EditorTestUtils) -> Vec<(Vector, DAffine2)> {
		let document = editor.active_document();
		let layers = document.metadata().all_layers();

		layers
			.filter_map(|layer| {
				let graph_layer = NodeGraphLayer::new(layer, &document.network_interface);
				// Only get layers with path nodes
				let _ = graph_layer.upstream_visible_node_id_from_name_in_layer("Path")?;

				let vector = document.network_interface.compute_modified_vector(layer)?;
				let transform = document.metadata().transform_to_viewport(layer);
				Some((vector, transform))
			})
			.collect()
	}

	fn verify_path_points(vector_and_transform_list: &[(Vector, DAffine2)], expected_captured_points: &[DVec2], tolerance: f64) -> Result<(), String> {
		assert_eq!(vector_and_transform_list.len(), 1, "There should be one row of Vector geometry");

		let (vector, transform) = vector_and_transform_list.iter().find(|(data, _)| data.point_domain.ids().len() > 0).ok_or("Could not find path data")?;

		let point_count = vector.point_domain.ids().len();
		let segment_count = vector.segment_domain.ids().len();

		let actual_positions: Vec<DVec2> = vector.point_domain.positions().iter().map(|&position| transform.transform_point2(position)).collect();

		if segment_count != point_count - 1 {
			return Err(format!("Expected segments to be one less than points, got {} segments for {} points", segment_count, point_count));
		}

		if point_count != expected_captured_points.len() {
			return Err(format!("Expected {} points, got {}", expected_captured_points.len(), point_count));
		}

		for (i, (&expected, &actual)) in expected_captured_points.iter().zip(actual_positions.iter()).enumerate() {
			let distance = (expected - actual).length();
			if distance >= tolerance {
				return Err(format!("Point {} position mismatch: expected {:?}, got {:?} (distance: {})", i, expected, actual, distance));
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
	async fn test_extend_existing_path() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let initial_points = [DVec2::new(100., 100.), DVec2::new(200., 200.), DVec2::new(300., 100.)];

		editor.select_tool(ToolType::Freehand).await;

		let first_point = initial_points[0];
		editor.move_mouse(first_point.x, first_point.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(first_point.x, first_point.y, ModifierKeys::empty()).await;

		for &point in &initial_points[1..] {
			editor.move_mouse(point.x, point.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		}

		let last_initial_point = initial_points[initial_points.len() - 1];
		editor
			.mouseup(
				EditorMouseState {
					editor_position: last_initial_point,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let initial_vector_and_transform_list = get_vector_and_transform_list(&mut editor).await;
		assert!(!initial_vector_and_transform_list.is_empty(), "No Vector geometry found after initial drawing");

		let (initial_vector, initial_transform) = &initial_vector_and_transform_list[0];
		let initial_point_count = initial_vector.point_domain.ids().len();
		let initial_segment_count = initial_vector.segment_domain.ids().len();

		assert!(initial_point_count >= 2, "Expected at least 2 points in initial path, found {}", initial_point_count);
		assert_eq!(
			initial_segment_count,
			initial_point_count - 1,
			"Expected {} segments in initial path, found {}",
			initial_point_count - 1,
			initial_segment_count
		);

		let extendable_points = initial_vector.extendable_points(false).collect::<Vec<_>>();
		assert!(!extendable_points.is_empty(), "No extendable points found in the path");

		let endpoint_id = extendable_points[0];
		let endpoint_pos_option = initial_vector.point_domain.position_from_id(endpoint_id);
		assert!(endpoint_pos_option.is_some(), "Could not find position for endpoint");

		let endpoint_pos = endpoint_pos_option.unwrap();
		let endpoint_viewport_pos = initial_transform.transform_point2(endpoint_pos);

		assert!(endpoint_viewport_pos.is_finite(), "Endpoint position is not finite");

		let extension_points = [DVec2::new(400., 200.), DVec2::new(500., 100.)];

		let layer_node_id = {
			let document = editor.active_document();
			let layer = document.metadata().all_layers().next().unwrap();
			layer.to_node()
		};

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer_node_id] }).await;

		editor.select_tool(ToolType::Freehand).await;

		editor.move_mouse(endpoint_viewport_pos.x, endpoint_viewport_pos.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(endpoint_viewport_pos.x, endpoint_viewport_pos.y, ModifierKeys::empty()).await;

		for &point in &extension_points {
			editor.move_mouse(point.x, point.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		}

		let last_extension_point = extension_points[extension_points.len() - 1];
		editor
			.mouseup(
				EditorMouseState {
					editor_position: last_extension_point,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let extended_vector_and_transform = get_vector_and_transform_list(&mut editor).await;
		assert!(!extended_vector_and_transform.is_empty(), "No Vector geometry found after extension");

		let (extended_vector, _) = &extended_vector_and_transform[0];
		let extended_point_count = extended_vector.point_domain.ids().len();
		let extended_segment_count = extended_vector.segment_domain.ids().len();

		assert!(
			extended_point_count > initial_point_count,
			"Expected more points after extension, initial: {}, after extension: {}",
			initial_point_count,
			extended_point_count
		);

		assert_eq!(
			extended_segment_count,
			extended_point_count - 1,
			"Expected segments to be one less than points, points: {}, segments: {}",
			extended_point_count,
			extended_segment_count
		);

		let layer_count = {
			let document = editor.active_document();
			document.metadata().all_layers().count()
		};
		assert_eq!(layer_count, 1, "Expected only one layer after extending path");
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
				EditorMouseState {
					editor_position: last_initial_point,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
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
			let layer = document.metadata().all_layers().next().unwrap();
			layer
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
				EditorMouseState {
					editor_position: first_second_point,
					mouse_keys: MouseKeys::LEFT,
					scroll_delta: ScrollDelta::default(),
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
				EditorMouseState {
					editor_position: last_second_point,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
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
			"Expected more points after appending to layer, initial: {}, after append: {}",
			initial_point_count,
			final_point_count
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
			.handle_message(ToolMessage::Freehand(FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::LineWeight(custom_line_weight))))
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
				EditorMouseState {
					editor_position: last_point,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
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
}
