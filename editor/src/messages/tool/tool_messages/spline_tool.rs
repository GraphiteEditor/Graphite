use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, DRAG_THRESHOLD, PATH_JOIN_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::input_mapper::utility_types::input_mouse::MouseKeys;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, find_spline, merge_layers, merge_points};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration, SnappedPoint};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, should_extend};
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::vector::{PointId, SegmentId, VectorModificationType};

#[derive(Default, ExtractField)]
pub struct SplineTool {
	fsm_state: SplineToolFsmState,
	tool_data: SplineToolData,
	options: SplineOptions,
}

pub struct SplineOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for SplineOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Spline)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SplineToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	CanvasTransformed,
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	Confirm,
	DragStart { append_to_selected: Key },
	DragStop,
	MergeEndpoints,
	PointerMove,
	PointerOutsideViewport,
	Undo,
	UpdateOptions(SplineOptionsUpdate),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SplineToolFsmState {
	#[default]
	Ready,
	Drawing,
	MergingEndpoints,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SplineOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for SplineTool {
	fn icon_name(&self) -> String {
		"VectorSplineTool".into()
	}
	fn tooltip(&self) -> String {
		"Spline Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Spline
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for SplineTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Spline(SplineToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			return;
		};
		match action {
			SplineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			SplineOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			SplineOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			SplineOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			SplineOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			SplineOptionsUpdate::WorkingColors(primary, secondary) => {
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
			SplineToolFsmState::Ready => actions!(SplineToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
			SplineToolFsmState::Drawing => actions!(SplineToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
			SplineToolFsmState::MergingEndpoints => actions!(SplineToolMessageDiscriminant;
				MergeEndpoints,
			),
		}
	}
}

impl ToolTransition for SplineTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context: OverlayContext| SplineToolMessage::Overlays(overlay_context).into()),
			canvas_transformed: Some(SplineToolMessage::CanvasTransformed.into()),
			tool_abort: Some(SplineToolMessage::Abort.into()),
			working_color_changed: Some(SplineToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug)]
enum EndpointPosition {
	Start,
	End,
}

#[derive(Clone, Debug, Default)]
struct SplineToolData {
	/// List of points inserted.
	points: Vec<(PointId, DVec2)>,
	/// Point to be inserted.
	next_point: DVec2,
	/// Point that was inserted temporarily to show preview.
	preview_point: Option<PointId>,
	/// Segment that was inserted temporarily to show preview.
	preview_segment: Option<SegmentId>,
	extend: bool,
	weight: f64,
	/// The layer we are editing.
	current_layer: Option<LayerNodeIdentifier>,
	/// The layers to merge to the current layer before we merge endpoints in merge_endpoint field.
	merge_layers: HashSet<LayerNodeIdentifier>,
	/// The endpoint IDs to merge with the spline's start/end endpoint after spline drawing is finished.
	merge_endpoints: Vec<(EndpointPosition, PointId)>,
	snap_manager: SnapManager,
	auto_panning: AutoPanning,
}

impl SplineToolData {
	fn cleanup(&mut self) {
		self.current_layer = None;
		self.merge_layers = HashSet::new();
		self.merge_endpoints = Vec::new();
		self.preview_point = None;
		self.preview_segment = None;
		self.extend = false;
		self.points = Vec::new();
	}

	/// Get the snapped point while ignoring current layer
	fn snapped_point(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> SnappedPoint {
		let metadata = document.metadata();
		let transform = self.current_layer.map_or(metadata.document_to_viewport, |layer| metadata.transform_to_viewport(layer));
		let point = SnapCandidatePoint::handle(transform.inverse().transform_point2(input.mouse.position));
		let ignore = if let Some(layer) = self.current_layer { vec![layer] } else { vec![] };
		let snap_data = SnapData::ignore(document, input, &ignore);
		self.snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default())
	}
}

impl Fsm for SplineToolFsmState {
	type ToolData = SplineToolData;
	type ToolOptions = SplineOptions;

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

		let ToolMessage::Spline(event) = event else { return self };
		match (self, event) {
			(_, SplineToolMessage::CanvasTransformed) => self,
			(_, SplineToolMessage::Overlays(mut overlay_context)) => {
				path_endpoint_overlays(document, shape_editor, &mut overlay_context, preferences);
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(SplineToolFsmState::MergingEndpoints, SplineToolMessage::MergeEndpoints) => {
				let Some(current_layer) = tool_data.current_layer else { return SplineToolFsmState::Ready };

				if let Some(&layer) = tool_data.merge_layers.iter().last() {
					merge_layers(document, current_layer, layer, responses);
					tool_data.merge_layers.remove(&layer);

					responses.add(SplineToolMessage::MergeEndpoints);
					return SplineToolFsmState::MergingEndpoints;
				}

				let Some((start_endpoint, _)) = tool_data.points.first() else { return SplineToolFsmState::Ready };
				let Some((last_endpoint, _)) = tool_data.points.last() else { return SplineToolFsmState::Ready };

				if let Some((position, second_endpoint)) = tool_data.merge_endpoints.pop() {
					let first_endpoint = match position {
						EndpointPosition::Start => *start_endpoint,
						EndpointPosition::End => *last_endpoint,
					};
					merge_points(document, current_layer, first_endpoint, second_endpoint, responses);

					responses.add(SplineToolMessage::MergeEndpoints);
					return SplineToolFsmState::MergingEndpoints;
				}

				responses.add(DocumentMessage::EndTransaction);
				SplineToolFsmState::Ready
			}
			(SplineToolFsmState::Ready, SplineToolMessage::DragStart { append_to_selected }) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.snap_manager.cleanup(responses);
				tool_data.cleanup();
				tool_data.weight = tool_options.line_weight;

				let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
				let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

				let layers = LayerNodeIdentifier::ROOT_PARENT
					.descendants(document.metadata())
					.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));

				// Extend an endpoint of the selected path
				if let Some((layer, point, position)) = should_extend(document, viewport, SNAP_POINT_TOLERANCE, layers, preferences) {
					if find_spline(document, layer).is_some() {
						// If the point is the part of Spline then we extend it.
						tool_data.current_layer = Some(layer);
						tool_data.points.push((point, position));
						tool_data.next_point = position;
						tool_data.extend = true;

						extend_spline(tool_data, true, responses);

						return SplineToolFsmState::Drawing;
					} else {
						tool_data.merge_layers.insert(layer);
						tool_data.merge_endpoints.push((EndpointPosition::Start, point));
					}
				}

				let selected_nodes = document.network_interface.selected_nodes();
				let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
				let selected_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());

				let append_to_selected_layer = input.keyboard.key(append_to_selected);

				// Create new path in the selected layer when shift is down
				if let (Some(layer), true) = (selected_layer, append_to_selected_layer) {
					tool_data.current_layer = Some(layer);

					let transform = document.metadata().transform_to_viewport(layer);
					let position = transform.inverse().transform_point2(input.mouse.position);
					tool_data.next_point = position;

					return SplineToolFsmState::Drawing;
				}

				responses.add(DocumentMessage::DeselectAllLayers);

				let parent = document.new_layer_bounding_artboard(input);

				let path_node_type = resolve_document_node_type("Path").expect("Path node does not exist");
				let path_node = path_node_type.default_node_template();
				let spline_node_type = resolve_document_node_type("Spline").expect("Spline node does not exist");
				let spline_node = spline_node_type.node_template_input_override([Some(NodeInput::node(NodeId(1), 0))]);
				let nodes = vec![(NodeId(1), path_node), (NodeId(0), spline_node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
				tool_options.fill.apply_fill(layer, responses);
				tool_options.stroke.apply_stroke(tool_data.weight, layer, responses);
				tool_data.current_layer = Some(layer);

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::DragStop) => {
				// The first DragStop event will be ignored to prevent insertion of new point.
				if tool_data.extend {
					tool_data.extend = false;
					return SplineToolFsmState::Drawing;
				}
				if tool_data.current_layer.is_none() {
					return SplineToolFsmState::Ready;
				};
				tool_data.next_point = tool_data.snapped_point(document, input).snapped_point_document;
				if tool_data.points.last().is_none_or(|last_pos| last_pos.1.distance(tool_data.next_point) > DRAG_THRESHOLD) {
					let preview_point = tool_data.preview_point;
					extend_spline(tool_data, false, responses);
					tool_data.preview_point = preview_point;

					if try_merging_lastest_endpoint(document, tool_data, preferences).is_some() {
						responses.add(SplineToolMessage::Confirm);
					}
				}

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerMove) => {
				let Some(layer) = tool_data.current_layer else { return SplineToolFsmState::Ready };
				let ignore = |cp: PointId| tool_data.preview_point.is_some_and(|pp| pp == cp) || tool_data.points.last().is_some_and(|(ep, _)| *ep == cp);
				let join_point = closest_point(document, input.mouse.position, PATH_JOIN_THRESHOLD, vec![layer].into_iter(), ignore, preferences);

				// Endpoints snapping
				if let Some((_, _, point)) = join_point {
					tool_data.next_point = point;
					tool_data.snap_manager.clear_indicator();
				} else {
					let snapped_point = tool_data.snapped_point(document, input);
					tool_data.next_point = snapped_point.snapped_point_document;
					tool_data.snap_manager.update_indicator(snapped_point);
				}

				extend_spline(tool_data, true, responses);

				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				SplineToolFsmState::Drawing
			}
			(_, SplineToolMessage::PointerMove) => {
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerOutsideViewport) => {
				if !input.mouse.mouse_keys.contains(MouseKeys::LEFT) {
					return self;
				}
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				SplineToolFsmState::Drawing
			}
			(state, SplineToolMessage::PointerOutsideViewport) => {
				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::Confirm) => {
				if tool_data.points.len() >= 2 {
					delete_preview(tool_data, responses);
				}
				responses.add(SplineToolMessage::MergeEndpoints);
				SplineToolFsmState::MergingEndpoints
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				SplineToolFsmState::Ready
			}
			(_, SplineToolMessage::WorkingColorChanged) => {
				responses.add(SplineToolMessage::UpdateOptions(SplineOptionsUpdate::WorkingColors(
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
			SplineToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Draw Spline"),
				HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
			])]),
			SplineToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Extend Spline")]),
				HintGroup(vec![HintInfo::keys([Key::Enter], "End Spline")]),
			]),
			SplineToolFsmState::MergingEndpoints => HintData(vec![]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn try_merging_lastest_endpoint(document: &DocumentMessageHandler, tool_data: &mut SplineToolData, preferences: &PreferencesMessageHandler) -> Option<()> {
	if tool_data.points.len() < 2 {
		return None;
	};
	let (last_endpoint, last_endpoint_position) = tool_data.points.last()?;
	let preview_point = tool_data.preview_point;
	let current_layer = tool_data.current_layer?;

	let layers = LayerNodeIdentifier::ROOT_PARENT
		.descendants(document.metadata())
		.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));

	let exclude = |p: PointId| preview_point.is_some_and(|pp| pp == p) || *last_endpoint == p;
	let position = document.metadata().transform_to_viewport(current_layer).transform_point2(*last_endpoint_position);

	let (layer, endpoint, _) = closest_point(document, position, PATH_JOIN_THRESHOLD, layers, exclude, preferences)?;
	tool_data.merge_layers.insert(layer);
	tool_data.merge_endpoints.push((EndpointPosition::End, endpoint));

	Some(())
}

