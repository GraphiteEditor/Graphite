use crate::consts::{COLOR_ACCENT, SELECTION_DRAG_ANGLE, SELECTION_TOLERANCE};
use crate::document::utility_types::{AlignAggregate, AlignAxis, FlipAxis};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::mouse::ViewportPosition;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{IconButton, PopoverButton, PropertyHolder, Separator, SeparatorDirection, SeparatorType, Widget, WidgetCallback, WidgetHolder, WidgetLayout, LayoutRow};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::document::Document;
use graphene::intersection::Quad;
use graphene::layers::style::{self, Fill, Stroke};
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum SelectMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	Align {
		axis: AlignAxis,
		aggregate: AlignAggregate,
	},
	DragStart {
		add_to_selection: Key,
	},
	DragStop,
	FlipHorizontal,
	FlipVertical,
	MouseMove {
		snap_angle: Key,
	},
}

impl PropertyHolder for Select {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			name: "".into(),
			widgets: vec![
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignLeft".into(),
					title: "Align Left".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Min,
						}
						.into()
					}),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignHorizontalCenter".into(),
					title: "Align Horizontal Center".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Center,
						}
						.into()
					}),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignRight".into(),
					title: "Align Right".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Max,
						}
						.into()
					}),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Horizontal,
					separator_type: SeparatorType::Unrelated,
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignTop".into(),
					title: "Align Top".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Min,
						}
						.into()
					}),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignVerticalCenter".into(),
					title: "Align Vertical Center".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Center,
						}
						.into()
					}),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignBottom".into(),
					title: "Align Bottom".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Max,
						}
						.into()
					}),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Horizontal,
					separator_type: SeparatorType::Related,
				})),
				WidgetHolder::new(Widget::PopoverButton(PopoverButton {
					title: "Align".into(),
					text: "The contents of this popover menu are coming soon".into(),
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Horizontal,
					separator_type: SeparatorType::Section,
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "FlipHorizontal".into(),
					title: "Flip Horizontal".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| SelectMessage::FlipHorizontal.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "FlipVertical".into(),
					title: "Flip Vertical".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| SelectMessage::FlipVertical.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Horizontal,
					separator_type: SeparatorType::Related,
				})),
				WidgetHolder::new(Widget::PopoverButton(PopoverButton {
					title: "Flip".into(),
					text: "The contents of this popover menu are coming soon".into(),
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Horizontal,
					separator_type: SeparatorType::Section,
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanUnion".into(),
					title: "Boolean Union".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogComingSoon { issue: Some(197) }.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanSubtractFront".into(),
					title: "Boolean Subtract Front".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogComingSoon { issue: Some(197) }.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanSubtractBack".into(),
					title: "Boolean Subtract Back".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogComingSoon { issue: Some(197) }.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanIntersect".into(),
					title: "Boolean Intersect".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogComingSoon { issue: Some(197) }.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanDifference".into(),
					title: "Boolean Difference".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogComingSoon { issue: Some(197) }.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Horizontal,
					separator_type: SeparatorType::Related,
				})),
				WidgetHolder::new(Widget::PopoverButton(PopoverButton {
					title: "Boolean".into(),
					text: "The contents of this popover menu are coming soon".into(),
				})),
			],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &(), data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(SelectMessageDiscriminant; DragStart),
			Dragging => actions!(SelectMessageDiscriminant; DragStop, MouseMove),
			DrawingBox => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	Ready,
	Dragging,
	DrawingBox,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		SelectToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	layers_dragging: Vec<Vec<LayerId>>, // Paths and offsets
	drag_box_overlay_layer: Option<Vec<LayerId>>,
	bounding_box_overlay_layer: Option<Vec<LayerId>>,
	snap_handler: SnapHandler,
}

impl SelectToolData {
	fn selection_quad(&self) -> Quad {
		let bbox = self.selection_box();
		Quad::from_box(bbox)
	}

	fn selection_box(&self) -> [DVec2; 2] {
		if self.drag_current == self.drag_start {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start - tolerance, self.drag_start + tolerance]
		} else {
			[self.drag_start, self.drag_current]
		}
	}
}

fn add_bounding_box(responses: &mut Vec<Message>) -> Vec<LayerId> {
	let path = vec![generate_uuid()];

	let operation = Operation::AddOverlayRect {
		path: path.clone(),
		transform: DAffine2::ZERO.to_cols_array(),
		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Some(Fill::none())),
	};
	responses.push(DocumentMessage::Overlays(operation.into()).into());

	path
}

