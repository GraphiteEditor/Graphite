use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, DRAG_THRESHOLD, PATH_JOIN_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{self, resolve_document_node_type};
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, InputConnector};
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration, SnappedPoint};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, should_extend};

use graph_craft::document::{NodeId, NodeInput};
use graphene_core::Color;
use graphene_std::vector::{PointId, SegmentId, VectorModificationType};

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
	layer: Option<LayerNodeIdentifier>,
	starting_layer: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
	auto_panning: AutoPanning,
}

impl SplineToolData {
	fn cleanup(&mut self) {
		self.layer = None;
		self.preview_point = None;
		self.preview_segment = None;
		self.extend = false;
		self.points = Vec::new();
		self.starting_layer = None;
	}

	/// Get the snapped point while ignoring current layer
	fn snapped_point(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> SnappedPoint {
		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
		let ignore = if let Some(layer) = self.layer { vec![layer] } else { vec![] };
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
			(SplineToolFsmState::Ready, SplineToolMessage::DragStart { append_to_selected }) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.cleanup();
				tool_data.weight = tool_options.line_weight;

				let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
				let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

				// Check if we're starting from an endpoint of any layer, even if not extending
				let closest_endpoint = closest_point(
					document,
					viewport,
					PATH_JOIN_THRESHOLD,
					LayerNodeIdentifier::ROOT_PARENT.descendants(document.metadata()),
					|_| false, // Don't exclude any points
					preferences,
				);
				if let Some((start_layer, _, _)) = closest_endpoint {
					tool_data.starting_layer = Some(start_layer);
				}

				// Extend an endpoint of the selected path
				let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
				if let Some((layer, point, position)) = should_extend(document, viewport, SNAP_POINT_TOLERANCE, selected_nodes.selected_layers(document.metadata()), preferences) {
					tool_data.layer = Some(layer);
					tool_data.points.push((point, position));
					tool_data.next_point = position;
					tool_data.extend = true;

					extend_spline(tool_data, true, responses);

					return SplineToolFsmState::Drawing;
				}

				// Create new path in the same layer when shift is down
				if input.keyboard.key(append_to_selected) {
					let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
					let existing_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());
					if let Some(layer) = existing_layer {
						tool_data.layer = Some(layer);

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
				tool_data.layer = Some(layer);

				responses.add(Message::StartBuffer);

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::DragStop) => {
				// The first DragStop event will be ignored to prevent insertion of new point.
				if tool_data.extend {
					tool_data.extend = false;
					return SplineToolFsmState::Drawing;
				}
				if tool_data.layer.is_none() {
					return SplineToolFsmState::Ready;
				};
				if join_path(document, input.mouse.position, tool_data, preferences, responses) {
					responses.add(DocumentMessage::EndTransaction);
					return SplineToolFsmState::Ready;
				}
				tool_data.next_point = tool_data.snapped_point(document, input).snapped_point_document;
				if tool_data.points.last().map_or(true, |last_pos| last_pos.1.distance(tool_data.next_point) > DRAG_THRESHOLD) {
					extend_spline(tool_data, false, responses);
				}

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerMove) => {
				let Some(layer) = tool_data.layer else { return SplineToolFsmState::Ready };
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
			(SplineToolFsmState::Drawing, SplineToolMessage::Confirm | SplineToolMessage::Abort) => {
				if tool_data.points.len() >= 2 {
					delete_preview(tool_data, responses);
					responses.add(DocumentMessage::EndTransaction);
				} else {
					responses.add(DocumentMessage::AbortTransaction);
				}

				tool_data.snap_manager.cleanup(responses);
				tool_data.cleanup();

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
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

/// Return `true` only if new segment is inserted to connect two end points in the selected layer otherwise `false`.
fn join_path(document: &DocumentMessageHandler, mouse_pos: DVec2, tool_data: &mut SplineToolData, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> bool {
	let Some(&(endpoint, _)) = tool_data.points.last() else { return false };
	let Some(&(start_point, _)) = tool_data.points.first() else { return false };
	let Some(starting_layer) = tool_data.starting_layer else { return false };
	let Some(current_layer) = tool_data.layer else { return false };
	let preview_point = tool_data.preview_point;

	// Get the closest point to mouse position which is not preview_point or end_point.
	let closest_point = closest_point(
		document,
		mouse_pos,
		PATH_JOIN_THRESHOLD,
		LayerNodeIdentifier::ROOT_PARENT.descendants(document.metadata()),
		|cp| preview_point.is_some_and(|pp| pp == cp) || cp == endpoint,
		preferences,
	);
	let Some((other_layer, join_point, _)) = closest_point else { return false };

	// Last end point inserted was the preview point and segment therefore we delete it before joining the end_point & join_point.
	delete_preview(tool_data, responses);

	// If the points are in different layers, merge them first
	if current_layer == other_layer {
		// If points are in the same layer, just connect them
		let points = [endpoint, join_point];
		let id = SegmentId::generate();
		let modification_type = VectorModificationType::InsertSegment { id, points, handles: [None, None] };
		responses.add(GraphOperationMessage::Vector {
			layer: current_layer,
			modification_type,
		});

		return true;
	}

	match (is_layer_spline(document, starting_layer), is_layer_spline(document, other_layer)) {
		(true, true) => {
			merge_two_spline_layer(document, current_layer, other_layer, responses);
			let points = [endpoint, join_point];
			let id = SegmentId::generate();
			let modification_type = VectorModificationType::InsertSegment { id, points, handles: [None, None] };
			responses.add(GraphOperationMessage::Vector {
				layer: current_layer,
				modification_type,
			});
		}
		(false, false) => {
			let Some(current_vector_data) = document.network_interface.compute_modified_vector(current_layer) else {
				log::error!("Could not get vector data for current layer");
				return false;
			};
			let Some(starting_vector_data) = document.network_interface.compute_modified_vector(starting_layer) else {
				log::error!("Could not get vector data for other layer");
				return false;
			};

			let Some(starting_layer_endpoint) = starting_vector_data.end_point().last() else {
				log::error!("Could not get endpoint");
				return false;
			};

			let handles = (0..current_vector_data.segment_domain.handles().len())
				.find_map(|index| {
					let (start_id, end_id, bezier) = current_vector_data.segment_points_from_index(index);
					if start_id == endpoint {
						Some([bezier.handles.start(), bezier.handles.end()])
					} else if end_id == endpoint {
						// Reverse the handles if connecting to end point
						Some([bezier.handles.end(), bezier.handles.start()])
					} else {
						None
					}
				})
				.unwrap();

			// Merge the layers first
			merge_non_spline_layers(document, starting_layer, current_layer, other_layer, responses);

			let points = [endpoint, join_point];
			let points2 = [start_point, starting_layer_endpoint];
			let id = SegmentId::generate();
			let modification_type = VectorModificationType::InsertSegment { id, points, handles };
			responses.add(GraphOperationMessage::Vector {
				layer: starting_layer,
				modification_type,
			});

			let id = SegmentId::generate();
			let modification_type = VectorModificationType::InsertSegment { id, points: points2, handles };
			responses.add(GraphOperationMessage::Vector {
				layer: starting_layer,
				modification_type,
			});
		}
		_ => {
			let current_vector_data = match document.network_interface.compute_modified_vector(current_layer) {
				Some(data) => data,
				None => {
					log::error!("Could not get vector data for current layer");
					return false;
				}
			};

			let handles = (0..current_vector_data.segment_domain.handles().len()).find_map(|index| {
				let (start_id, end_id, bezier) = current_vector_data.segment_points_from_index(index);
				if start_id == endpoint {
					Some([bezier.handles.start(), bezier.handles.end()])
				} else if end_id == endpoint {
					Some([bezier.handles.end(), bezier.handles.start()])
				} else {
					None
				}
			});

			let Some(handles) = handles else {
				log::error!("Could not find handles for endpoint");
				return false;
			};

			merge_path_spline_layer(document, current_layer, other_layer, responses);

			let points = [endpoint, join_point];
			let id = SegmentId::generate();
			responses.add(GraphOperationMessage::Vector {
				layer: other_layer,
				modification_type: VectorModificationType::InsertSegment { id, points, handles },
			});
		}
	}

	true
}

fn extend_spline(tool_data: &mut SplineToolData, show_preview: bool, responses: &mut VecDeque<Message>) {
	delete_preview(tool_data, responses);

	let Some(layer) = tool_data.layer else { return };

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
	let Some(layer) = tool_data.layer else { return };

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

fn merge_two_spline_layer(document: &DocumentMessageHandler, current_layer: LayerNodeIdentifier, other_layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	// Calculate the downstream transforms in order to bring the other vector data into the same layer space
	let current_transform = document.metadata().downstream_transform_to_document(current_layer);
	let other_transform = document.metadata().downstream_transform_to_document(other_layer);

	// Represents the change in position that would occur if the other layer was moved below the current layer
	let transform_delta = current_transform * other_transform.inverse();
	let offset = transform_delta.inverse();
	responses.add(GraphOperationMessage::TransformChange {
		layer: other_layer,
		transform: offset,
		transform_in: TransformIn::Local,
		skip_rerender: false,
	});

	// First find their IDs
	let current_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(
			vec![current_layer.to_node()],
			&[],
			crate::messages::portfolio::document::utility_types::network_interface::FlowType::HorizontalFlow,
		)
		.collect::<Vec<_>>();

	let other_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(vec![other_layer.to_node()], &[], FlowType::HorizontalFlow)
		.collect::<Vec<_>>();

	// Add merge node and insert between path and spline
	let merge_node_id = NodeId::new();
	let merge_node = document_node_definitions::resolve_document_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id,
		node_template: merge_node,
	});
	responses.add(NodeGraphMessage::SetToNodeOrLayer {
		node_id: merge_node_id,
		is_layer: false,
	});

	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: merge_node_id,
		parent: current_layer,
	});

	responses.add(NodeGraphMessage::ConnectUpstreamOutputToInput {
		downstream_input: InputConnector::node(other_layer.to_node(), 1),
		input_connector: InputConnector::node(merge_node_id, 1),
	});

	// Add flatten vector elements node after merge
	let flatten_node_id = NodeId::new();
	let flatten_node = document_node_definitions::resolve_document_node_type("Flatten Vector Elements")
		.expect("Failed to create flatten node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: flatten_node_id,
		node_template: flatten_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: flatten_node_id,
		parent: current_layer,
	});

	let path_node_id = NodeId::new();
	let path_node = document_node_definitions::resolve_document_node_type("Path")
		.expect("Failed to create path node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: path_node_id,
		node_template: path_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: path_node_id,
		parent: current_layer,
	});

	let spline_node_id = NodeId::new();
	let spline_node = document_node_definitions::resolve_document_node_type("Splines from Points")
		.expect("Failed to create spline node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: spline_node_id,
		node_template: spline_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: spline_node_id,
		parent: current_layer,
	});