fn extend_spline(tool_data: &mut SplineToolData, show_preview: bool, responses: &mut VecDeque<Message>) {
	delete_preview(tool_data, responses);

	let Some(layer) = tool_data.current_layer else { return };

	let next_point_pos = tool_data.next_point;
	let next_point_id = PointId::generate();
	let modification_type = VectorModificationType::InsertPoint {
		id: next_point_id,
		position: next_point_pos,
	};
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	if let Some((last_point_id, _)) = tool_data.points.last() {
		let points = [*last_point_id, next_point_id];
		let id = SegmentId::generate();
		let modification_type = VectorModificationType::InsertSegment { id, points, handles: [None, None] };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		if show_preview {
			tool_data.preview_segment = Some(id);
		}
	}

	if show_preview {
		tool_data.preview_point = Some(next_point_id);
	} else {
		tool_data.points.push((next_point_id, next_point_pos));
	}
}

fn delete_preview(tool_data: &mut SplineToolData, responses: &mut VecDeque<Message>) {
	let Some(layer) = tool_data.current_layer else { return };

	if let Some(id) = tool_data.preview_point {
		let modification_type = VectorModificationType::RemovePoint { id };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });
	}
	if let Some(id) = tool_data.preview_segment {
		let modification_type = VectorModificationType::RemoveSegment { id };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });
	}

	tool_data.preview_point = None;
	tool_data.preview_segment = None;
}

