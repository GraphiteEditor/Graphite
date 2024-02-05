use super::tool_prelude::*;
use crate::consts::{DRAG_THRESHOLD, SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::tool::common_functionality::graph_modification_utils::{get_manipulator_from_id, get_mirror_handles, get_subpaths};
use crate::messages::tool::common_functionality::shape_editor::{ManipulatorAngle, ManipulatorPointInfo, OpposingHandleLengths, SelectedPointsInfo, ShapeState};
use crate::messages::tool::common_functionality::snapping::{SnapData, SnapManager};

use graph_craft::document::NodeNetwork;
use graphene_core::renderer::Quad;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use std::vec;

#[derive(Default)]
pub struct PathTool {
	fsm_state: PathToolFsmState,
	tool_data: PathToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum PathToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	Overlays(OverlayContext),
	#[remain::unsorted]
	SelectionChanged,

	// Tool-specific messages
	BreakPath,
	Delete,
	DragStart {
		add_to_selection: Key,
	},
	DragStop {
		shift_mirror_distance: Key,
	},
	Enter {
		add_to_selection: Key,
	},
	InsertPoint,
	ManipulatorAngleMakeSharp,
	ManipulatorAngleMakeSmooth,
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		alt: Key,
		shift: Key,
	},
	SelectAllPoints,
	SelectedPointUpdated,
	SelectedPointXChanged {
		new_x: f64,
	},
	SelectedPointYChanged {
		new_y: f64,
	},
}

impl ToolMetadata for PathTool {
	fn icon_name(&self) -> String {
		"VectorPathTool".into()
	}
	fn tooltip(&self) -> String {
		"Path Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Path
	}
}

impl LayoutHolder for PathTool {
	fn layout(&self) -> Layout {
		let coordinates = self.tool_data.selection_status.as_one().as_ref().map(|point| point.coordinates);
		let (x, y) = coordinates.map(|point| (Some(point.x), Some(point.y))).unwrap_or((None, None));

		let selection_status = &self.tool_data.selection_status;
		let manipulator_angle = selection_status
			.as_multiple()
			.map(|multiple| multiple.manipulator_angle)
			.or_else(|| selection_status.as_one().map(|point| point.manipulator_angle));

		let x_location = NumberInput::new(x)
			.unit(" px")
			.label("X")
			.min_width(120)
			.disabled(x.is_none())
			.min(-((1_u64 << std::f64::MANTISSA_DIGITS) as f64))
			.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				let new_x = number_input.value.unwrap_or(x.unwrap());
				PathToolMessage::SelectedPointXChanged { new_x }.into()
			})
			.widget_holder();

		let y_location = NumberInput::new(y)
			.unit(" px")
			.label("Y")
			.min_width(120)
			.disabled(y.is_none())
			.min(-((1_u64 << std::f64::MANTISSA_DIGITS) as f64))
			.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				let new_y = number_input.value.unwrap_or(y.unwrap());
				PathToolMessage::SelectedPointYChanged { new_y }.into()
			})
			.widget_holder();

		let related_seperator = Separator::new(SeparatorType::Related).widget_holder();
		let unrelated_seperator = Separator::new(SeparatorType::Unrelated).widget_holder();

		let manipulator_angle_options = vec![
			RadioEntryData::new("Smooth").on_update(|_| PathToolMessage::ManipulatorAngleMakeSmooth.into()),
			RadioEntryData::new("Sharp").on_update(|_| PathToolMessage::ManipulatorAngleMakeSharp.into()),
		];
		let manipulator_angle_index = manipulator_angle.and_then(|angle| match angle {
			ManipulatorAngle::Smooth => Some(0),
			ManipulatorAngle::Sharp => Some(1),
			ManipulatorAngle::Mixed => None,
		});

		let manipulator_angle_radio = RadioInput::new(manipulator_angle_options)
			.disabled(self.tool_data.selection_status.is_none())
			.selected_index(manipulator_angle_index)
			.widget_holder();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![x_location, related_seperator, y_location, unrelated_seperator, manipulator_angle_radio],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PathTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let updating_point = message == ToolMessage::Path(PathToolMessage::SelectedPointUpdated);

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);

		if updating_point {
			self.send_layout(responses, LayoutTarget::ToolOptions);
		}
	}

	// Different actions depending on state may be wanted:
	fn actions(&self) -> ActionList {
		use PathToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStart,
				Delete,
				NudgeSelectedPoints,
				Enter,
				SelectAllPoints,
				BreakPath,
			),
			Dragging => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
				SelectAllPoints,
				BreakPath,
			),
			DrawingBox => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
				Enter,
				SelectAllPoints,
				BreakPath,
			),
		}
	}
}