fn transform_from_box(pos1: DVec2, pos2: DVec2) -> [f64; 6] {
	DAffine2::from_scale_angle_translation(pos2 - pos1, 0., pos1).to_cols_array()
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		_tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectMessage::*;
		use SelectToolFsmState::*;

		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, DocumentIsDirty) => {
					let mut buffer = Vec::new();
					let response = match (document.selected_visible_layers_bounding_box(), data.bounding_box_overlay_layer.take()) {
						(None, Some(path)) => DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into(),
						(Some([pos1, pos2]), path) => {
							let path = path.unwrap_or_else(|| add_bounding_box(&mut buffer));

							data.bounding_box_overlay_layer = Some(path.clone());

							let half_pixel_offset = DVec2::splat(0.5);
							let pos1 = pos1 + half_pixel_offset;
							let pos2 = pos2 - half_pixel_offset;
							let transform = transform_from_box(pos1, pos2);
							DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path, transform }.into()).into()
						}
						(_, _) => Message::NoOp,
					};
					responses.push_front(response);
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));
					self
				}
				(Ready, DragStart { add_to_selection }) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					let mut buffer = Vec::new();
					let mut selected: Vec<_> = document.selected_visible_layers().map(|path| path.to_vec()).collect();
					let quad = data.selection_quad();
					let mut intersection = document.graphene_document.intersects_quad_root(quad);
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// If the user clicks on new shape, make that layer their new selection.
					// Otherwise enter the box select mode
					let state = if selected.iter().any(|path| intersection.contains(path)) {
						buffer.push(DocumentMessage::StartTransaction.into());
						data.layers_dragging = selected;
						Dragging
					} else {
						if !input.keyboard.get(add_to_selection as usize) {
							buffer.push(DocumentMessage::DeselectAllLayers.into());
							data.layers_dragging.clear();
						}

						if let Some(intersection) = intersection.pop() {
							selected = vec![intersection];
							buffer.push(DocumentMessage::AddSelectedLayers { additional_layers: selected.clone() }.into());
							buffer.push(DocumentMessage::StartTransaction.into());
							data.layers_dragging.append(&mut selected);
							Dragging
						} else {
							data.drag_box_overlay_layer = Some(add_bounding_box(&mut buffer));
							DrawingBox
						}
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));

					// TODO: Probably delete this now that the overlays system has moved to a separate Graphene document? (@0hypercube)
					let ignore_layers = if let Some(bounding_box) = &data.bounding_box_overlay_layer {
						vec![bounding_box.clone()]
					} else {
						Vec::new()
					};
					data.snap_handler.start_snap(document, document.non_selected_layers_sorted(), &ignore_layers);
					state
				}
				(Dragging, MouseMove { snap_angle }) => {
					// TODO: This is a cheat. Break out the relevant functionality from the handler above and call it from there and here.
					responses.push_front(SelectMessage::DocumentIsDirty.into());

					let mouse_position = if input.keyboard.get(snap_angle as usize) {
						let mouse_position = input.mouse.position - data.drag_start;
						let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
						let angle = -mouse_position.angle_between(DVec2::X);
						let snapped_angle = (angle / snap_resolution).round() * snap_resolution;
						DVec2::new(snapped_angle.cos(), snapped_angle.sin()) * mouse_position.length() + data.drag_start
					} else {
						input.mouse.position
					};

					let mouse_delta = mouse_position - data.drag_current;

					let closest_move = data.snap_handler.snap_layers(document, &data.layers_dragging, mouse_delta);
					// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
					for path in Document::shallowest_unique_layers(data.layers_dragging.iter().map(|path| path.as_slice())) {
						responses.push_front(
							Operation::TransformLayerInViewport {
								path: path.to_vec(),
								transform: DAffine2::from_translation(mouse_delta + closest_move).to_cols_array(),
							}
							.into(),
						);
					}
					data.drag_current = mouse_position + closest_move;
					Dragging
				}
				(DrawingBox, MouseMove { .. }) => {
					data.drag_current = input.mouse.position;
					let half_pixel_offset = DVec2::splat(0.5);
					let start = data.drag_start + half_pixel_offset;
					let size = data.drag_current - start + half_pixel_offset;

					responses.push_front(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: data.drag_box_overlay_layer.clone().unwrap(),
								transform: DAffine2::from_scale_angle_translation(size, 0., start).to_cols_array(),
							}
							.into(),
						)
						.into(),
					);
					DrawingBox
				}
				(Dragging, DragStop) => {
					let response = match input.mouse.position.distance(data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					data.snap_handler.cleanup();
					responses.push_front(response.into());
					Ready
				}
				(DrawingBox, DragStop) => {
					let quad = data.selection_quad();
					responses.push_front(
						DocumentMessage::AddSelectedLayers {
							additional_layers: document.graphene_document.intersects_quad_root(quad),
						}
						.into(),
					);
					responses.push_front(
						DocumentMessage::Overlays(
							Operation::DeleteLayer {
								path: data.drag_box_overlay_layer.take().unwrap(),
							}
							.into(),
						)
						.into(),
					);
					Ready
				}
				(_, Abort) => {
					let mut delete = |path: &mut Option<Vec<LayerId>>| path.take().map(|path| responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into()));
					delete(&mut data.drag_box_overlay_layer);
					delete(&mut data.bounding_box_overlay_layer);
					Ready
				}
				(_, Align { axis, aggregate }) => {
					responses.push_back(DocumentMessage::AlignSelectedLayers { axis, aggregate }.into());

					self
				}
				(_, FlipHorizontal) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::X }.into());

					self
				}
				(_, FlipVertical) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::Y }.into());

					self
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			SelectToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Drag Selected"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyG])],
						mouse: None,
						label: String::from("Grab Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyR])],
						mouse: None,
						label: String::from("Rotate Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyS])],
						mouse: None,
						label: String::from("Scale Selected"),
						plus: false,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::Lmb),
						label: String::from("Select Object"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl])],
						mouse: None,
						label: String::from("Innermost"),
						plus: true,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Grow/Shrink Selection"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Select Area"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Grow/Shrink Selection"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![
							KeysGroup(vec![Key::KeyArrowUp]),
							KeysGroup(vec![Key::KeyArrowRight]),
							KeysGroup(vec![Key::KeyArrowDown]),
							KeysGroup(vec![Key::KeyArrowLeft]),
						],
						mouse: None,
						label: String::from("Nudge Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Big Increment Nudge"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Move Duplicate"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl, Key::KeyD])],
						mouse: None,
						label: String::from("Duplicate"),
						plus: false,
					},
				]),
			]),
			SelectToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Constrain to Axis"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyControl])],
					mouse: None,
					label: String::from("Snap to Points (coming soon)"),
					plus: false,
				},
			])]),
			SelectToolFsmState::DrawingBox => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}
