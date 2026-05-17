use super::tool_prelude::*;
use crate::consts::{DRAG_THRESHOLD, PATH_JOIN_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::input_mapper::utility_types::input_pointer::MouseKeys;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{resolve_network_node_type, resolve_proto_node_type};
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{
	DrawingToolState, apply_fill_color_pick, apply_fill_enabled, apply_stroke_color_pick, apply_stroke_enabled, apply_working_colors, reset_colors_on_deactivation, swap_fill_and_stroke,
	sync_drawing_state,
};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, find_spline, merge_layers, merge_points};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration, SnappedPoint};
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
		// On tool deactivation (Abort fires from the dispatcher's tool transition), reset the displayed fill/stroke colors so
		// the next activation starts fresh from the current working colors. The global swap state persists across tool switches.
		// Guarded on `Ready` so Esc-mid-drawing (which also fires Abort) doesn't wipe the user's customized fill/stroke options.
		if matches!(&message, ToolMessage::Spline(SplineToolMessage::Abort)) && self.fsm_state == SplineToolFsmState::Ready {
			reset_colors_on_deactivation(&mut self.options.drawing, context.global_tool_data);
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
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
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
			selection_changed: Some(SplineToolMessage::SelectionChanged.into()),
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
	snap_manager: SnapManager,
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
		self.preview_point = None;
		self.preview_segment = None;
		self.extend = false;
		self.points = Vec::new();
	}

	/// Get the snapped point while ignoring current layer
	fn snapped_point(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> SnappedPoint {
		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.pointer.position));
		let ignore = if let Some(layer) = self.current_layer { vec![layer] } else { vec![] };
		let snap_data = SnapData::ignore(document, input, viewport, &ignore);
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
			input,
			shape_editor,
			viewport,
			..
		} = tool_action_data;

		let ToolMessage::Spline(event) = event else { return self };
		match (self, event) {
			(_, SplineToolMessage::CanvasTransformed) => self,
			(_, SplineToolMessage::Overlays { context: mut overlay_context }) => {
				path_endpoint_overlays(document, shape_editor, &mut overlay_context);
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input, viewport), &mut overlay_context);
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
				tool_data.weight = tool_options.drawing.effective_line_weight();

				let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.pointer.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input, viewport), &point, SnapTypeConfiguration::default());
				let viewport_vec = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

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
					let position = transform.inverse().transform_point2(input.pointer.position);
					tool_data.next_point = position;

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
				tool_options.drawing.apply_stroke_to_new_layer(layer, responses);
				tool_options.drawing.fill.apply_fill(layer, responses);
				tool_data.current_layer = Some(layer);
				tool_data.new_layer_viewport_start = Some(viewport_vec);

				// Position the layer at the initial mouse position via Transform
				responses.add(DeferMessage::AfterGraphRun {
					messages: vec![
						GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_translation(viewport_vec),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						}
						.into(),
						NodeGraphMessage::RunDocumentGraph.into(),
					],
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

				// Convert snapped document-space position to layer-local space
				let snapped_document = tool_data.snapped_point(document, input, viewport).snapped_point_document;
				let document_to_viewport = document.metadata().document_to_viewport;
				let viewport_pos = document_to_viewport.transform_point2(snapped_document);

				// For newly created layers, the deferred TransformSet may not yet be reflected
				// in the metadata, so compute local position from the known viewport start.
				tool_data.next_point = if let Some(start) = tool_data.new_layer_viewport_start {
					viewport_pos - start
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
				let join_point = closest_point(document, input.pointer.position, PATH_JOIN_THRESHOLD, vec![layer].into_iter(), ignore);

				// Endpoints snapping - closest_point returns local-space positions
				if let Some((_, _, point)) = join_point {
					tool_data.next_point = point;
					tool_data.snap_manager.clear_indicator();
				} else {
					// Convert snapped document-space position to layer-local space
					let snapped_point = tool_data.snapped_point(document, input, viewport);
					let document_to_viewport = document.metadata().document_to_viewport;
					let viewport_pos = document_to_viewport.transform_point2(snapped_point.snapped_point_document);

					// For newly created layers, the deferred TransformSet may not yet be reflected
					// in the metadata, so compute local position from the known viewport start.
					tool_data.next_point = if let Some(start) = tool_data.new_layer_viewport_start {
						viewport_pos - start
					} else {
						let transform = document.metadata().transform_to_viewport(layer);
						transform.inverse().transform_point2(viewport_pos)
					};
					tool_data.snap_manager.update_indicator(snapped_point);
				}

				extend_spline(tool_data, true, responses);

				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				SplineToolFsmState::Drawing
			}
			(_, SplineToolMessage::PointerMove) => {
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input, viewport), input.pointer.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerOutsideViewport) => {
				if !input.pointer.mouse_keys.contains(MouseKeys::LEFT) {
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
				responses.add(DocumentMessage::AbortTransaction);
				SplineToolFsmState::Ready
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
	let position = document.metadata().transform_to_viewport(current_layer).transform_point2(*last_endpoint_position);

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
				"Point {i} position mismatch: expected {expected_point:?}, got {actual_point:?} (distance: {distance})"
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
		assert_eq!(initial_point_count, 3, "Expected 3 points in initial spline, found {initial_point_count}");
		assert_eq!(initial_segment_count, 2, "Expected 2 segments in initial spline, found {initial_segment_count}");

		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		let endpoints: Vec<(PointId, DVec2)> = first_vector
			.anchor_endpoints()
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

		// Evaluate the graph to ensure everything is processed
		if let Err(e) = editor.eval_graph().await {
			panic!("Graph evaluation failed: {e}");
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
			panic!("Graph evaluation failed: {e}");
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
			panic!("Graph evaluation failed: {e}");
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
			panic!("Graph evaluation failed: {e}");
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

		assert_eq!(point_count, 3, "Expected 3 points in the spline, found {point_count}");
		assert_eq!(segment_count, 2, "Expected 2 segments in the spline, found {segment_count}");

		let layer_to_viewport = document.metadata().transform_to_viewport(spline_layer);

		assert_point_positions(&vector, layer_to_viewport, &spline_points, 1e-10);
	}
}