impl ToolTransition for PathTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(PathToolMessage::Abort.into()),
			selection_changed: Some(PathToolMessage::SelectionChanged.into()),
			overlay_provider: Some(|overlay_context| PathToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PathToolFsmState {
	#[default]
	Ready,
	Dragging,
	DrawingBox,
}

#[derive(Default)]
struct PathToolData {
	snap_manager: SnapManager,
	drag_start_pos: DVec2,
	previous_mouse_position: DVec2,
	alt_debounce: bool,
	opposing_handle_lengths: Option<OpposingHandleLengths>,
	/// Describes information about the selected point(s), if any, across one or multiple shapes and manipulator point types (anchor or handle).
	/// The available information varies depending on whether `None`, `One`, or `Multiple` points are currently selected.
	selection_status: SelectionStatus,
}

impl PathToolData {
	fn mouse_down(
		&mut self,
		shift: bool,
		shape_editor: &mut ShapeState,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> PathToolFsmState {
		self.opposing_handle_lengths = None;
		let _selected_layers = shape_editor.selected_layers().cloned().collect::<Vec<_>>();

		// Select the first point within the threshold (in pixels)
		if let Some(selected_points) = shape_editor.select_point(&document.network, &document.metadata, input.mouse.position, SELECTION_THRESHOLD, shift) {
			self.start_dragging_point(selected_points, input, document, responses);
			responses.add(OverlaysMessage::Draw);

			PathToolFsmState::Dragging
		}
		// We didn't find a point nearby, so consider selecting the nearest shape instead
		else if let Some(layer) = document.click(input.mouse.position, &document.network) {
			if shift {
				responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![layer.to_node()] });
			} else {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
			}
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = input.mouse.position;
			shape_editor.select_all_anchors(&document.network, layer);

			PathToolFsmState::Dragging
		} else {
			// Start drawing a box
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = input.mouse.position;

			PathToolFsmState::DrawingBox
		}
	}

	fn start_dragging_point(&mut self, mut selected_points: SelectedPointsInfo, input: &InputPreprocessorMessageHandler, _document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::StartTransaction);

		// TODO: enable snapping

		//self
		//	.snap_manager
		//	.start_snap(document, input, document.bounding_boxes(Some(&selected_layers), None, font_cache), true, true);

		// Do not snap against handles when anchor is selected
		let mut additional_selected_points = Vec::new();
		for point in selected_points.points.iter() {
			if point.point_id.manipulator_type == SelectedType::Anchor {
				additional_selected_points.push(ManipulatorPointInfo {
					layer: point.layer,
					point_id: ManipulatorPointId::new(point.point_id.group, SelectedType::InHandle),
				});
				additional_selected_points.push(ManipulatorPointInfo {
					layer: point.layer,
					point_id: ManipulatorPointId::new(point.point_id.group, SelectedType::OutHandle),
				});
			}
		}
		selected_points.points.extend(additional_selected_points);

		self.drag_start_pos = input.mouse.position;
		self.previous_mouse_position = input.mouse.position - selected_points.offset;
	}

	fn drag(&mut self, shift: bool, alt: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Check if the alt key has just been pressed
		if alt && !self.alt_debounce {
			self.opposing_handle_lengths = None;
			shape_editor.toggle_handle_mirroring_on_selected(responses);
		}
		self.alt_debounce = alt;

		if shift {
			if self.opposing_handle_lengths.is_none() {
				self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(&document.network));
			}
		} else if let Some(opposing_handle_lengths) = &self.opposing_handle_lengths {
			shape_editor.reset_opposing_handle_lengths(&document.network, opposing_handle_lengths, responses);
			self.opposing_handle_lengths = None;
		}

		// Move the selected points with the mouse
		let snapped_delta = shape_editor.snap(&mut self.snap_manager, document, input, self.previous_mouse_position);
		shape_editor.move_selected_points(&document.network, &document.metadata, snapped_delta, shift, responses);
		self.previous_mouse_position += snapped_delta;
	}
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData { document, input, shape_editor, .. } = tool_action_data;
		let ToolMessage::Path(event) = event else {
			return self;
		};

		match (self, event) {
			(_, PathToolMessage::SelectionChanged) => {
				// Set the newly targeted layers to visible
				let target_layers = document.selected_nodes.selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);

				responses.add(OverlaysMessage::Draw);

				responses.add(PathToolMessage::SelectedPointUpdated);
				// This can happen in any state (which is why we return self)
				self
			}
			(_, PathToolMessage::Overlays(mut overlay_context)) => {
				path_overlays(document, shape_editor, &mut overlay_context);

				if self == Self::DrawingBox {
					overlay_context.quad(Quad::from_box([tool_data.drag_start_pos, tool_data.previous_mouse_position]))
				} else if self == Self::Dragging {
					tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				}

				responses.add(PathToolMessage::SelectedPointUpdated);

				self
			}
			// Mouse down
			(_, PathToolMessage::DragStart { add_to_selection }) => {
				let shift = input.keyboard.get(add_to_selection as usize);

				tool_data.mouse_down(shift, shape_editor, document, input, responses)
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::PointerMove { .. }) => {
				tool_data.previous_mouse_position = input.mouse.position;
				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::DrawingBox
			}
			(PathToolFsmState::Dragging, PathToolMessage::PointerMove { alt, shift }) => {
				let alt = input.keyboard.get(alt as usize);
				let shift = input.keyboard.get(shift as usize);
				tool_data.drag(shift, alt, shape_editor, document, input, responses);

				PathToolFsmState::Dragging
			}

			(PathToolFsmState::DrawingBox, PathToolMessage::Enter { add_to_selection }) => {
				let shift_pressed = input.keyboard.get(add_to_selection as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.network, &document.metadata, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
				}
				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::Ready
			}

			// Mouse up
			(PathToolFsmState::DrawingBox, PathToolMessage::DragStop { shift_mirror_distance }) => {
				let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.network, &document.metadata, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
				}
				responses.add(OverlaysMessage::Draw);
				responses.add(PathToolMessage::SelectedPointUpdated);

				PathToolFsmState::Ready
			}

			(_, PathToolMessage::DragStop { shift_mirror_distance }) => {
				let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

				let nearest_point = shape_editor
					.find_nearest_point_indices(&document.network, &document.metadata, input.mouse.position, SELECTION_THRESHOLD)
					.map(|(_, nearest_point)| nearest_point);

				shape_editor.delete_selected_handles_with_zero_length(&document.network, &document.metadata, &tool_data.opposing_handle_lengths, responses);

				if tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD && !shift_pressed {
					let clicked_selected = shape_editor.selected_points().any(|&point| nearest_point == Some(point));
					if clicked_selected {
						shape_editor.deselect_all();
						shape_editor.select_point(&document.network, &document.metadata, input.mouse.position, SELECTION_THRESHOLD, false);
						responses.add(OverlaysMessage::Draw);
					}
				}

				responses.add(PathToolMessage::SelectedPointUpdated);
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}

			// Delete key
			(_, PathToolMessage::Delete) => {
				// Delete the selected points and clean up overlays
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.delete_selected_points(responses);
				responses.add(PathToolMessage::SelectionChanged);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::BreakPath) => {
				shape_editor.break_closed_curve(&document.network, responses);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::InsertPoint) => {
				// First we try and flip the sharpness (if they have clicked on an anchor)
				if !shape_editor.flip_sharp(&document.network, &document.metadata, input.mouse.position, SELECTION_TOLERANCE, responses) {
					// If not, then we try and split the path that may have been clicked upon
					shape_editor.split(&document.network, &document.metadata, input.mouse.position, SELECTION_TOLERANCE, responses);
				}

				responses.add(PathToolMessage::SelectedPointUpdated);
				self
			}
			(_, PathToolMessage::Abort) => {
				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::PointerMove { .. }) => self,
			(_, PathToolMessage::NudgeSelectedPoints { delta_x, delta_y }) => {
				shape_editor.move_selected_points(&document.network, &document.metadata, (delta_x, delta_y).into(), true, responses);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectAllPoints) => {
				shape_editor.select_all_points(&document.network);
				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointXChanged { new_x }) => {
				if let Some(&SingleSelectedPoint { coordinates, id, layer, .. }) = tool_data.selection_status.as_one() {
					shape_editor.reposition_control_point(&id, responses, &document.network, &document.metadata, DVec2::new(new_x, coordinates.y), layer);
				}
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointYChanged { new_y }) => {
				if let Some(&SingleSelectedPoint { coordinates, id, layer, .. }) = tool_data.selection_status.as_one() {
					shape_editor.reposition_control_point(&id, responses, &document.network, &document.metadata, DVec2::new(coordinates.x, new_y), layer);
				}
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointUpdated) => {
				tool_data.selection_status = get_selection_status(&document.network, &document.metadata, shape_editor);
				self
			}
			(_, PathToolMessage::ManipulatorAngleMakeSmooth) => {
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.set_handle_mirroring_on_selected(true, responses);
				shape_editor.smooth_selected_groups(responses, &document.network);
				responses.add(DocumentMessage::CommitTransaction);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::ManipulatorAngleMakeSharp) => {
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.set_handle_mirroring_on_selected(false, responses);
				responses.add(DocumentMessage::CommitTransaction);
				PathToolFsmState::Ready
			}
			(_, _) => PathToolFsmState::Ready,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let general_hint_data = HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Point"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()]),
			HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
			HintGroup(vec![HintInfo::arrow_keys("Nudge Selected"), HintInfo::keys([Key::Shift], "10x").prepend_plus()]),
			HintGroup(vec![HintInfo::keys([Key::KeyG, Key::KeyR, Key::KeyS], "Grab/Rotate/Scale Selected")]),
		]);

		let hint_data = match self {
			PathToolFsmState::Ready => general_hint_data,
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Alt], "Split/Align Handles (Toggle)"),
				HintInfo::keys([Key::Shift], "Share Lengths of Aligned Handles"),
			])]),
			PathToolFsmState::DrawingBox => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
				HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus(),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[derive(Debug, PartialEq, Default)]
