use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, DRAG_THRESHOLD, PATH_JOIN_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, find_spline, merge_layers};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration, SnappedPoint};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, should_extend};

use graph_craft::document::{NodeId, NodeInput};
use graphene_core::Color;
use graphene_std::vector::{ManipulatorPointId, PointId, SegmentId, VectorModificationType};

#[derive(Default)]
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
	Merge,
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
			|color: &ColorInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Spline(SplineToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
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
			SplineToolFsmState::Merge => actions!(SplineToolMessageDiscriminant;
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
	/// The layer and its endpoint we will merge with the start point of the spline.
	merge_layer: Option<(LayerNodeIdentifier, PointId)>,
	/// The endpoints to merge after we have finised drawing the spline and merged the layers.
	merge_points: Vec<(PointId, PointId)>,
	snap_manager: SnapManager,
	auto_panning: AutoPanning,
}

impl SplineToolData {
	fn cleanup(&mut self) {
		self.current_layer = None;
		self.merge_layer = None;
		self.merge_points = Vec::new();
		self.preview_point = None;
		self.preview_segment = None;
		self.extend = false;
		self.points = Vec::new();
	}

	/// Get the snapped point while ignoring current layer
	fn snapped_point(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> SnappedPoint {
		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
		let ignore = if let Some(layer) = self.current_layer { vec![layer] } else { vec![] };
		let snap_data = SnapData::ignore(document, input, &ignore);
		self.snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default())
	}
}

impl Fsm for SplineToolFsmState {
	type ToolData = SplineToolData;
	type ToolOptions = SplineOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
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
			(SplineToolFsmState::Merge, SplineToolMessage::MergeEndpoints) => {
				let Some(layer) = tool_data.current_layer else { return SplineToolFsmState::Ready };

				if let Some((first, second)) = tool_data.merge_points.pop() {
					merge_points(document, layer, first, second, responses);
					responses.add(SplineToolMessage::MergeEndpoints);
					return SplineToolFsmState::Merge;
				}

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
						// Otherwise we keep the layer identifier and endpoint identifier to merge it with the spline later when we finish drawing the spline.
						tool_data.merge_layer = Some((layer, point));
					}
				}

				// Create new path in the same layer when shift is down
				if input.keyboard.key(append_to_selected) {
					// FIX:
					let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
					let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
					let existing_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());

					if let Some(layer) = existing_layer {
						tool_data.current_layer = Some(layer);

						let transform = document.metadata().transform_to_viewport(layer);
						let position = transform.inverse().transform_point2(input.mouse.position);
						tool_data.next_point = position;

						return SplineToolFsmState::Drawing;
					}
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

				responses.add(Message::StartBuffer);

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
				if tool_data.points.last().map_or(true, |last_pos| last_pos.1.distance(tool_data.next_point) > DRAG_THRESHOLD) {
					extend_spline(tool_data, false, responses);
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
					join_path(document, tool_data, preferences, responses);
				}
				responses.add(DocumentMessage::EndTransaction);
				responses.add(SplineToolMessage::MergeEndpoints);
				SplineToolFsmState::Merge
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

	fn update_hints(&self, responses: &mut VecDeque<Message>, _tool_data: &Self::ToolData) {
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
			_ => HintData(vec![]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn join_path(document: &DocumentMessageHandler, tool_data: &mut SplineToolData, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) {
	if tool_data.points.len() < 2 {
		return;
	};
	let (last_endpoint, last_endpoint_position) = tool_data.points.last().unwrap();
	let (start_endpoint, _) = tool_data.points.first().unwrap();
	let preview_point = tool_data.preview_point;
	let Some(current_layer) = tool_data.current_layer else { return };

	let layers = LayerNodeIdentifier::ROOT_PARENT
		.descendants(document.metadata())
		.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));

	let exclude = |p: PointId| preview_point.is_some_and(|pp| pp == p) || *last_endpoint == p;
	let position = document.metadata().transform_to_viewport(current_layer).transform_point2(*last_endpoint_position);

	let closest_point = closest_point(document, position, PATH_JOIN_THRESHOLD, layers, exclude, preferences);

	let mut endpoints_to_merge = [None, None];

	match (tool_data.merge_layer, closest_point) {
		(Some(first), Some(last)) => {
			merge_layers(document, first.0, last.0, responses);
			merge_layers(document, current_layer, first.0, responses);
			endpoints_to_merge = [first.1, last.1].map(|p| Some(p));
		}
		(Some(first), None) => {
			endpoints_to_merge[0] = Some(first.1);
			merge_layers(document, current_layer, first.0, responses);
		}
		(None, Some(last)) => {
			endpoints_to_merge[1] = Some(last.1);
			merge_layers(document, current_layer, last.0, responses);
		}
		(None, None) => return,
	}

	if let Some(merge_endpoint) = endpoints_to_merge[0] {
		tool_data.merge_points.push((*start_endpoint, merge_endpoint));
	}
	if let Some(merge_endpoint) = endpoints_to_merge[1] {
		tool_data.merge_points.push((*last_endpoint, merge_endpoint));
	}
}

/// Merges `first_endpoint` in `first_layer` with `second_endpoint` in `second_layer`.
/// The layers must be merged before this function is called.
/// For merging endpoints within the same layer, `first_layer` and `second_layer` will be the same.
fn merge_points(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, first_endpoint: PointId, second_endpont: PointId, responses: &mut VecDeque<Message>) {
	let transform = document.metadata().transform_to_document(layer);
	let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else { return };

	let Some((segment, _, mut start_point, mut end_point)) = vector_data.segment_bezier_iter().find(|(_, _, start, end)| *end == second_endpont || *start == second_endpont) else {
		log::error!("Could not get the segment for second_endpoint.");
		return;
	};

	let mut handles = [None; 2];
	if let Some(handle_position) = ManipulatorPointId::PrimaryHandle(segment).get_position(&vector_data) {
		let anchor_position = ManipulatorPointId::Anchor(start_point).get_position(&vector_data).unwrap();
		let handle_position = transform.transform_point2(handle_position);
		let anchor_position = transform.transform_point2(anchor_position);
		let anchor_to_handle = handle_position - anchor_position;
		handles[0] = Some(anchor_to_handle);
	}
	if let Some(handle_position) = ManipulatorPointId::EndHandle(segment).get_position(&vector_data) {
		let anchor_position = ManipulatorPointId::Anchor(end_point).get_position(&vector_data).unwrap();
		let handle_position = transform.transform_point2(handle_position);
		let anchor_position = transform.transform_point2(anchor_position);
		let anchor_to_handle = handle_position - anchor_position;
		handles[1] = Some(anchor_to_handle);
	}

	if start_point == second_endpont {
		core::mem::swap(&mut start_point, &mut end_point);
		handles.reverse();
	}

	let modification_type = VectorModificationType::RemovePoint { id: second_endpont };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });
	let modification_type = VectorModificationType::RemoveSegment { id: segment };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let points = [start_point, first_endpoint];
	let id = SegmentId::generate();
	let modification_type = VectorModificationType::InsertSegment { id, points, handles };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });
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