#[cfg(test)]
mod test_spline_tool {
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::tool::tool_messages::spline_tool::find_spline;
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::PointId;
	use graphene_std::vector::Vector;

	fn assert_point_positions(vector: &Vector, layer_to_viewport: DAffine2, expected_points: &[DVec2], epsilon: f64) {
		let points_in_viewport: Vec<DVec2> = vector
			.point_domain
			.ids()
			.iter()
			.filter_map(|&point_id| {
				let position = vector.point_domain.position_from_id(point_id)?;
				Some(layer_to_viewport.transform_point2(position))
			})
			.collect();

		// Verify each point position is close to the expected position
		for (i, expected_point) in expected_points.iter().enumerate() {
			let actual_point = points_in_viewport[i];
			let distance = (actual_point - *expected_point).length();

			assert!(
				distance < epsilon,
				"Point {} position mismatch: expected {:?}, got {:?} (distance: {})",
				i,
				expected_point,
				actual_point,
				distance
			);
		}
	}

	#[tokio::test]
	async fn test_continue_drawing_from_existing_spline() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let initial_points = [DVec2::new(100., 100.), DVec2::new(200., 150.), DVec2::new(300., 100.)];

		editor.select_tool(ToolType::Spline).await;

		for &point in &initial_points {
			editor.click_tool(ToolType::Spline, MouseKeys::LEFT, point, ModifierKeys::empty()).await;
		}