	let stroke_node_id = NodeId::new();
	let stroke_node = document_node_definitions::resolve_document_node_type("Stroke")
		.expect("Failed to create stroke node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: stroke_node_id,
		node_template: stroke_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: stroke_node_id,
		parent: current_layer,
	});

	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: current_layer_nodes[1..3].to_vec(),
		delete_children: false,
	});

	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: other_layer_nodes[..3].to_vec(),
		delete_children: false,
	});

	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(Message::StartBuffer);
}

fn merge_non_spline_layers(
	document: &DocumentMessageHandler,
	starting_layer: LayerNodeIdentifier,
	current_layer: LayerNodeIdentifier,
	other_layer: LayerNodeIdentifier,
	responses: &mut VecDeque<Message>,
) {
	// Calculate the downstream transforms in order to bring the other vector data into the same layer space
	let current_transform = document.metadata().downstream_transform_to_document(current_layer);
	let other_transform = document.metadata().downstream_transform_to_document(other_layer);

	// Represents the change in position that would occur if the other layer was moved below the current layer
	let transform_delta = current_transform * other_transform.inverse();
	let offset = transform_delta.inverse();
	responses.add(GraphOperationMessage::TransformChange {
		layer: other_layer,
		transform: offset,
		transform_in: TransformIn::Local,
		skip_rerender: false,
	});

	// Delete spline and stroke nodes from both layers
	// First find their IDs
	let starting_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(
			vec![starting_layer.to_node()],
			&[],
			crate::messages::portfolio::document::utility_types::network_interface::FlowType::HorizontalFlow,
		)
		.collect::<Vec<_>>();

	let current_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(
			vec![current_layer.to_node()],
			&[],
			crate::messages::portfolio::document::utility_types::network_interface::FlowType::HorizontalFlow,
		)
		.collect::<Vec<_>>();

	let other_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(vec![other_layer.to_node()], &[], FlowType::HorizontalFlow)
		.collect::<Vec<_>>();

	// Add merge node and insert between path and spline
	let merge_node_id = NodeId::new();
	let merge_node = document_node_definitions::resolve_document_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id,
		node_template: merge_node,
	});
	responses.add(NodeGraphMessage::SetToNodeOrLayer {
		node_id: merge_node_id,
		is_layer: false,
	});

	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: merge_node_id,
		parent: starting_layer,
	});
	responses.add(NodeGraphMessage::ConnectUpstreamOutputToInput {
		downstream_input: InputConnector::node(other_layer.to_node(), 1),
		input_connector: InputConnector::node(merge_node_id, 1),
	});

	let merge_node_id2 = NodeId::new();
	let merge_node2 = document_node_definitions::resolve_document_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id2,
		node_template: merge_node2,
	});
	responses.add(NodeGraphMessage::SetToNodeOrLayer {
		node_id: merge_node_id2,
		is_layer: false,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: merge_node_id2,
		parent: starting_layer,
	});
	responses.add(NodeGraphMessage::ConnectUpstreamOutputToInput {
		downstream_input: InputConnector::node(current_layer.to_node(), 1),
		input_connector: InputConnector::node(merge_node_id2, 1),
	});
	// Add flatten vector elements node after merge
	let flatten_node_id2 = NodeId::new();
	let flatten_node2 = document_node_definitions::resolve_document_node_type("Flatten Vector Elements")
		.expect("Failed to create flatten node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: flatten_node_id2,
		node_template: flatten_node2,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: flatten_node_id2,
		parent: starting_layer,
	});

	let path_node_id = NodeId::new();
	let path_node = document_node_definitions::resolve_document_node_type("Path")
		.expect("Failed to create path node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: path_node_id,
		node_template: path_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: path_node_id,
		parent: starting_layer,
	});

	let stroke_node_id = NodeId::new();
	let stroke_node = document_node_definitions::resolve_document_node_type("Stroke")
		.expect("Failed to create stroke node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: stroke_node_id,
		node_template: stroke_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: stroke_node_id,
		parent: starting_layer,
	});

	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: starting_layer_nodes[1..(if is_layer_line(document, starting_layer) { 2 } else { 3 })].to_vec(),
		delete_children: false,
	});
	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: other_layer_nodes[..(if is_layer_line(document, other_layer) { 2 } else { 3 })].to_vec(),
		delete_children: false,
	});
	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: current_layer_nodes[..2].to_vec(),
		delete_children: false,
	});

	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(Message::StartBuffer);
}

