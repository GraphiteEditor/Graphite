use super::tool_prelude::*;
use crate::consts::{DRAG_THRESHOLD, PATH_JOIN_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::input_mapper::utility_types::pointer::MouseKeys;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{resolve_network_node_type, resolve_proto_node_type};
use crate::messages::portfolio::document::overlays::utility_functions::open_path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{
	DrawingToolState, apply_fill_color_pick, apply_fill_enabled, apply_stroke_color_pick, apply_stroke_enabled, apply_working_colors, reset_colors_on_deactivation, swap_fill_and_stroke,
	sync_drawing_state,
};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, find_spline, merge_layers, merge_points};
use crate::messages::tool::common_functionality::resize::translation_transform_set;
use crate::messages::tool::common_functionality::snap_session::{Moving, PointOptions, SnapSession, SnapView, snapping_hints};
use crate::messages::tool::common_functionality::stroke_options::{StrokeOptionsUpdate, apply_stroke_option, create_stroke_options_popover_widget};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, should_extend};
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::vector::style::FillChoice;
use graphene_std::vector::{PointId, SegmentId, VectorModificationType};

#[derive(Default, ExtractField)]
pub struct SplineTool {
	fsm_state: SplineToolFsmState,
	tool_data: SplineToolData,
	options: SplineOptions,
}

pub struct SplineOptions {
	drawing: DrawingToolState,
}

impl Default for SplineOptions {
	fn default() -> Self {
		Self {
			drawing: DrawingToolState::new(false),
		}
	}
}

