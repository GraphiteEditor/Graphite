use super::tool_prelude::*;
use crate::consts::{COLOR_OVERLAY_YELLOW, DRAG_THRESHOLD, INSERT_POINT_ON_SEGMENT_TOO_FAR_DISTANCE, SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::shape_editor::{ClosestSegment, ManipulatorAngle, OpposingHandleLengths, SelectedPointsInfo, ShapeState};
use crate::messages::tool::common_functionality::snapping::{SnapCache, SnapCandidatePoint, SnapData, SnapManager};

use graphene_core::renderer::Quad;
use graphene_core::vector::ManipulatorPointId;
use graphene_std::vector::NoHashBuilder;

use std::vec;

#[derive(Default)]
pub struct PathTool {
	fsm_state: PathToolFsmState,
	tool_data: PathToolData,
}

#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PathToolMessage {
	// Standard messages
	Abort,
	Overlays(OverlayContext),
	SelectionChanged,

	// Tool-specific messages
	BreakPath,
	DeselectAllPoints,
	Delete,
	DeleteAndBreakPath,
	DragStop {
		equidistant: Key,
	},
	Enter {
		add_to_selection: Key,
	},
	Escape,
	FlipSmoothSharp,
	GRS {
		// Should be `Key::KeyG` (Grab), `Key::KeyR` (Rotate), or `Key::KeyS` (Scale)
		key: Key,
	},
	ManipulatorMakeHandlesFree,
	ManipulatorMakeHandlesColinear,
	MouseDown {
		ctrl: Key,
		shift: Key,
	},
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		alt: Key,
		shift: Key,
	},
	PointerOutsideViewport {
		alt: Key,
		shift: Key,
	},
	RightClick,
	SelectAllAnchors,
	SelectedPointUpdated,
	SelectedPointXChanged {
		new_x: f64,
	},
	SelectedPointYChanged {
		new_y: f64,
	},
	SelectAnchorAndHandle,
	ResumeOriginalSelection,
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
		let manipulator_angle = selection_status.angle();

		let x_location = NumberInput::new(x)
			.unit(" px")
			.label("X")
			.min_width(120)
			.disabled(x.is_none())
			.min(-((1_u64 << f64::MANTISSA_DIGITS) as f64))
			.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				if let Some(new_x) = number_input.value.or(x) {
					PathToolMessage::SelectedPointXChanged { new_x }.into()
				} else {
					Message::NoOp
				}
			})
			.widget_holder();

		let y_location = NumberInput::new(y)
			.unit(" px")
			.label("Y")
			.min_width(120)
			.disabled(y.is_none())
			.min(-((1_u64 << f64::MANTISSA_DIGITS) as f64))
			.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				if let Some(new_y) = number_input.value.or(y) {
					PathToolMessage::SelectedPointYChanged { new_y }.into()
				} else {
					Message::NoOp
				}
			})
			.widget_holder();

		let related_seperator = Separator::new(SeparatorType::Related).widget_holder();
		let unrelated_seperator = Separator::new(SeparatorType::Unrelated).widget_holder();

		let colinear_handles_tooltip = "Keep both handles unbent, each 180° apart, when moving either";
		let colinear_handles_state = manipulator_angle.and_then(|angle| match angle {
			ManipulatorAngle::Colinear => Some(true),
			ManipulatorAngle::Free => Some(false),
			ManipulatorAngle::Mixed => None,
		})
		// TODO: Remove `unwrap_or_default` once checkboxes are capable of displaying a mixed state
		.unwrap_or_default();
		let colinear_handle_checkbox = CheckboxInput::new(colinear_handles_state)
			.disabled(self.tool_data.selection_status.is_none())
			.on_update(|&CheckboxInput { checked, .. }| {
				if checked {
					PathToolMessage::ManipulatorMakeHandlesColinear.into()
				} else {
					PathToolMessage::ManipulatorMakeHandlesFree.into()
				}
			})
			.tooltip(colinear_handles_tooltip)
			.widget_holder();
		let colinear_handles_label = TextLabel::new("Colinear Handles").tooltip(colinear_handles_tooltip).widget_holder();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				x_location,
				related_seperator.clone(),
				y_location,
				unrelated_seperator,
				colinear_handle_checkbox,
				related_seperator,
				colinear_handles_label,
			],
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
		match self.fsm_state {
			PathToolFsmState::Ready => actions!(PathToolMessageDiscriminant;
				FlipSmoothSharp,
				MouseDown,
				Delete,
				NudgeSelectedPoints,
				Enter,
				SelectAllAnchors,
				DeselectAllPoints,
				BreakPath,
				DeleteAndBreakPath,
				ResumeOriginalSelection,
			),
			PathToolFsmState::Dragging => actions!(PathToolMessageDiscriminant;
				Escape,
				RightClick,
				FlipSmoothSharp,
				DragStop,
				PointerMove,
				Delete,
				BreakPath,
				DeleteAndBreakPath,
				SelectAnchorAndHandle,
				ResumeOriginalSelection,
			),
			PathToolFsmState::DrawingBox => actions!(PathToolMessageDiscriminant;
				FlipSmoothSharp,
				DragStop,
				PointerMove,
				Delete,
				Enter,
				BreakPath,
				DeleteAndBreakPath,
				Escape,
				RightClick,
				ResumeOriginalSelection,
			),
			PathToolFsmState::InsertPoint => actions!(PathToolMessageDiscriminant;
				Enter,
				MouseDown,
				PointerMove,
				Escape,
				Delete,
				RightClick,
				GRS,
				ResumeOriginalSelection,
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
	InsertPoint,
}

enum InsertEndKind {
	Abort,
	Add { shift: bool },
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
	segment: Option<ClosestSegment>,
	snap_cache: SnapCache,
	double_click_handled: bool,
	auto_panning: AutoPanning,
	selected_points_before_space: Vec<ManipulatorPointId>,
	space_held: bool,
}

impl PathToolData {
	fn add_selected_points(&mut self, points: Vec<ManipulatorPointId>) -> PathToolFsmState {
		self.selected_points_before_space = points;
		PathToolFsmState::Dragging
	}

	fn remove_selected_points(&mut self) {
		self.selected_points_before_space.clear();
	}

	fn start_insertion(&mut self, responses: &mut VecDeque<Message>, segment: ClosestSegment) -> PathToolFsmState {
		if self.segment.is_some() {
			warn!("Segment was `Some(..)` before `start_insertion`")
		}
		self.segment = Some(segment);
		responses.add(OverlaysMessage::Draw);
		PathToolFsmState::InsertPoint
	}

	fn update_insertion(&mut self, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, input: &InputPreprocessorMessageHandler) -> PathToolFsmState {
		if let Some(closed_segment) = &mut self.segment {
			closed_segment.update_closest_point(document.metadata(), input.mouse.position);
			if closed_segment.too_far(input.mouse.position, INSERT_POINT_ON_SEGMENT_TOO_FAR_DISTANCE, document.metadata()) {
				self.end_insertion(shape_editor, responses, InsertEndKind::Abort)
			} else {
				PathToolFsmState::InsertPoint
			}
		} else {
			warn!("Segment was `None` on `update_insertion`");
			PathToolFsmState::Ready
		}
	}

	fn end_insertion(&mut self, shape_editor: &mut ShapeState, responses: &mut VecDeque<Message>, kind: InsertEndKind) -> PathToolFsmState {
		let mut commit_transaction = false;
		match self.segment.as_mut() {
			None => {
				warn!("Segment was `None` before `end_insertion`")
			}
			Some(closed_segment) => {
				if let InsertEndKind::Add { shift } = kind {
					closed_segment.adjusted_insert_and_select(shape_editor, responses, shift);
					commit_transaction = true;
				}
			}
		}

		self.segment = None;
		if commit_transaction {
			responses.add(DocumentMessage::EndTransaction);
		} else {
			responses.add(DocumentMessage::AbortTransaction);
		}
		responses.add(OverlaysMessage::Draw);
		PathToolFsmState::Ready
	}

	fn mouse_down(
		&mut self,
		shape_editor: &mut ShapeState,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		add_to_selection: bool,
		direct_insert_without_sliding: bool,
	) -> PathToolFsmState {
		self.double_click_handled = false;
		self.opposing_handle_lengths = None;

		self.drag_start_pos = input.mouse.position;

		// Select the first point within the threshold (in pixels)
		if let Some(selected_points) = shape_editor.change_point_selection(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD, add_to_selection) {
			responses.add(DocumentMessage::StartTransaction);

			if let Some(selected_points) = selected_points {
				self.drag_start_pos = input.mouse.position;
				self.start_dragging_point(selected_points, input, document, shape_editor);
				responses.add(OverlaysMessage::Draw);
			}
			PathToolFsmState::Dragging
		}
		// We didn't find a point nearby, so now we'll try to add a point into the closest path segment
		else if let Some(closed_segment) = shape_editor.upper_closest_segment(&document.network_interface, input.mouse.position, SELECTION_TOLERANCE) {
			responses.add(DocumentMessage::StartTransaction);
			if direct_insert_without_sliding {
				self.start_insertion(responses, closed_segment);
				self.end_insertion(shape_editor, responses, InsertEndKind::Add { shift: add_to_selection })
			} else {
				self.start_insertion(responses, closed_segment)
			}
		}
		// We didn't find a segment path, so consider selecting the nearest shape instead
		else if let Some(layer) = document.click(input) {
			if add_to_selection {
				responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![layer.to_node()] });
			} else {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
			}
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);
			shape_editor.select_connected_anchors(document, layer, input.mouse.position);

			responses.add(DocumentMessage::StartTransaction);
			PathToolFsmState::Dragging
		}
		// Start drawing a box
		else {
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);

			PathToolFsmState::DrawingBox
		}
	}

	fn start_dragging_point(&mut self, selected_points: SelectedPointsInfo, input: &InputPreprocessorMessageHandler, document: &DocumentMessageHandler, shape_editor: &mut ShapeState) {
		let mut manipulators = HashMap::with_hasher(NoHashBuilder);
		let mut unselected = Vec::new();
		for (&layer, state) in &shape_editor.selected_shape_state {
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				continue;
			};
			let transform = document.metadata().transform_to_document(layer);

			let mut layer_manipulators = HashSet::with_hasher(NoHashBuilder);
			for point in state.selected() {
				let Some(anchor) = point.get_anchor(&vector_data) else { continue };
				layer_manipulators.insert(anchor);
			}
			for (&id, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
				if layer_manipulators.contains(&id) {
					continue;
				}
				unselected.push(SnapCandidatePoint::handle(transform.transform_point2(position)))
			}
			if !layer_manipulators.is_empty() {
				manipulators.insert(layer, layer_manipulators);
			}
		}
		self.snap_cache = SnapCache { manipulators, unselected };

		let viewport_to_document = document.metadata().document_to_viewport.inverse();
		self.previous_mouse_position = viewport_to_document.transform_point2(input.mouse.position - selected_points.offset);
	}

	fn update_colinear(&mut self, shift: bool, alt: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		// Check if the alt key has just been pressed
		if alt && !self.alt_debounce {
			self.opposing_handle_lengths = None;
			let colinear = self.selection_status.angle().map_or(false, |angle| match angle {
				ManipulatorAngle::Colinear => true,
				ManipulatorAngle::Free => false,
				ManipulatorAngle::Mixed => false,
			});
			if colinear {
				shape_editor.disable_colinear_handles_state_on_selected(&document.network_interface, responses);
			} else {
				shape_editor.convert_selected_manipulators_to_colinear_handles(responses, document);
			}
			self.alt_debounce = true;
			return true;
		}
		self.alt_debounce = alt;

		if shift && self.opposing_handle_lengths.is_none() {
			self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(document));
		}
		false
	}

	fn drag(&mut self, equidistant: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Move the selected points with the mouse
		let previous_mouse = document.metadata().document_to_viewport.transform_point2(self.previous_mouse_position);
		let snapped_delta = shape_editor.snap(&mut self.snap_manager, &self.snap_cache, document, input, previous_mouse);
		let handle_lengths = if equidistant { None } else { self.opposing_handle_lengths.take() };
		shape_editor.move_selected_points(handle_lengths, document, snapped_delta, equidistant, responses);
		self.previous_mouse_position += document.metadata().document_to_viewport.inverse().transform_vector2(snapped_delta);
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
				let target_layers = document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);

				responses.add(OverlaysMessage::Draw);

				responses.add(PathToolMessage::SelectedPointUpdated);
				self
			}
			(_, PathToolMessage::Overlays(mut overlay_context)) => {
				path_overlays(document, shape_editor, &mut overlay_context);

				match self {
					Self::DrawingBox => {
						let fill_color = graphene_std::Color::from_rgb_str(crate::consts::COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap())
							.unwrap()
							.with_alpha(0.05)
							.rgba_hex();

						overlay_context.quad(Quad::from_box([tool_data.drag_start_pos, tool_data.previous_mouse_position]), Some(&("#".to_string() + &fill_color)));
					}
					Self::Dragging => {
						tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
					}
					Self::InsertPoint => {
						let state = tool_data.update_insertion(shape_editor, document, responses, input);

						if let Some(closest_segment) = &tool_data.segment {
							overlay_context.manipulator_anchor(closest_segment.closest_point_to_viewport(), false, Some(COLOR_OVERLAY_YELLOW));
						}

						responses.add(PathToolMessage::SelectedPointUpdated);
						return state;
					}
					_ => {}
				}

				responses.add(PathToolMessage::SelectedPointUpdated);
				self
			}

			// `Self::InsertPoint` case:
			(Self::InsertPoint, PathToolMessage::MouseDown { .. } | PathToolMessage::Enter { .. }) => {
				tool_data.double_click_handled = true;
				// TODO: Don't use `Key::Shift` directly, instead take it as a variable from the input mappings list like in all other places
				let shift = input.keyboard.get(Key::Shift as usize);
				tool_data.end_insertion(shape_editor, responses, InsertEndKind::Add { shift })
			}
			(Self::InsertPoint, PathToolMessage::PointerMove { .. }) => {
				responses.add(OverlaysMessage::Draw);
				// `tool_data.update_insertion` would be called on `OverlaysMessage::Draw`
				// we anyway should to call it on `::Draw` because we can change scale by ctrl+scroll without `::PointerMove`
				self
			}
			(Self::InsertPoint, PathToolMessage::Escape | PathToolMessage::Delete | PathToolMessage::RightClick) => tool_data.end_insertion(shape_editor, responses, InsertEndKind::Abort),
			(Self::InsertPoint, PathToolMessage::GRS { key: propagate }) => {
				// MAYBE: use `InputMapperMessage::KeyDown(..)` instead
				match propagate {
					// TODO: Don't use `Key::G` directly, instead take it as a variable from the input mappings list like in all other places
					Key::KeyG => responses.add(TransformLayerMessage::BeginGrab),
					// TODO: Don't use `Key::R` directly, instead take it as a variable from the input mappings list like in all other places
					Key::KeyR => responses.add(TransformLayerMessage::BeginRotate),
					// TODO: Don't use `Key::S` directly, instead take it as a variable from the input mappings list like in all other places
					Key::KeyS => responses.add(TransformLayerMessage::BeginScale),
					_ => warn!("Unexpected GRS key"),
				}
				tool_data.end_insertion(shape_editor, responses, InsertEndKind::Abort)
			}
			// Mouse down
			(_, PathToolMessage::MouseDown { ctrl, shift }) => {
				let add_to_selection = input.keyboard.get(shift as usize);
				let direct_insert_without_sliding = input.keyboard.get(ctrl as usize);
				tool_data.mouse_down(shape_editor, document, input, responses, add_to_selection, direct_insert_without_sliding)
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::PointerMove { alt, shift }) => {
				tool_data.previous_mouse_position = input.mouse.position;
				responses.add(OverlaysMessage::Draw);

				// Auto-panning
				let messages = [PathToolMessage::PointerOutsideViewport { alt, shift }.into(), PathToolMessage::PointerMove { alt, shift }.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				PathToolFsmState::DrawingBox
			}
			(PathToolFsmState::Dragging, PathToolMessage::PointerMove { alt, shift }) => {
				let alt_state = input.keyboard.get(alt as usize);
				let shift_state = input.keyboard.get(shift as usize);
				if !tool_data.update_colinear(shift_state, alt_state, shape_editor, document, responses) {
					tool_data.drag(shift_state, shape_editor, document, input, responses);
				}

				// Auto-panning
				let messages = [PathToolMessage::PointerOutsideViewport { alt, shift }.into(), PathToolMessage::PointerMove { alt, shift }.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				PathToolFsmState::Dragging
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_start_pos += shift;
				}

				PathToolFsmState::DrawingBox
			}
			(PathToolFsmState::Dragging, PathToolMessage::PointerOutsideViewport { shift, .. }) => {
				// Auto-panning
				if tool_data.auto_panning.shift_viewport(input, responses).is_some() {
					let shift_state = input.keyboard.get(shift as usize);
					tool_data.drag(shift_state, shape_editor, document, input, responses);
				}

				PathToolFsmState::Dragging
			}
			(state, PathToolMessage::PointerOutsideViewport { alt, shift }) => {
				// Auto-panning
				let messages = [PathToolMessage::PointerOutsideViewport { alt, shift }.into(), PathToolMessage::PointerMove { alt, shift }.into()];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::Enter { add_to_selection }) => {
				let shift_pressed = input.keyboard.get(add_to_selection as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.network_interface, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
				}
				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::Ready
			}
			(PathToolFsmState::Dragging, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				responses.add(DocumentMessage::AbortTransaction);
				shape_editor.deselect_all_points();
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}
			// Mouse up
			(PathToolFsmState::DrawingBox, PathToolMessage::DragStop { equidistant }) => {
				let equidistant = input.keyboard.get(equidistant as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.network_interface, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !equidistant);
				}
				responses.add(OverlaysMessage::Draw);
				responses.add(PathToolMessage::SelectedPointUpdated);

				PathToolFsmState::Ready
			}
			(PathToolFsmState::Dragging, PathToolMessage::SelectAnchorAndHandle) => {
				if tool_data.space_held {
					return PathToolFsmState::Dragging;
				}
				tool_data.space_held = true;
				tool_data.add_selected_points(tool_action_data.shape_editor.selected_points().cloned().collect());
				tool_action_data.shape_editor.select_handles_and_anchor(&tool_action_data.document.network_interface);
				responses.add(PathToolMessage::SelectedPointUpdated);
				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Dragging
			}
			(PathToolFsmState::Dragging, PathToolMessage::ResumeOriginalSelection) => {
				tool_data.space_held = false;
				tool_action_data.shape_editor.deselect_all_points();
				tool_action_data.shape_editor.select_points_by_manipulator_id(&tool_data.selected_points_before_space);
				responses.add(PathToolMessage::SelectedPointUpdated);
				PathToolFsmState::Dragging
			}
			(PathToolFsmState::Ready, PathToolMessage::ResumeOriginalSelection) => {
				tool_data.space_held = false;
				tool_data.remove_selected_points();
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DragStop { equidistant }) => {
				let equidistant = input.keyboard.get(equidistant as usize);

				let nearest_point = shape_editor.find_nearest_point_indices(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD);

				if let Some((layer, nearest_point)) = nearest_point {
					if tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD && !equidistant {
						let clicked_selected = shape_editor.selected_points().any(|&point| nearest_point == point);
						if clicked_selected {
							shape_editor.deselect_all_points();
							shape_editor.selected_shape_state.entry(layer).or_default().select_point(nearest_point);
							responses.add(OverlaysMessage::Draw);
						}
					}
				}

				responses.add(DocumentMessage::EndTransaction);
				responses.add(PathToolMessage::SelectedPointUpdated);
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}

			// Delete key
			(_, PathToolMessage::Delete) => {
				// Delete the selected points and clean up overlays
				responses.add(DocumentMessage::AddTransaction);
				shape_editor.delete_selected_points(document, responses);
				responses.add(PathToolMessage::SelectionChanged);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::BreakPath) => {
				shape_editor.break_path_at_selected_point(document, responses);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DeleteAndBreakPath) => {
				shape_editor.delete_point_and_break_path(document, responses);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::FlipSmoothSharp) => {
				if !tool_data.double_click_handled {
					shape_editor.flip_smooth_sharp(&document.network_interface, input.mouse.position, SELECTION_TOLERANCE, responses);
					responses.add(PathToolMessage::SelectedPointUpdated);
				}
				self
			}
			(_, PathToolMessage::Abort) => {
				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::PointerMove { .. }) => self,
			(_, PathToolMessage::NudgeSelectedPoints { delta_x, delta_y }) => {
				shape_editor.move_selected_points(tool_data.opposing_handle_lengths.take(), document, (delta_x, delta_y).into(), true, responses);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectAllAnchors) => {
				shape_editor.select_all_anchors_in_selected_layers(document);
				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DeselectAllPoints) => {
				shape_editor.deselect_all_points();
				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointXChanged { new_x }) => {
				if let Some(&SingleSelectedPoint { coordinates, id, layer, .. }) = tool_data.selection_status.as_one() {
					shape_editor.reposition_control_point(&id, &document.network_interface, DVec2::new(new_x, coordinates.y), layer, responses);
				}
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointYChanged { new_y }) => {
				if let Some(&SingleSelectedPoint { coordinates, id, layer, .. }) = tool_data.selection_status.as_one() {
					shape_editor.reposition_control_point(&id, &document.network_interface, DVec2::new(coordinates.x, new_y), layer, responses);
				}
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointUpdated) => {
				tool_data.selection_status = get_selection_status(&document.network_interface, shape_editor);
				self
			}
			(_, PathToolMessage::ManipulatorMakeHandlesColinear) => {
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.convert_selected_manipulators_to_colinear_handles(responses, document);
				responses.add(DocumentMessage::EndTransaction);
				responses.add(PathToolMessage::SelectionChanged);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::ManipulatorMakeHandlesFree) => {
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.disable_colinear_handles_state_on_selected(&document.network_interface, responses);
				responses.add(DocumentMessage::EndTransaction);
				PathToolFsmState::Ready
			}
			(_, _) => PathToolFsmState::Ready,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PathToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Point"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Insert Point on Segment")]),
				// TODO: Only show if at least one anchor is selected, and dynamically show either "Smooth" or "Sharp" based on the current state
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Make Anchor Smooth/Sharp")]),
				// TODO: Only show the following hints if at least one point is selected
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
				HintGroup(vec![HintInfo::keys([Key::KeyG, Key::KeyR, Key::KeyS], "Grab/Rotate/Scale Selected")]),
				HintGroup(vec![HintInfo::arrow_keys("Nudge Selected"), HintInfo::keys([Key::Shift], "10x").prepend_plus()]),
				HintGroup(vec![
					HintInfo::keys([Key::Delete], "Delete Selected"),
					// TODO: Only show the following hints if at least one anchor is selected
					HintInfo::keys([Key::Accel], "No Dissolve").prepend_plus(),
					HintInfo::keys([Key::Shift], "Break Anchor").prepend_plus(),
				]),
			]),
			PathToolFsmState::Dragging => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![
					// TODO: Switch this to the "S" key. Also, make the hint dynamically say "Make Colinear" or "Make Not Colinear" based on its current state. And only
					// TODO: show this hint if a handle (not an anchor) is being dragged, and disable that shortcut so it can't be pressed even with the hint not shown.
					HintInfo::keys([Key::Alt], "Toggle Colinear Handles"),
					// TODO: Switch this to the "Alt" key (since it's equivalent to the "From Center" modifier when drawing a line). And show this only when a handle is being dragged.
					HintInfo::keys([Key::Shift], "Equidistant Handles"),
					HintInfo::keys([Key::Space], "Drag anchor"),
					// TODO: Add "Snap 15°" modifier with the "Shift" key (only when a handle is being dragged).
					// TODO: Add "Lock Angle" modifier with the "Ctrl" key (only when a handle is being dragged).
				]),
			]),
			PathToolFsmState::DrawingBox => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
					HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus(),
				]),
			]),
			PathToolFsmState::InsertPoint => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Insert Point")]),
			]),
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

	fn angle(&self) -> Option<ManipulatorAngle> {
		match self {
			Self::None => None,
			Self::One(one) => Some(one.manipulator_angle),
			Self::Multiple(one) => Some(one.manipulator_angle),
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
fn get_selection_status(network_interface: &NodeNetworkInterface, shape_state: &mut ShapeState) -> SelectionStatus {
	let mut selection_layers = shape_state.selected_shape_state.iter().map(|(k, v)| (*k, v.selected_points_count()));
	let total_selected_points = selection_layers.clone().map(|(_, v)| v).sum::<usize>();

	// Check to see if only one manipulator group in a single shape is selected
	if total_selected_points == 1 {
		let Some(layer) = selection_layers.find(|(_, v)| *v > 0).map(|(k, _)| k) else {
			return SelectionStatus::None;
		};
		let Some(vector_data) = network_interface.compute_modified_vector(layer) else {
			return SelectionStatus::None;
		};
		let Some(&point) = shape_state.selected_points().next() else {
			return SelectionStatus::None;
		};
		let Some(local_position) = point.get_position(&vector_data) else {
			return SelectionStatus::None;
		};

		let coordinates = network_interface.document_metadata().transform_to_document(layer).transform_point2(local_position);
		let manipulator_angle = if vector_data.colinear(point) { ManipulatorAngle::Colinear } else { ManipulatorAngle::Free };

		return SelectionStatus::One(SingleSelectedPoint {
			coordinates,
			layer,
			id: point,
			manipulator_angle,
		});
	};

	// Check to see if multiple manipulator groups are selected
	if total_selected_points > 1 {
		return SelectionStatus::Multiple(MultipleSelectedPoints {
			manipulator_angle: shape_state.selected_manipulator_angles(network_interface),
		});
	}

	SelectionStatus::None
}