fn merge_path_spline_layer(document: &DocumentMessageHandler, current_layer: LayerNodeIdentifier, other_layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	// Calculate the downstream transforms in order to bring the other vector data into the same layer space
	let current_transform = document.metadata().downstream_transform_to_document(current_layer);
	let other_transform = document.metadata().downstream_transform_to_document(other_layer);

	// Represents the change in position that would occur if the other layer was moved below the current layer
	let transform_delta = current_transform * other_transform.inverse();
	let offset = transform_delta.inverse();
	responses.add(GraphOperationMessage::TransformChange {
		layer: other_layer,
		transform: offset,
		transform_in: TransformIn::Local,
		skip_rerender: false,
	});

	// First find their IDs
	let current_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(
			vec![current_layer.to_node()],
			&[],
			crate::messages::portfolio::document::utility_types::network_interface::FlowType::HorizontalFlow,
		)
		.collect::<Vec<_>>();

	let other_layer_nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(vec![other_layer.to_node()], &[], FlowType::HorizontalFlow)
		.collect::<Vec<_>>();

	// Add merge node and insert between path and spline
	let merge_node_id = NodeId::new();
	let merge_node = document_node_definitions::resolve_document_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id,
		node_template: merge_node,
	});
	responses.add(NodeGraphMessage::SetToNodeOrLayer {
		node_id: merge_node_id,
		is_layer: false,
	});

	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: merge_node_id,
		parent: other_layer,
	});

	responses.add(NodeGraphMessage::ConnectUpstreamOutputToInput {
		downstream_input: InputConnector::node(current_layer.to_node(), 1),
		input_connector: InputConnector::node(merge_node_id, 1),
	});

	// Add flatten vector elements node after merge
	let flatten_node_id = NodeId::new();
	let flatten_node = document_node_definitions::resolve_document_node_type("Flatten Vector Elements")
		.expect("Failed to create flatten node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: flatten_node_id,
		node_template: flatten_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: flatten_node_id,
		parent: other_layer,
	});

	let path_node_id = NodeId::new();
	let path_node = document_node_definitions::resolve_document_node_type("Path")
		.expect("Failed to create path node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: path_node_id,
		node_template: path_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: path_node_id,
		parent: other_layer,
	});

	let stroke_node_id = NodeId::new();
	let stroke_node = document_node_definitions::resolve_document_node_type("Stroke")
		.expect("Failed to create stroke node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: stroke_node_id,
		node_template: stroke_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: stroke_node_id,
		parent: other_layer,
	});

	let node_range = if is_layer_line(document, other_layer) {
		1..2 // transform node is not deleted
	} else {
		1..3
	};

	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: other_layer_nodes[node_range].to_vec(),
		delete_children: false,
	});

	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: current_layer_nodes[..2].to_vec(),
		delete_children: false,
	});

	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(Message::StartBuffer);
}

fn is_layer_spline(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> bool {
	let nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::HorizontalFlow)
		.collect::<Vec<NodeId>>();

	// Check node types in the chain
	let mut has_spline = false;

	for node in nodes {
		if let Some(reference) = document.network_interface.reference(&node, &[]) {
			match reference.as_deref() {
				Some("Splines from Points") => has_spline = true,
				_ => continue,
			}
		}
	}

	has_spline
}

fn is_layer_line(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> bool {
	let nodes = document
		.network_interface
		.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::HorizontalFlow)
		.collect::<Vec<NodeId>>();

	// Check node types in the chain
	let mut has_line = false;

	for node in nodes {
		if let Some(reference) = document.network_interface.reference(&node, &[]) {
			match reference.as_deref() {
				Some("Line") => has_line = true,
				_ => continue,
			}
		}
	}

	has_line
}
