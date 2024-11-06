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
		extend_selection: Key,
	},
	Enter {
		extend_selection: Key,
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
		direct_insert_without_sliding: Key,
		extend_selection: Key,
	},
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		equidistant: Key,
		toggle_colinear: Key,
		move_anchor_with_handles: Key,
	},
	PointerOutsideViewport {
		equidistant: Key,
		toggle_colinear: Key,
		move_anchor_with_handles: Key,
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
	SwapSelectedHandles,
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

		match message {
			ToolMessage::Path(PathToolMessage::SwapSelectedHandles) => {
				if tool_data.shape_editor.handle_with_pair_selected(&tool_data.document.network_interface) {
					tool_data.shape_editor.alternate_selected_handles(&tool_data.document.network_interface);
					responses.add(PathToolMessage::SelectedPointUpdated);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::None });
					responses.add(OverlaysMessage::Draw);
				}
			}
			_ => {
				self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
			}
		}

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
			),
			PathToolFsmState::Dragging(_) => actions!(PathToolMessageDiscriminant;
				Escape,
				RightClick,
				FlipSmoothSharp,
				DragStop,
				PointerMove,
				Delete,
				BreakPath,
				DeleteAndBreakPath,
				SwapSelectedHandles,
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
			),
			PathToolFsmState::InsertPoint => actions!(PathToolMessageDiscriminant;
				Enter,
				MouseDown,
				PointerMove,
				Escape,
				Delete,
				RightClick,
				GRS,
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
pub struct DraggingState {
	point_select_state: PointSelectState,
	colinear: ManipulatorAngle,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PointSelectState {
	HandleWithPair,
	#[default]
	HandleNoPair,
	Anchor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PathToolFsmState {
	#[default]
	Ready,
	Dragging(DraggingState),
	DrawingBox,
	InsertPoint,
}

enum InsertEndKind {
	Abort,
	Add { extend_selection: bool },
}

#[derive(Default)]
struct PathToolData {
	snap_manager: SnapManager,
	drag_start_pos: DVec2,
	previous_mouse_position: DVec2,
	toggle_colinear_debounce: bool,
	opposing_handle_lengths: Option<OpposingHandleLengths>,
	/// Describes information about the selected point(s), if any, across one or multiple shapes and manipulator point types (anchor or handle).
	/// The available information varies depending on whether `None`, `One`, or `Multiple` points are currently selected.
	selection_status: SelectionStatus,
	segment: Option<ClosestSegment>,
	snap_cache: SnapCache,
	double_click_handled: bool,
	auto_panning: AutoPanning,
	saved_points_before_anchor_select_toggle: Vec<ManipulatorPointId>,
	select_anchor_toggled: bool,
	dragging_state: DraggingState,
}

impl PathToolData {
	fn save_points_before_anchor_toggle(&mut self, points: Vec<ManipulatorPointId>) -> PathToolFsmState {
		self.saved_points_before_anchor_select_toggle = points;
		PathToolFsmState::Dragging(self.dragging_state)
	}

	fn remove_saved_points(&mut self) {
		self.saved_points_before_anchor_select_toggle.clear();
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
				if let InsertEndKind::Add { extend_selection } = kind {
					closed_segment.adjusted_insert_and_select(shape_editor, responses, extend_selection);
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
		extend_selection: bool,
		direct_insert_without_sliding: bool,
	) -> PathToolFsmState {
		self.double_click_handled = false;
		self.opposing_handle_lengths = None;

		self.drag_start_pos = input.mouse.position;

		// Select the first point within the threshold (in pixels)
		if let Some(selected_points) = shape_editor.change_point_selection(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD, extend_selection) {
			responses.add(DocumentMessage::StartTransaction);

			if let Some(selected_points) = selected_points {
				self.drag_start_pos = input.mouse.position;
				self.start_dragging_point(selected_points, input, document, shape_editor);
				responses.add(OverlaysMessage::Draw);
			}
			PathToolFsmState::Dragging(self.dragging_state)
		}
		// We didn't find a point nearby, so now we'll try to add a point into the closest path segment
		else if let Some(closed_segment) = shape_editor.upper_closest_segment(&document.network_interface, input.mouse.position, SELECTION_TOLERANCE) {
			responses.add(DocumentMessage::StartTransaction);
			if direct_insert_without_sliding {
				self.start_insertion(responses, closed_segment);
				self.end_insertion(shape_editor, responses, InsertEndKind::Add { extend_selection })
			} else {
				self.start_insertion(responses, closed_segment)
			}
		}
		// We didn't find a segment path, so consider selecting the nearest shape instead
		else if let Some(layer) = document.click(input) {
			if extend_selection {
				responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![layer.to_node()] });
			} else {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
			}
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);
			shape_editor.select_connected_anchors(document, layer, input.mouse.position);

			responses.add(DocumentMessage::StartTransaction);

			PathToolFsmState::Dragging(self.dragging_state)
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

	fn update_colinear(&mut self, equidistant: bool, toggle_colinear: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		// Check if the toggle_colinear key has just been pressed
		if toggle_colinear && !self.toggle_colinear_debounce {
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
			self.toggle_colinear_debounce = true;
			return true;
		}
		self.toggle_colinear_debounce = toggle_colinear;

		if equidistant && self.opposing_handle_lengths.is_none() {
			self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(document));
		}
		false
	}

	fn drag(&mut self, equidistant: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Move the selected points with the mouse
		let previous_mouse = document.metadata().document_to_viewport.transform_point2(self.previous_mouse_position);
		let snapped_delta = shape_editor.snap(&mut self.snap_manager, &self.snap_cache, document, input, previous_mouse);
		let handle_lengths = if equidistant { None } else { self.opposing_handle_lengths.take() };
		shape_editor.move_selected_points(handle_lengths, document, snapped_delta, equidistant, responses, true);
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
					Self::Dragging(_) => {
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
			(Self::InsertPoint, PathToolMessage::MouseDown { extend_selection, .. } | PathToolMessage::Enter { extend_selection }) => {
				tool_data.double_click_handled = true;
				let extend_selection = input.keyboard.get(extend_selection as usize);
				tool_data.end_insertion(shape_editor, responses, InsertEndKind::Add { extend_selection })
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
			(
				_,
				PathToolMessage::MouseDown {
					direct_insert_without_sliding,
					extend_selection,
				},
			) => {
				let extend_selection = input.keyboard.get(extend_selection as usize);
				let direct_insert_without_sliding = input.keyboard.get(direct_insert_without_sliding as usize);
				tool_data.mouse_down(shape_editor, document, input, responses, extend_selection, direct_insert_without_sliding)
			}
			(
				PathToolFsmState::DrawingBox,
				PathToolMessage::PointerMove {
					equidistant,
					toggle_colinear,
					move_anchor_with_handles,
				},
			) => {
				tool_data.previous_mouse_position = input.mouse.position;
				responses.add(OverlaysMessage::Draw);

				// Auto-panning
				let messages = [
					PathToolMessage::PointerOutsideViewport {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
					}
					.into(),
					PathToolMessage::PointerMove {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				PathToolFsmState::DrawingBox
			}
			(
				PathToolFsmState::Dragging(_),
				PathToolMessage::PointerMove {
					equidistant,
					toggle_colinear,
					move_anchor_with_handles,
				},
			) => {
				let anchor_and_handle_toggled = input.keyboard.get(move_anchor_with_handles as usize);
				let initial_press = anchor_and_handle_toggled && !tool_data.select_anchor_toggled;
				let released_from_toggle = tool_data.select_anchor_toggled && !anchor_and_handle_toggled;

				if initial_press {
					responses.add(PathToolMessage::SelectedPointUpdated);
					tool_data.select_anchor_toggled = true;
					tool_data.save_points_before_anchor_toggle(shape_editor.selected_points().cloned().collect());
					shape_editor.select_handles_and_anchor_connected_to_current_handle(&document.network_interface);
				} else if released_from_toggle {
					responses.add(PathToolMessage::SelectedPointUpdated);
					tool_data.select_anchor_toggled = false;
					shape_editor.deselect_all_points();
					shape_editor.select_points_by_manipulator_id(&tool_data.saved_points_before_anchor_select_toggle);
					tool_data.remove_saved_points();
				}

				let toggle_colinear_state = input.keyboard.get(toggle_colinear as usize);
				let equidistant_state = input.keyboard.get(equidistant as usize);
				if !tool_data.update_colinear(equidistant_state, toggle_colinear_state, shape_editor, document, responses) {
					tool_data.drag(equidistant_state, shape_editor, document, input, responses);
				}

				// Auto-panning
				let messages = [
					PathToolMessage::PointerOutsideViewport {
						toggle_colinear,
						equidistant,
						move_anchor_with_handles,
					}
					.into(),
					PathToolMessage::PointerMove {
						toggle_colinear,
						equidistant,
						move_anchor_with_handles,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				PathToolFsmState::Dragging(tool_data.dragging_state)
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(offset) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_start_pos += offset;
				}

				PathToolFsmState::DrawingBox
			}
			(PathToolFsmState::Dragging(dragging_state), PathToolMessage::PointerOutsideViewport { equidistant, .. }) => {
				// Auto-panning
				if tool_data.auto_panning.shift_viewport(input, responses).is_some() {
					let equidistant = input.keyboard.get(equidistant as usize);
					tool_data.drag(equidistant, shape_editor, document, input, responses);
				}

				PathToolFsmState::Dragging(dragging_state)
			}
			(
				state,
				PathToolMessage::PointerOutsideViewport {
					equidistant,
					toggle_colinear,
					move_anchor_with_handles,
				},
			) => {
				// Auto-panning
				let messages = [
					PathToolMessage::PointerOutsideViewport {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
					}
					.into(),
					PathToolMessage::PointerMove {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
					}
					.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::Enter { extend_selection }) => {
				let extend_selection = input.keyboard.get(extend_selection as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.network_interface, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !extend_selection);
				}
				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::Ready
			}
			(PathToolFsmState::Dragging { .. }, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}
			// Mouse up
			(PathToolFsmState::DrawingBox, PathToolMessage::DragStop { extend_selection }) => {
				let extend_selection = input.keyboard.get(extend_selection as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.network_interface, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !extend_selection);
				}
				responses.add(OverlaysMessage::Draw);
				responses.add(PathToolMessage::SelectedPointUpdated);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DragStop { extend_selection }) => {
				if tool_data.select_anchor_toggled {
					shape_editor.deselect_all_points();
					shape_editor.select_points_by_manipulator_id(&tool_data.saved_points_before_anchor_select_toggle);
					tool_data.remove_saved_points();
					tool_data.select_anchor_toggled = false;
				}

				let extend_selection = input.keyboard.get(extend_selection as usize);

				let nearest_point = shape_editor.find_nearest_point_indices(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD);

				if let Some((layer, nearest_point)) = nearest_point {
					if tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD && !extend_selection {
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
				shape_editor.move_selected_points(tool_data.opposing_handle_lengths.take(), document, (delta_x, delta_y).into(), true, responses, false);

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
				let colinear = shape_editor.selected_manipulator_angles(&document.network_interface);
				tool_data.dragging_state = DraggingState {
					point_select_state: shape_editor.get_dragging_state(&document.network_interface),
					colinear,
				};
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
			PathToolFsmState::Dragging(dragging_state) => {
				let colinear = dragging_state.colinear;
				let mut dragging_hint_data = HintData(Vec::new());
				dragging_hint_data
					.0
					.push(HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]));

				let drag_anchor = HintInfo::keys([Key::Space], "Drag Anchor");
				let point_select_state_hint_group = match dragging_state.point_select_state {
					PointSelectState::HandleNoPair => vec![drag_anchor],
					PointSelectState::HandleWithPair => {
						let mut hints = vec![drag_anchor];
						hints.push(HintInfo::keys([Key::Tab], "Swap Selected Handles"));
						hints.push(HintInfo::keys(
							[Key::KeyC],
							if colinear == ManipulatorAngle::Colinear {
								"Break Colinear Handles"
							} else {
								"Make Handles Colinear"
							},
						));
						if colinear != ManipulatorAngle::Free {
							hints.push(HintInfo::keys([Key::Alt], "Equidistant Handles"));
						}
						hints
					}
					PointSelectState::Anchor => Vec::new(),
				};

				if !point_select_state_hint_group.is_empty() {
					dragging_hint_data.0.push(HintGroup(point_select_state_hint_group));
				}

				dragging_hint_data
			}
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