#[impl_message(Message, ToolMessage, Spline)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SplineToolMessage {
	// Standard messages
	Overlays { context: OverlayContext },
	CanvasTransformed,
	Abort,
	SelectionChanged,
	SwitchedToAnotherTool,
	WorkingColorChanged,

	// Tool-specific messages
	Confirm,
	DragStart { append_to_selected: Key },
	DragStop,
	MergeEndpoints,
	PointerMove,
	PointerOutsideViewport,
	Undo,
	UpdateOptions { options: SplineOptionsUpdate },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SplineToolFsmState {
	#[default]
	Ready,
	Drawing,
	MergingEndpoints,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SplineOptionsUpdate {
	FillColor(FillChoice),
	FillEnabled(bool),
	StrokeOption(StrokeOptionsUpdate),
	StrokeColor(Option<Color>),
	StrokeEnabled(bool),
	SwapFillAndStroke,
	WorkingColorsChanged,
}

impl ToolMetadata for SplineTool {
	fn icon_name(&self) -> String {
		"VectorSplineTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Spline Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Spline
	}
}

impl ToolEditTarget for SplineTool {
	fn edit_target(&self) -> EditTarget {
		EditTarget::path()
	}
}

impl LayoutHolder for SplineTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.drawing.fill.create_widgets(
			"Fill:",
			|checkbox: &CheckboxInput| {
				SplineToolMessage::UpdateOptions {
					options: SplineOptionsUpdate::FillEnabled(checkbox.checked),
				}
				.into()
			},
			|color: &ColorInput| {
				SplineToolMessage::UpdateOptions {
					options: SplineOptionsUpdate::FillColor(FillChoice::from(&color.value)),
				}
				.into()
			},
		);

		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.push(
			IconButton::new("SwapHorizontal", 16)
				.tooltip_label("Swap Fill/Stroke Colors")
				.on_update(|_| {
					SplineToolMessage::UpdateOptions {
						options: SplineOptionsUpdate::SwapFillAndStroke,
					}
					.into()
				})
				.widget_instance(),
		);
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

		widgets.append(&mut self.options.drawing.stroke.create_widgets(
			"Stroke:",
			|checkbox: &CheckboxInput| {
				SplineToolMessage::UpdateOptions {
					options: SplineOptionsUpdate::StrokeEnabled(checkbox.checked),
				}
				.into()
			},
			|color: &ColorInput| {
				SplineToolMessage::UpdateOptions {
					options: SplineOptionsUpdate::StrokeColor(color.value.as_solid().map(Color::from)),
				}
				.into()
			},
		));
		let weight_disabled = self.options.drawing.stroke.enabled == Some(false);
		widgets.push(create_stroke_options_popover_widget(&self.options.drawing, weight_disabled, |update| {
			SplineToolMessage::UpdateOptions {
				options: SplineOptionsUpdate::StrokeOption(update),
			}
			.into()
		}));

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		// Switching to another tool resets the fill/stroke colors so the next activation starts from the working colors (the swap state persists)
		if matches!(&message, ToolMessage::Spline(SplineToolMessage::SwitchedToAnotherTool)) {
			reset_colors_on_deactivation(&mut self.options.drawing, context.global_tool_data);
			return;
		}

		if matches!(&message, ToolMessage::Spline(SplineToolMessage::SelectionChanged)) {
			if self.fsm_state != SplineToolFsmState::Ready {
				return;
			}
			if sync_drawing_state(&mut self.options.drawing, false, true, context.global_tool_data, context.document) {
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			return;
		}

		let ToolMessage::Spline(SplineToolMessage::UpdateOptions { options }) = message else {
			let was_drawing = self.fsm_state == SplineToolFsmState::Drawing;
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);

			// However the drawing ends, its snapping gesture ends with it
			if was_drawing && self.fsm_state != SplineToolFsmState::Drawing {
				context.snap_session.end_gesture();
				responses.add(OverlaysMessage::Draw);
			}
			return;
		};
		match options {
			SplineOptionsUpdate::StrokeOption(update) => {
				apply_stroke_option(&mut self.options.drawing, update, context.document, responses);
			}
			SplineOptionsUpdate::FillColor(fill_choice) => {
				apply_fill_color_pick(&mut self.options.drawing, fill_choice, context.document, responses);
			}
			SplineOptionsUpdate::FillEnabled(enabled) => {
				apply_fill_enabled(&mut self.options.drawing, enabled, context.global_tool_data, context.document, responses);
			}
			SplineOptionsUpdate::StrokeColor(color) => {
				apply_stroke_color_pick(&mut self.options.drawing, color, context.document, responses);
			}
			SplineOptionsUpdate::StrokeEnabled(enabled) => {
				apply_stroke_enabled(&mut self.options.drawing, enabled, context.global_tool_data, context.document, responses);
			}
			SplineOptionsUpdate::SwapFillAndStroke => {
				swap_fill_and_stroke(&mut self.options.drawing, context.document, responses);
			}
			SplineOptionsUpdate::WorkingColorsChanged => {
				apply_working_colors(&mut self.options.drawing, context.global_tool_data, context.document);
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
			overlay_provider: Some(|context: OverlayContext| SplineToolMessage::Overlays { context }.into()),
			canvas_transformed: Some(SplineToolMessage::CanvasTransformed.into()),
			tool_abort: Some(SplineToolMessage::Abort.into()),
			switched_to_another_tool: Some(SplineToolMessage::SwitchedToAnotherTool.into()),
			selection_changed: Some(SplineToolMessage::SelectionChanged.into()),
			graph_changed: Some(SplineToolMessage::SelectionChanged.into()),
			working_color_changed: Some(SplineToolMessage::WorkingColorChanged.into()),
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
	/// Where the pointer was pressed to start the spline, and where that press landed, both in viewport space.
	press: Option<(DVec2, DVec2)>,
	auto_panning: AutoPanning,
	/// Viewport-space start position for newly created layers, used to compute local-space
	/// positions before the deferred TransformSet has been reflected in metadata.
	new_layer_viewport_start: Option<DVec2>,
}

impl SplineToolData {
	fn cleanup(&mut self) {
		self.current_layer = None;
		self.new_layer_viewport_start = None;
		self.merge_layers = HashSet::new();
		self.merge_endpoints = Vec::new();
		self.press = None;
		self.preview_point = None;
		self.preview_segment = None;
		self.extend = false;
		self.points = Vec::new();
	}

	/// Snaps the rest of the drawing to everything but the spline being drawn.
	fn begin_snapping_gesture(&self, snap_session: &mut SnapSession, view: SnapView, responses: &mut VecDeque<Message>) {
		let moving = Moving {
			layers: self.current_layer.into_iter().collect(),
			..Default::default()
		};
		snap_session.begin_gesture(view, moving);

		responses.add(OverlaysMessage::Draw);
	}

	/// Snaps the pointer and returns where it lands in viewport space, which stays where the press landed until the pointer moves.
	fn snapped_point(&mut self, snap_session: &mut SnapSession, view: SnapView) -> DVec2 {
		let pointer = view.input.mouse.position;
		self.press = self.press.filter(|&(pressed_at, _)| pressed_at == pointer);
		if let Some((_, landed)) = self.press {
			return landed;
		}

		let document_to_viewport = view.document.metadata().document_to_viewport;
		let free = document_to_viewport.inverse().transform_point2(pointer);
		document_to_viewport.transform_point2(snap_session.snap_point(view, free, PointOptions::default()))
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
			document_id,
			global_tool_data,
			input,
			viewport,
			shape_editor,
			snap_session,
			..
		} = tool_action_data;
		let view = SnapView::new(document, *document_id, input);

		let ToolMessage::Spline(event) = event else { return self };
		match (self, event) {
			(_, SplineToolMessage::CanvasTransformed) => self,
			(_, SplineToolMessage::Overlays { context: mut overlay_context }) => {
				let pointer = (self == SplineToolFsmState::Ready).then_some(input.mouse.position);
				open_path_endpoint_overlays(document, shape_editor, pointer, &mut overlay_context);
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

				tool_data.cleanup();
				tool_data.weight = tool_options.drawing.effective_line_weight();

				// The spline starts on whatever the pointer was shown snapping to before the press
				let document_to_viewport = document.metadata().document_to_viewport;
				let pointer = document_to_viewport.inverse().transform_point2(input.mouse.position);
				let viewport_vec = document_to_viewport.transform_point2(snap_session.snap_press(view, pointer));

				let layers = LayerNodeIdentifier::ROOT_PARENT
					.descendants(document.metadata())
					.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));

				// Extend an endpoint of the selected path
				if let Some((layer, point, position)) = should_extend(document, viewport_vec, SNAP_POINT_TOLERANCE, layers) {
					if find_spline(document, layer).is_some() {
						// If the point is the part of Spline then we extend it.
						tool_data.current_layer = Some(layer);
						tool_data.points.push((point, position));
						tool_data.next_point = position;
						tool_data.extend = true;

						extend_spline(tool_data, true, responses);
						tool_data.begin_snapping_gesture(snap_session, view, responses);

						return SplineToolFsmState::Drawing;
					} else {
						tool_data.merge_layers.insert(layer);
						tool_data.merge_endpoints.push((EndpointPosition::Start, point));
					}
				}

				// A release without a move places the first point where the press landed
				tool_data.press = Some((input.mouse.position, viewport_vec));

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
					tool_data.begin_snapping_gesture(snap_session, view, responses);

					return SplineToolFsmState::Drawing;
				}

				responses.add(DocumentMessage::DeselectAllLayers);

				let parent = document.new_layer_bounding_artboard(input, viewport);

				let path_node_type = resolve_network_node_type("Path").expect("Path node does not exist");
				let path_node = path_node_type.default_node_template();
				let spline_node_type = resolve_proto_node_type(graphene_std::vector::spline::IDENTIFIER).expect("Spline node does not exist");
				let spline_node = spline_node_type.node_template_input_override([Some(NodeInput::node(NodeId(1), 0))]);
				let nodes = vec![(NodeId(1), path_node), (NodeId(0), spline_node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
				tool_options.drawing.apply_fill_to_new_layer(layer, global_tool_data, responses);
				tool_options.drawing.apply_stroke_to_new_layer(layer, global_tool_data, responses);
				tool_options.drawing.apply_stroke_order_to_new_layer(layer, responses);
				tool_data.current_layer = Some(layer);
				tool_data.new_layer_viewport_start = Some(viewport_vec);
				tool_data.begin_snapping_gesture(snap_session, view, responses);

				// Position the layer at the initial mouse position via Transform
				responses.add(DeferMessage::AfterGraphRun {
					messages: vec![translation_transform_set(document, layer, viewport_vec), NodeGraphMessage::RunDocumentGraph.into()],
				});

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::DragStop) => {
				// The first DragStop event will be ignored to prevent insertion of new point.
				if tool_data.extend {
					tool_data.extend = false;
					return SplineToolFsmState::Drawing;
				}
				let Some(layer) = tool_data.current_layer else {
					return SplineToolFsmState::Ready;
				};

				// Convert the snapped position to layer-local space
				let viewport_pos = tool_data.snapped_point(snap_session, view);
				let document_to_viewport = document.metadata().document_to_viewport;

				// For newly created layers, the deferred TransformSet may not yet be reflected
				// in the metadata, so compute local position from the known viewport start.
				tool_data.next_point = if let Some(start) = tool_data.new_layer_viewport_start {
					document_to_viewport.inverse().transform_vector2(viewport_pos - start)
				} else {
					let transform = document.metadata().transform_to_viewport(layer);
					transform.inverse().transform_point2(viewport_pos)
				};
				tool_data.new_layer_viewport_start = None;

				if tool_data.points.last().is_none_or(|last_pos| last_pos.1.distance(tool_data.next_point) > DRAG_THRESHOLD) {
					let preview_point = tool_data.preview_point;
					extend_spline(tool_data, false, responses);
					tool_data.preview_point = preview_point;

					if try_merging_lastest_endpoint(document, tool_data).is_some() {
						responses.add(SplineToolMessage::Confirm);
					}
				}

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerMove) => {
				let Some(layer) = tool_data.current_layer else { return SplineToolFsmState::Ready };
				let ignore = |cp: PointId| tool_data.preview_point.is_some_and(|pp| pp == cp) || tool_data.points.last().is_some_and(|(ep, _)| *ep == cp);
				let join_point = closest_point(document, input.mouse.position, PATH_JOIN_THRESHOLD, vec![layer].into_iter(), ignore);

				// Endpoints snapping - closest_point returns local-space positions
				if let Some((_, _, point)) = join_point {
					tool_data.next_point = point;
					snap_session.forget();
				} else {
					// Convert the snapped position to layer-local space
					let viewport_pos = tool_data.snapped_point(snap_session, view);
					let document_to_viewport = document.metadata().document_to_viewport;

					// For newly created layers, the deferred TransformSet may not yet be reflected
					// in the metadata, so compute local position from the known viewport start.
					tool_data.next_point = if let Some(start) = tool_data.new_layer_viewport_start {
						document_to_viewport.inverse().transform_vector2(viewport_pos - start)
					} else {
						let transform = document.metadata().transform_to_viewport(layer);
						transform.inverse().transform_point2(viewport_pos)
					};
				}

				extend_spline(tool_data, true, responses);

				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				SplineToolFsmState::Drawing
			}
			(_, SplineToolMessage::PointerMove) => {
				// Show where a press would start the spline
				let pointer = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);
				snap_session.snap_point(view, pointer, PointOptions::default());

				responses.add(OverlaysMessage::Draw);
				self
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerOutsideViewport) => {
				if !input.mouse.mouse_keys.contains(MouseKeys::LEFT) {
					return self;
				}
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, viewport, responses);

				SplineToolFsmState::Drawing
			}
			(state, SplineToolMessage::PointerOutsideViewport) => {
				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::Confirm) => {
				if tool_data.points.len() <= 1 {
					responses.add(DocumentMessage::AbortTransaction);
					return SplineToolFsmState::Ready;
				}

				delete_preview(tool_data, responses);

				responses.add(SplineToolMessage::MergeEndpoints);
				SplineToolFsmState::MergingEndpoints
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::Abort) => {
				// Switching tools keeps the spline drawn so far, like the Pen tool, but leaves its ends unjoined as the tool won't be active to finish them
				if tool_data.points.len() <= 1 {
					responses.add(DocumentMessage::AbortTransaction);
				} else {
					delete_preview(tool_data, responses);
					responses.add(DocumentMessage::EndTransaction);
				}

				tool_data.cleanup();
				SplineToolFsmState::Ready
			}
			(SplineToolFsmState::Ready, SplineToolMessage::Confirm | SplineToolMessage::Abort) => {
				// Escape with nothing under way puts back to sleep whatever resting the pointer had woken
				snap_session.sleep_all();
				responses.add(OverlaysMessage::Draw);
				self
			}
			(_, SplineToolMessage::WorkingColorChanged) => {
				responses.add(SplineToolMessage::UpdateOptions {
					options: SplineOptionsUpdate::WorkingColorsChanged,
				});
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			SplineToolFsmState::Ready => HintData(vec![
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Lmb, "Draw Spline"),
					HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
				]),
				snapping_hints(),
			]),
			SplineToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Extend Spline")]),
				HintGroup(vec![HintInfo::keys([Key::Enter], "End Spline")]),
				snapping_hints(),
			]),
			SplineToolFsmState::MergingEndpoints => HintData(vec![]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn try_merging_lastest_endpoint(document: &DocumentMessageHandler, tool_data: &mut SplineToolData) -> Option<()> {
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
	let position = document
		.metadata()
		.transform_to_viewport_if_feeds(current_layer, &document.network_interface)
		.transform_point2(*last_endpoint_position);

	let (layer, endpoint, _) = closest_point(document, position, PATH_JOIN_THRESHOLD, layers, exclude)?;
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
	use crate::messages::tool::common_functionality::snap_session::WAKE_DWELL_ON_APPROACH;
	use crate::messages::tool::tool_messages::spline_tool::find_spline;
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::PointId;
	use graphene_std::vector::VectorGeometry;

	fn assert_point_positions(vector: &VectorGeometry, layer_to_viewport: DAffine2, expected_points: &[DVec2], epsilon: f64) {
		let points_in_viewport: Vec<DVec2> = vector
			.anchor_points()
			.filter_map(|point_id| {
				let position = vector.position_from_id(point_id)?;
				Some(layer_to_viewport.transform_point2(position))
			})
			.collect();

		// Verify each point position is close to the expected position
		for (i, expected_point) in expected_points.iter().enumerate() {
			let actual_point = points_in_viewport[i];
			let distance = (actual_point - *expected_point).length();

			assert!(
				distance < epsilon,
				"Point {i} position mismatch: expected {expected_point:?}, got {actual_point:?} (distance: {distance})"
			);
		}
	}

	/// The layers drawn by the Spline tool.
	fn spline_layers(editor: &EditorTestUtils) -> Vec<LayerNodeIdentifier> {
		let document = editor.active_document();
		document.metadata().all_layers().filter(|&layer| find_spline(document, layer).is_some()).collect()
	}

	/// Asserts a spline's points sit at the given viewport positions to within a millionth of a pixel, in the order they were placed.
	fn assert_spline_points(editor: &EditorTestUtils, layer: LayerNodeIdentifier, expected: &[DVec2]) {
		let document = editor.active_document();
		let vector = document.network_interface.compute_modified_vector(layer).expect("The spline should have a path");

		assert_eq!(vector.anchor_points().count(), expected.len(), "Unexpected point count");
		assert_point_positions(&vector, document.metadata().transform_to_viewport(layer), expected, 1e-6);
	}

	/// The center of a layer's bounding box in the viewport.
	fn bounding_box_center(editor: &EditorTestUtils, layer: LayerNodeIdentifier) -> DVec2 {
		let [min, max] = editor.active_document().metadata().bounding_box_viewport(layer).expect("The layer should have bounds");
		(min + max) / 2.
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
		let initial_point_count = first_vector.anchor_points().count();
		let initial_segment_count = first_vector.segment_ids().len();
		assert_eq!(initial_point_count, 3, "Expected 3 points in initial spline, found {initial_point_count}");
		assert_eq!(initial_segment_count, 2, "Expected 2 segments in initial spline, found {initial_segment_count}");

		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		let endpoints: Vec<(PointId, DVec2)> = first_vector
			.anchor_endpoints()
			.filter_map(|point_id| first_vector.position_from_id(point_id).map(|pos| (point_id, layer_to_viewport.transform_point2(pos))))
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
		let extended_point_count = extended_vector.anchor_points().count();
		let extended_segment_count = extended_vector.segment_ids().len();

		assert_eq!(extended_point_count, 5, "Expected 5 points in extended spline, found {extended_point_count}");
		assert_eq!(extended_segment_count, 4, "Expected 4 segments in extended spline, found {extended_segment_count}");

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
		let point_count = vector.anchor_points().count();
		let segment_count = vector.segment_ids().len();

		assert_eq!(point_count, 3, "Expected 3 points in the spline, found {point_count}");
		assert_eq!(segment_count, 2, "Expected 2 segments in the spline, found {segment_count}");

		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		assert_point_positions(&vector, layer_to_viewport, &spline_points, 1e-10);
	}

	/// A spline through three points, the middle one well above the other two.
	const ARCH: [DVec2; 3] = [DVec2::new(100., 300.), DVec2::new(250., 150.), DVec2::new(400., 300.)];

	#[tokio::test]
	async fn a_spline_pressed_near_the_corner_of_another_layer_starts_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// The corner of an ellipse's bounding box, which unlike an anchor is nothing a spline joins to
		editor.draw_ellipse(300., 200., 500., 320.).await;
		let corner = DVec2::new(300., 200.);

		let points = [corner + DVec2::new(4., 3.), DVec2::new(150., 100.), DVec2::new(100., 250.)];
		editor.draw_spline(&points).await;

		let splines = spline_layers(&editor);
		assert_eq!(splines.len(), 1);
		assert_spline_points(&editor, splines[0], &[corner, points[1], points[2]]);
	}

	#[tokio::test]
	async fn a_spline_pressed_while_a_snap_is_held_starts_where_that_snap_was_shown() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_ellipse(300., 200., 500., 320.).await;
		let corner = DVec2::new(300., 200.);

		// The pointer comes within reach of the corner, then strays to where only a snap already engaged holds on
		editor.select_tool(ToolType::Spline).await;
		editor.move_mouse(306., 204., ModifierKeys::empty(), MouseKeys::empty()).await;
		let points = [corner + DVec2::new(9., 5.), DVec2::new(150., 100.), DVec2::new(100., 250.)];
		editor.draw_spline(&points).await;

		let splines = spline_layers(&editor);
		assert_eq!(splines.len(), 1);
		assert_spline_points(&editor, splines[0], &[corner, points[1], points[2]]);
	}

	#[tokio::test]
	async fn a_spline_starts_where_its_press_was_shown_after_resting_there_wakes_a_nearer_snap() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// The corner of an ellipse's bounding box, which a spline doesn't join to, sits six pixels left of a rectangle's side, kept apart by drawing them with snapping off
		editor.active_document_mut().snapping_state.snapping_enabled = false;
		editor.draw_rect(206., 160., 300., 300.).await;
		editor.draw_ellipse(100., 100., 200., 200.).await;
		editor.active_document_mut().snapping_state.snapping_enabled = true;
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;
		editor.select_tool(ToolType::Spline).await;

		// The pointer rests on the side beside the corner, where a press is shown starting on the corner, for long enough to wake its box
		let (corner, pointer) = (DVec2::new(200., 200.), DVec2::new(206., 200.));
		editor.handle_message(InputPreprocessorMessage::CurrentTime { timestamp: 1000 }).await;
		editor.move_mouse(pointer.x, pointer.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor
			.handle_message(InputPreprocessorMessage::CurrentTime {
				timestamp: 1000 + WAKE_DWELL_ON_APPROACH,
			})
			.await;
		editor.handle_message(BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame)).await;

		// The woken corner's level line crosses the side right under the pointer, but the spline starts where the press was shown
		editor.left_mousedown(pointer.x, pointer.y, ModifierKeys::empty()).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;
		let later_points = [DVec2::new(150., 400.), DVec2::new(100., 450.)];
		for point in later_points {
			editor.click_tool(ToolType::Spline, MouseKeys::LEFT, point, ModifierKeys::empty()).await;
		}
		editor.press(Key::Enter, ModifierKeys::empty()).await;

		let splines = spline_layers(&editor);
		assert_eq!(splines.len(), 1);
		assert_spline_points(&editor, splines[0], &[corner, later_points[0], later_points[1]]);
	}

	#[tokio::test]
	async fn a_later_point_placed_near_the_middle_of_an_edge_of_another_layer_lands_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// The middle of a rectangle's top edge, which unlike its corners is nothing a spline joins to
		editor.draw_rect(100., 200., 300., 400.).await;
		let middle = DVec2::new(200., 200.);

		let points = [DVec2::new(60., 80.), middle + DVec2::new(3., -4.), DVec2::new(420., 120.)];
		editor.draw_spline(&points).await;

		let splines = spline_layers(&editor);
		assert_eq!(splines.len(), 1);
		assert_spline_points(&editor, splines[0], &[points[0], middle, points[2]]);
	}

	#[tokio::test]
	async fn a_spline_being_extended_does_not_snap_to_its_own_points() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_spline(&ARCH).await;

		// Continued from its end, a point goes beside its middle point, out of reach of joining it but within reach of snapping to it
		let beside_the_middle = ARCH[1] + DVec2::new(6., 2.);
		let continuation = [ARCH[2], beside_the_middle, DVec2::new(550., 150.)];
		editor.draw_spline(&continuation).await;

		let splines = spline_layers(&editor);
		assert_eq!(splines.len(), 1);
		assert_spline_points(&editor, splines[0], &[ARCH[0], ARCH[1], ARCH[2], beside_the_middle, continuation[2]]);
	}

	#[tokio::test]
	async fn a_finished_spline_is_snapped_to_by_the_next_one() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_spline(&ARCH).await;
		let first = spline_layers(&editor)[0];

		// The center of the first spline's bounding box, far from the points a spline joins to
		let center = bounding_box_center(&editor, first);
		let points = [center + DVec2::new(4., -3.), DVec2::new(600., 80.), DVec2::new(650., 400.)];
		editor.draw_spline(&points).await;

		let second = spline_layers(&editor).into_iter().find(|&layer| layer != first).expect("A second spline should have been drawn");
		assert_spline_points(&editor, second, &[center, points[1], points[2]]);
	}

	#[tokio::test]
	async fn after_an_extension_is_aborted_the_next_spline_snaps_to_the_spline_it_was_extending() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_spline(&ARCH).await;
		let first = spline_layers(&editor)[0];

		// Aborted before placing a point of its own, the extension is dropped
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, ARCH[2], ModifierKeys::empty()).await;
		editor.move_mouse(500., 200., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.handle_message(SplineToolMessage::Abort).await;
		assert_spline_points(&editor, first, &ARCH);

		let center = bounding_box_center(&editor, first);
		let points = [center + DVec2::new(4., -3.), DVec2::new(600., 80.), DVec2::new(650., 400.)];
		editor.draw_spline(&points).await;

		let second = spline_layers(&editor).into_iter().find(|&layer| layer != first).expect("A second spline should have been drawn");
		assert_spline_points(&editor, second, &[center, points[1], points[2]]);
	}

	#[tokio::test]
	async fn switching_tools_mid_spline_keeps_the_spline() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.select_tool(ToolType::Spline).await;
		for point in ARCH {
			editor.click_tool(ToolType::Spline, MouseKeys::LEFT, point, ModifierKeys::empty()).await;
		}
		editor.move_mouse(500., 200., ModifierKeys::empty(), MouseKeys::empty()).await;

		// The Select tool's own abort, which runs as it becomes active, would otherwise cancel the spline's open edit
		editor.select_tool(ToolType::Select).await;

		let splines = spline_layers(&editor);
		assert_eq!(splines.len(), 1);
		assert_spline_points(&editor, splines[0], &ARCH);

		// The finished spline is one step of history
		editor.press(Key::KeyZ, ModifierKeys::CONTROL).await;
		assert!(spline_layers(&editor).is_empty(), "Undo should remove the whole spline");
	}

	#[tokio::test]
	async fn an_extension_is_kept_when_switching_tools() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_spline(&ARCH).await;
		let first = spline_layers(&editor)[0];

		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, ARCH[2], ModifierKeys::empty()).await;
		let extension = DVec2::new(500., 200.);
		editor.click_tool(ToolType::Spline, MouseKeys::LEFT, extension, ModifierKeys::empty()).await;
		editor.select_tool(ToolType::Rectangle).await;

		assert_spline_points(&editor, first, &[ARCH[0], ARCH[1], ARCH[2], extension]);
	}
}