		editor.press(Key::Enter, ModifierKeys::empty()).await;

		let document = editor.active_document();
		let spline_layer = document
			.metadata()
			.all_layers()
			.find(|layer| find_spline(document, *layer).is_some())
			.expect("Failed to find a layer with a spline node");

		let first_spline_node = find_spline(document, spline_layer).expect("Spline node not found in the layer");

		let first_vector = document.network_interface.compute_modified_vector(spline_layer).expect("Vector not found for the spline layer");

		// Verify initial spline has correct number of points and segments
		let initial_point_count = first_vector.point_domain.ids().len();
		let initial_segment_count = first_vector.segment_domain.ids().len();
		assert_eq!(initial_point_count, 3, "Expected 3 points in initial spline, found {}", initial_point_count);
		assert_eq!(initial_segment_count, 2, "Expected 2 segments in initial spline, found {}", initial_segment_count);

		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		let endpoints: Vec<(PointId, DVec2)> = first_vector
			.extendable_points(false)
			.filter_map(|point_id| first_vector.point_domain.position_from_id(point_id).map(|pos| (point_id, layer_to_viewport.transform_point2(pos))))
			.collect();

		assert_eq!(endpoints.len(), 2, "Expected 2 endpoints in the initial spline");

		let (_, endpoint_position) = endpoints.first().expect("No endpoints found in spline");

