use crate::consts::{ROTATE_SNAP_ANGLE, SELECTION_TOLERANCE};
use crate::document::transformation::Selected;
use crate::document::utility_types::{AlignAggregate, AlignAxis, FlipAxis};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::mouse::ViewportPosition;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{IconButton, LayoutRow, PopoverButton, PropertyHolder, Separator, SeparatorDirection, SeparatorType, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolType};
use graphene::boolean_ops::BooleanOperation;
use graphene::document::Document;
use graphene::intersection::Quad;
use graphene::layers::layer_info::LayerDataType;
use graphene::Operation;

use super::shared::hover_outline::*;
use super::shared::transformation_cage::*;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct SelectTool {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum SelectToolMessage {
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
	EditLayer,
	FlipHorizontal,
	FlipVertical,
	PointerMove {
		axis_align: Key,
		snap_angle: Key,
		center: Key,
	},
}

impl PropertyHolder for SelectTool {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			widgets: vec![
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "AlignLeft".into(),
					tooltip: "Align Left".into(),
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
					tooltip: "Align Horizontal Center".into(),
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
					tooltip: "Align Right".into(),
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
					tooltip: "Align Top".into(),
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
					tooltip: "Align Vertical Center".into(),
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
					tooltip: "Align Bottom".into(),
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
					tooltip: "Flip Horizontal".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| SelectToolMessage::FlipHorizontal.into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "FlipVertical".into(),
					tooltip: "Flip Vertical".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| SelectToolMessage::FlipVertical.into()),
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
					tooltip: "Boolean Union".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::BooleanOperation(BooleanOperation::Union).into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanSubtractFront".into(),
					tooltip: "Boolean Subtract Front".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::BooleanOperation(BooleanOperation::SubtractFront).into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanSubtractBack".into(),
					tooltip: "Boolean Subtract Back".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::BooleanOperation(BooleanOperation::SubtractBack).into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanIntersect".into(),
					tooltip: "Boolean Intersect".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::BooleanOperation(BooleanOperation::Intersection).into()),
					..IconButton::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "BooleanDifference".into(),
					tooltip: "Boolean Difference".into(),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::BooleanOperation(BooleanOperation::Difference).into()),
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

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for SelectTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &(), data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(SelectToolMessageDiscriminant; DragStart, PointerMove, Abort, EditLayer),
			_ => actions!(SelectToolMessageDiscriminant; DragStop, PointerMove, Abort, EditLayer),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	Ready,
	Dragging,
	DrawingBox,
	ResizingBounds,
	RotatingBounds,
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
	hover_outline_overlay: HoverOutline,
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	snap_handler: SnapHandler,
	cursor: MouseCursorIcon,
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
		use SelectToolFsmState::*;
		use SelectToolMessage::*;

		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, DocumentIsDirty) => {
					let mut buffer = Vec::new();
					match (document.selected_visible_layers_bounding_box(), data.bounding_box_overlays.take()) {
						(None, Some(bounding_box_overlays)) => bounding_box_overlays.delete(&mut buffer),
						(Some(bounds), paths) => {
							let mut bounding_box_overlays = paths.unwrap_or_else(|| BoundingBoxOverlays::new(&mut buffer));

							bounding_box_overlays.bounds = bounds;
							bounding_box_overlays.transform = DAffine2::IDENTITY;

							bounding_box_overlays.transform(&mut buffer);

							data.bounding_box_overlays = Some(bounding_box_overlays);
						}
						(_, _) => {}
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));
					self
				}
				(_, EditLayer) => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					if let Some(Ok(intersect)) = document.graphene_document.intersects_quad_root(quad).last().map(|path| document.graphene_document.layer(path)) {
						match intersect.data {
							LayerDataType::Text(_) => {
								responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Text }.into());
								responses.push_back(TextMessage::Interact.into());
							}
							LayerDataType::Shape(_) => {
								responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Path }.into());
							}
							_ => {}
						}
					}

					self
				}
				(Ready, DragStart { add_to_selection }) => {
					data.hover_outline_overlay.clear(responses);

					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					let mut buffer = Vec::new();

					let dragging_bounds = if let Some(bounding_box) = &mut data.bounding_box_overlays {
						let edges = bounding_box.check_selected_edges(input.mouse.position);

						bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
							let edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
							bounding_box.pivot = edges.calculate_pivot();
							edges
						});

						edges
					} else {
						None
					};

					let rotating_bounds = if let Some(bounding_box) = &mut data.bounding_box_overlays {
						bounding_box.check_rotate(input.mouse.position)
					} else {
						false
					};

					let mut selected: Vec<_> = document.selected_visible_layers().map(|path| path.to_vec()).collect();
					let quad = data.selection_quad();
					let mut intersection = document.graphene_document.intersects_quad_root(quad);
					// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
					// If the user is dragging the rotate trigger, go into RotatingBounds mode.
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// If the user clicks on new shape, make that layer their new selection.
					// Otherwise enter the box select mode
					let state = if let Some(selected_edges) = dragging_bounds {
						let snap_x = selected_edges.2 || selected_edges.3;
						let snap_y = selected_edges.0 || selected_edges.1;

						data.snap_handler.start_snap(document, document.bounding_boxes(Some(&selected), None), snap_x, snap_y);

						data.layers_dragging = selected;

						ResizingBounds
					} else if rotating_bounds {
						if let Some(bounds) = &mut data.bounding_box_overlays {
							let selected = selected.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.pivot, &selected, responses, &document.graphene_document);

							*selected.pivot = selected.calculate_pivot(&document.graphene_document.font_cache);
						}

						data.layers_dragging = selected;

						RotatingBounds
					} else if selected.iter().any(|path| intersection.contains(path)) {
						buffer.push(DocumentMessage::StartTransaction.into());
						data.layers_dragging = selected;

						data.snap_handler.start_snap(document, document.bounding_boxes(Some(&data.layers_dragging), None), true, true);

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
							data.snap_handler.start_snap(document, document.bounding_boxes(Some(&data.layers_dragging), None), true, true);

							Dragging
						} else {
							data.drag_box_overlay_layer = Some(add_bounding_box(&mut buffer));
							DrawingBox
						}
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));

					state
				}
				(Dragging, PointerMove { axis_align, .. }) => {
					// TODO: This is a cheat. Break out the relevant functionality from the handler above and call it from there and here.
					responses.push_front(SelectToolMessage::DocumentIsDirty.into());

					let mouse_position = axis_align_drag(input.keyboard.get(axis_align as usize), input.mouse.position, data.drag_start);

					let mouse_delta = mouse_position - data.drag_current;

					let snap = data
						.layers_dragging
						.iter()
						.filter_map(|path| document.graphene_document.viewport_bounding_box(path).ok()?)
						.flat_map(|[bound1, bound2]| [bound1, bound2, (bound1 + bound2) / 2.])
						.map(|vec| vec.into())
						.unzip();

					let closest_move = data.snap_handler.snap_layers(responses, document, snap, input.viewport_bounds.size(), mouse_delta);
					// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
					for path in Document::shallowest_unique_layers(data.layers_dragging.iter()) {
						responses.push_front(
							Operation::TransformLayerInViewport {
								path: path.clone(),
								transform: DAffine2::from_translation(mouse_delta + closest_move).to_cols_array(),
							}
							.into(),
						);
					}
					data.drag_current = mouse_position + closest_move;
					Dragging
				}
				(ResizingBounds, PointerMove { axis_align, center, .. }) => {
					if let Some(bounds) = &mut data.bounding_box_overlays {
						if let Some(movement) = &mut bounds.selected_edges {
							let (center, axis_align) = (input.keyboard.get(center as usize), input.keyboard.get(axis_align as usize));

							let mouse_position = input.mouse.position;

							let snapped_mouse_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, mouse_position);

							let [_position, size] = movement.new_size(snapped_mouse_position, bounds.transform, center, axis_align);
							let delta = movement.bounds_to_scale_transform(center, size);

							let selected = data.layers_dragging.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.pivot, &selected, responses, &document.graphene_document);

							selected.update_transforms(delta);
						}
					}
					ResizingBounds
				}
				(RotatingBounds, PointerMove { snap_angle, .. }) => {
					if let Some(bounds) = &mut data.bounding_box_overlays {
						let angle = {
							let start_offset = data.drag_start - bounds.pivot;
							let end_offset = input.mouse.position - bounds.pivot;

							start_offset.angle_between(end_offset)
						};

						let snapped_angle = if input.keyboard.get(snap_angle as usize) {
							let snap_resolution = ROTATE_SNAP_ANGLE.to_radians();
							(angle / snap_resolution).round() * snap_resolution
						} else {
							angle
						};

						let delta = DAffine2::from_angle(snapped_angle);

						let selected = data.layers_dragging.iter().collect::<Vec<_>>();
						let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.pivot, &selected, responses, &document.graphene_document);

						selected.update_transforms(delta);
					}

					RotatingBounds
				}
				(DrawingBox, PointerMove { .. }) => {
					data.drag_current = input.mouse.position;

					responses.push_front(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: data.drag_box_overlay_layer.clone().unwrap(),
								transform: transform_from_box(data.drag_start, data.drag_current, DAffine2::IDENTITY).to_cols_array(),
							}
							.into(),
						)
						.into(),
					);
					DrawingBox
				}
				(Ready, PointerMove { .. }) => {
					let cursor = data.bounding_box_overlays.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true));

					// Generate the select outline (but not if the user is going to use the bound overlays)
					if cursor == MouseCursorIcon::Default {
						// Get the layer the user is hovering over
						let tolerance = DVec2::splat(SELECTION_TOLERANCE);
						let quad = Quad::from_box([input.mouse.position - tolerance, input.mouse.position + tolerance]);
						let mut intersection = document.graphene_document.intersects_quad_root(quad);

						// If the user is hovering over a layer they have not already selected, then update outline
						if !document.selected_visible_layers().any(|path| intersection.contains(&path.to_vec())) {
							if let Some(path) = intersection.pop() {
								data.hover_outline_overlay.update(path, document, responses)
							} else {
								data.hover_outline_overlay.clear(responses);
							}
						} else {
							data.hover_outline_overlay.clear(responses);
						}
					} else {
						data.hover_outline_overlay.clear(responses);
					}

					if data.cursor != cursor {
						data.cursor = cursor;
						responses.push_back(FrontendMessage::UpdateMouseCursor { cursor }.into());
					}

					Ready
				}
				(Dragging, DragStop) => {
					let response = match input.mouse.position.distance(data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					data.snap_handler.cleanup(responses);
					responses.push_front(response.into());
					Ready
				}
				(ResizingBounds, DragStop) => {
					data.snap_handler.cleanup(responses);

					if let Some(bounds) = &mut data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(RotatingBounds, DragStop) => {
					if let Some(bounds) = &mut data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

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
				(Dragging, Abort) => {
					data.snap_handler.cleanup(responses);
					responses.push_back(DocumentMessage::Undo.into());
					Ready
				}
				(_, Abort) => {
					if let Some(path) = data.drag_box_overlay_layer.take() {
						responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into())
					};
					if let Some(mut bounding_box_overlays) = data.bounding_box_overlays.take() {
						let selected = data.layers_dragging.iter().collect::<Vec<_>>();
						let mut selected = Selected::new(
							&mut bounding_box_overlays.original_transforms,
							&mut bounding_box_overlays.pivot,
							&selected,
							responses,
							&document.graphene_document,
						);

						selected.revert_operation();

						bounding_box_overlays.delete(responses);
					}

					data.hover_outline_overlay.clear(responses);

					data.snap_handler.cleanup(responses);
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
			SelectToolFsmState::ResizingBounds => HintData(vec![]),
			SelectToolFsmState::RotatingBounds => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyControl])],
				mouse: None,
				label: String::from("Snap 15°"),
				plus: false,
			}])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}