enum SelectionStatus {
	#[default]
	None,
	One(SingleSelectedPoint),
	Multiple(MultipleSelectedPoints),
}

impl SelectionStatus {
	fn is_none(&self) -> bool {
		self == &SelectionStatus::None
	}

	fn as_one(&self) -> Option<&SingleSelectedPoint> {
		match self {
			SelectionStatus::One(one) => Some(one),
			_ => None,
		}
	}

	fn as_multiple(&self) -> Option<&MultipleSelectedPoints> {
		match self {
			SelectionStatus::Multiple(multiple) => Some(multiple),
			_ => None,
		}
	}
}

#[derive(Debug, PartialEq)]
struct MultipleSelectedPoints {
	manipulator_angle: ManipulatorAngle,
}

#[derive(Debug, PartialEq)]
struct SingleSelectedPoint {
	coordinates: DVec2,
	id: ManipulatorPointId,
	layer: LayerNodeIdentifier,
	manipulator_angle: ManipulatorAngle,
}

/// Sets the cumulative description of the selected points: if `None` are selected, if `One` is selected, or if `Multiple` are selected.
/// Applies to any selected points, whether they are anchors or handles; and whether they are from a single shape or across multiple shapes.
fn get_selection_status(document_network: &NodeNetwork, document_metadata: &DocumentMetadata, shape_state: &mut ShapeState) -> SelectionStatus {
	let mut selection_layers = shape_state.selected_shape_state.iter().map(|(k, v)| (*k, v.selected_points_count()));
	let total_selected_points = selection_layers.clone().map(|(_, v)| v).sum::<usize>();

	// Check to see if only one manipulator group in a single shape is selected
	if total_selected_points == 1 {
		let Some(layer) = selection_layers.find(|(_, v)| *v > 0).map(|(k, _)| k) else {
			return SelectionStatus::None;
		};

		let Some(subpaths) = get_subpaths(layer, document_network) else {
			return SelectionStatus::None;
		};
		let Some(mirror) = get_mirror_handles(layer, document_network) else {
			return SelectionStatus::None;
		};
		let Some(point) = shape_state.selected_points().next() else {
			return SelectionStatus::None;
		};

		let Some(group) = get_manipulator_from_id(subpaths, point.group) else {
			return SelectionStatus::None;
		};
		let Some(local_position) = point.manipulator_type.get_position(group) else {
			return SelectionStatus::None;
		};

		let manipulator_angle = if mirror.contains(&point.group) { ManipulatorAngle::Smooth } else { ManipulatorAngle::Sharp };

		return SelectionStatus::One(SingleSelectedPoint {
			coordinates: document_metadata.transform_to_document(layer).transform_point2(local_position),
			layer,
			id: *point,
			manipulator_angle,
		});
	};

	// Check to see if multiple manipulator groups are selected
	if total_selected_points > 1 {
		return SelectionStatus::Multiple(MultipleSelectedPoints {
			manipulator_angle: shape_state.selected_manipulator_angles(document_network),
		});
	}

	SelectionStatus::None
}