		editor.select_tool(ToolType::Spline).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, *endpoint_position, ModifierKeys::empty()).await;

		let continuation_points = [DVec2::new(400., 150.), DVec2::new(500., 100.)];

		for &point in &continuation_points {
			editor.click_tool(ToolType::Spline, MouseKeys::LEFT, point, ModifierKeys::empty()).await;
		}

		editor.press(Key::Enter, ModifierKeys::empty()).await;

		let document = editor.active_document();
		let extended_vector = document
			.network_interface
			.compute_modified_vector(spline_layer)
			.expect("Vector not found for the extended spline layer");

		// Verify extended spline has correct number of points and segments
		let extended_point_count = extended_vector.point_domain.ids().len();
		let extended_segment_count = extended_vector.segment_domain.ids().len();

		assert_eq!(extended_point_count, 5, "Expected 5 points in extended spline, found {}", extended_point_count);
		assert_eq!(extended_segment_count, 4, "Expected 4 segments in extended spline, found {}", extended_segment_count);

		// Verify the spline node is still the same
		let extended_spline_node = find_spline(document, spline_layer).expect("Spline node not found after extension");
		assert_eq!(first_spline_node, extended_spline_node, "Spline node changed after extension");

		// Verify the positions of all points in the extended spline
		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		let all_expected_points = [initial_points[0], initial_points[1], initial_points[2], continuation_points[0], continuation_points[1]];

		assert_point_positions(&extended_vector, layer_to_viewport, &all_expected_points, 1e-10);
	}

	#[tokio::test]
	async fn test_spline_with_zoomed_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Zooming the viewport
		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;

		// Selecting the spline tool
		editor.select_tool(ToolType::Spline).await;

		// Adding points by clicking at different positions
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(50., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(100., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(150., 100.), ModifierKeys::empty()).await;

		// Finish the spline
		editor.handle_message(SplineToolMessage::Confirm).await;

		// Evaluate the graph to ensure everything is processed
		if let Err(e) = editor.eval_graph().await {
			panic!("Graph evaluation failed: {}", e);
		}

		// Get the layer and vector data
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		let layer = network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(network_interface)
			.next()
			.expect("Should have a selected layer");
		let vector = network_interface.compute_modified_vector(layer).expect("Should have vector data");
		let layer_to_viewport = document.metadata().transform_to_viewport(layer);

		// Expected points in viewport coordinates
		let expected_points = vec![DVec2::new(50., 50.), DVec2::new(100., 50.), DVec2::new(150., 100.)];

		// Assert all points are correctly positioned
		assert_point_positions(&vector, layer_to_viewport, &expected_points, 1e-10);
	}

	#[tokio::test]
	async fn test_spline_with_panned_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let pan_amount = DVec2::new(200., 150.);
		editor.handle_message(NavigationMessage::CanvasPan { delta: pan_amount }).await;

		editor.select_tool(ToolType::Spline).await;

		// Add points by clicking at different positions
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(50., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(100., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(150., 100.), ModifierKeys::empty()).await;

		editor.handle_message(SplineToolMessage::Confirm).await;

		// Evaluating the graph to ensure everything is processed
		if let Err(e) = editor.eval_graph().await {
			panic!("Graph evaluation failed: {}", e);
		}

		// Get the layer and vector data
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		let layer = network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(network_interface)
			.next()
			.expect("Should have a selected layer");
		let vector = network_interface.compute_modified_vector(layer).expect("Should have vector data");
		let layer_to_viewport = document.metadata().transform_to_viewport(layer);

		// Expected points in viewport coordinates
		let expected_points = vec![DVec2::new(50., 50.), DVec2::new(100., 50.), DVec2::new(150., 100.)];

		// Assert all points are correctly positioned
		assert_point_positions(&vector, layer_to_viewport, &expected_points, 1e-10);
	}

	#[tokio::test]
	async fn test_spline_with_tilted_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Tilt/rotate the viewport (45 degrees)
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 45_f64.to_radians() }).await;
		editor.select_tool(ToolType::Spline).await;

		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(50., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(100., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(150., 100.), ModifierKeys::empty()).await;

		editor.handle_message(SplineToolMessage::Confirm).await;

		// Evaluating the graph to ensure everything is processed
		if let Err(e) = editor.eval_graph().await {
			panic!("Graph evaluation failed: {}", e);
		}

		// Get the layer and vector data
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		let layer = network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(network_interface)
			.next()
			.expect("Should have a selected layer");
		let vector = network_interface.compute_modified_vector(layer).expect("Should have vector data");
		let layer_to_viewport = document.metadata().transform_to_viewport(layer);

		// Expected points in viewport coordinates
		let expected_points = vec![DVec2::new(50., 50.), DVec2::new(100., 50.), DVec2::new(150., 100.)];

		// Assert all points are correctly positioned
		assert_point_positions(&vector, layer_to_viewport, &expected_points, 1e-10);
	}

	#[tokio::test]
	async fn test_spline_with_combined_transformations() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Applying multiple transformations
		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 1.5 }).await;
		editor.handle_message(NavigationMessage::CanvasPan { delta: DVec2::new(100., 75.) }).await;
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 30_f64.to_radians() }).await;

		editor.select_tool(ToolType::Spline).await;

		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(50., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(100., 50.), ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, DVec2::new(150., 100.), ModifierKeys::empty()).await;

		editor.handle_message(SplineToolMessage::Confirm).await;
		if let Err(e) = editor.eval_graph().await {
			panic!("Graph evaluation failed: {}", e);
		}

		// Get the layer and vector data
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		let layer = network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(network_interface)
			.next()
			.expect("Should have a selected layer");
		let vector = network_interface.compute_modified_vector(layer).expect("Should have vector data");
		let layer_to_viewport = document.metadata().transform_to_viewport(layer);

		// Expected points in viewport coordinates
		let expected_points = vec![DVec2::new(50., 50.), DVec2::new(100., 50.), DVec2::new(150., 100.)];

		// Assert all points are correctly positioned
		assert_point_positions(&vector, layer_to_viewport, &expected_points, 1e-10);
	}

	#[tokio::test]
	async fn test_spline_tool_with_transformed_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Artboard, 0., 0., 500., 500., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let artboard_layer = document.network_interface.selected_nodes().selected_layers(document.metadata()).next().unwrap();

		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer: artboard_layer,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1.5, 1.2), 30_f64.to_radians(), DVec2::new(50., 25.)),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		let spline_points = [DVec2::new(100., 100.), DVec2::new(200., 150.), DVec2::new(300., 100.)];

		editor.draw_spline(&spline_points).await;

		let document = editor.active_document();

		let mut layers = document.metadata().all_layers();
		layers.next();

		let spline_layer = layers.next().expect("Failed to find the spline layer");
		assert!(find_spline(document, spline_layer).is_some(), "Spline node not found in the layer");

		let vector = document.network_interface.compute_modified_vector(spline_layer).expect("Vector not found for the spline layer");

		// Verify we have the correct number of points and segments
		let point_count = vector.point_domain.ids().len();
		let segment_count = vector.segment_domain.ids().len();

		assert_eq!(point_count, 3, "Expected 3 points in the spline, found {}", point_count);
		assert_eq!(segment_count, 2, "Expected 2 segments in the spline, found {}", segment_count);

		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		assert_point_positions(&vector, layer_to_viewport, &spline_points, 1e-10);
	}
}
