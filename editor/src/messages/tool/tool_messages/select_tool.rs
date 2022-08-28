use crate::application::generate_uuid;
use crate::consts::{ROTATE_SNAP_ANGLE, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, MouseMotion};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::{IconButton, PopoverButton};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis};
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::path_outline::*;
use crate::messages::tool::common_functionality::snapping::{self, SnapManager};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene::boolean_ops::BooleanOperation;
use graphene::document::Document;
use graphene::intersection::Quad;
use graphene::layers::layer_info::LayerDataType;
use graphene::LayerId;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct SelectTool {
	fsm_state: SelectToolFsmState,
	tool_data: SelectToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum SelectToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	SelectionChanged,

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
		duplicate: Key,
	},
}

impl ToolMetadata for SelectTool {
	fn icon_name(&self) -> String {
		"GeneralSelectTool".into()
	}
	fn tooltip(&self) -> String {
		"Select Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Select
	}
}

impl PropertyHolder for SelectTool {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
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
					header: "Align".into(),
					text: "The contents of this popover menu are coming soon".into(),
					..Default::default()
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
					header: "Flip".into(),
					text: "The contents of this popover menu are coming soon".into(),
					..Default::default()
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
					header: "Boolean".into(),
					text: "The contents of this popover menu are coming soon".into(),
					..Default::default()
				})),
			],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for SelectTool {
	fn process_message(&mut self, message: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if message == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if message == ToolMessage::UpdateCursor {
			responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
			return;
		}

		let new_state = self.fsm_state.transition(message, &mut self.tool_data, tool_data, &(), responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(SelectToolMessageDiscriminant;
				DragStart,
				PointerMove,
				Abort,
				EditLayer,
			),
			_ => actions!(SelectToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
				EditLayer,
			),
		}
	}
}

impl ToolTransition for SelectTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(SelectToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(SelectToolMessage::Abort.into()),
			selection_changed: Some(SelectToolMessage::SelectionChanged.into()),
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
	layers_dragging: Vec<Vec<LayerId>>,
	not_duplicated_layers: Option<Vec<Vec<LayerId>>>,
	drag_box_overlay_layer: Option<Vec<LayerId>>,
	path_outlines: PathOutline,
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	snap_manager: SnapManager,
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
		tool_data: &mut Self::ToolData,
		(document, _global_tool_data, input, font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectToolFsmState::*;
		use SelectToolMessage::*;

		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, DocumentIsDirty | SelectionChanged) => {
					match (document.selected_visible_layers_bounding_box(font_cache), tool_data.bounding_box_overlays.take()) {
						(None, Some(bounding_box_overlays)) => bounding_box_overlays.delete(responses),
						(Some(bounds), paths) => {
							let mut bounding_box_overlays = paths.unwrap_or_else(|| BoundingBoxOverlays::new(responses));

							bounding_box_overlays.bounds = bounds;
							bounding_box_overlays.transform = DAffine2::IDENTITY;

							bounding_box_overlays.transform(responses);

							tool_data.bounding_box_overlays = Some(bounding_box_overlays);
						}
						(_, _) => {}
					};

					tool_data.path_outlines.update_selected(document.selected_visible_layers(), document, responses, font_cache);
					tool_data.path_outlines.intersect_test_hovered(input, document, responses, font_cache);

					self
				}
				(_, EditLayer) => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					if let Some(Ok(intersect)) = document
						.graphene_document
						.intersects_quad_root(quad, font_cache)
						.last()
						.map(|path| document.graphene_document.layer(path))
					{
						match intersect.data {
							LayerDataType::Text(_) => {
								responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Text }.into());
								responses.push_back(TextToolMessage::Interact.into());
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
					tool_data.path_outlines.clear_hovered(responses);

					tool_data.drag_start = input.mouse.position;
					tool_data.drag_current = input.mouse.position;

					let dragging_bounds = if let Some(bounding_box) = &mut tool_data.bounding_box_overlays {
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

					let rotating_bounds = if let Some(bounding_box) = &mut tool_data.bounding_box_overlays {
						bounding_box.check_rotate(input.mouse.position)
					} else {
						false
					};

					let mut selected: Vec<_> = document.selected_visible_layers().map(|path| path.to_vec()).collect();
					let quad = tool_data.selection_quad();
					let mut intersection = document.graphene_document.intersects_quad_root(quad, font_cache);
					// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
					// If the user is dragging the rotate trigger, go into RotatingBounds mode.
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// If the user clicks on new shape, make that layer their new selection.
					// Otherwise enter the box select mode
					let state = if let Some(selected_edges) = dragging_bounds {
						let snap_x = selected_edges.2 || selected_edges.3;
						let snap_y = selected_edges.0 || selected_edges.1;

						tool_data.snap_manager.start_snap(document, document.bounding_boxes(Some(&selected), None, font_cache), snap_x, snap_y);
						tool_data
							.snap_manager
							.add_all_document_handles(document, &[], &selected.iter().map(|x| x.as_slice()).collect::<Vec<_>>(), &[]);

						tool_data.layers_dragging = selected;

						ResizingBounds
					} else if rotating_bounds {
						if let Some(bounds) = &mut tool_data.bounding_box_overlays {
							let selected = selected.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.pivot, &selected, responses, &document.graphene_document);

							*selected.pivot = selected.calculate_pivot(font_cache);
						}

						tool_data.layers_dragging = selected;

						RotatingBounds
					} else if intersection.last().map(|last| selected.contains(last)).unwrap_or(false) {
						responses.push_back(DocumentMessage::StartTransaction.into());

						tool_data.layers_dragging = selected;

						tool_data
							.snap_manager
							.start_snap(document, document.bounding_boxes(Some(&tool_data.layers_dragging), None, font_cache), true, true);

						Dragging
					} else {
						if !input.keyboard.get(add_to_selection as usize) {
							responses.push_back(DocumentMessage::DeselectAllLayers.into());
							tool_data.layers_dragging.clear();
						}

						if let Some(intersection) = intersection.pop() {
							selected = vec![intersection];
							responses.push_back(DocumentMessage::AddSelectedLayers { additional_layers: selected.clone() }.into());
							responses.push_back(DocumentMessage::StartTransaction.into());
							tool_data.layers_dragging.append(&mut selected);
							tool_data
								.snap_manager
								.start_snap(document, document.bounding_boxes(Some(&tool_data.layers_dragging), None, font_cache), true, true);

							Dragging
						} else {
							tool_data.drag_box_overlay_layer = Some(add_bounding_box(responses));
							DrawingBox
						}
					};
					tool_data.not_duplicated_layers = None;

					state
				}
				(Dragging, PointerMove { axis_align, duplicate, .. }) => {
					// TODO: This is a cheat. Break out the relevant functionality from the handler above and call it from there and here.
					responses.push_front(SelectToolMessage::DocumentIsDirty.into());

					let mouse_position = axis_align_drag(input.keyboard.get(axis_align as usize), input.mouse.position, tool_data.drag_start);

					let mouse_delta = mouse_position - tool_data.drag_current;

					let snap = tool_data
						.layers_dragging
						.iter()
						.filter_map(|path| document.graphene_document.viewport_bounding_box(path, font_cache).ok()?)
						.flat_map(snapping::expand_bounds)
						.collect();

					if input.keyboard.get(duplicate as usize) && tool_data.not_duplicated_layers.is_none() {
						tool_data.duplicate(document, responses);
					} else if !input.keyboard.get(duplicate as usize) && tool_data.not_duplicated_layers.is_some() {
						tool_data.undo_duplicate(responses);
					}

					let closest_move = tool_data.snap_manager.snap_layers(responses, document, snap, mouse_delta);
					// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
					for path in Document::shallowest_unique_layers(tool_data.layers_dragging.iter()) {
						responses.push_front(
							Operation::TransformLayerInViewport {
								path: path.clone(),
								transform: DAffine2::from_translation(mouse_delta + closest_move).to_cols_array(),
							}
							.into(),
						);
					}
					tool_data.drag_current = mouse_position + closest_move;
					Dragging
				}
				(ResizingBounds, PointerMove { axis_align, center, .. }) => {
					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						if let Some(movement) = &mut bounds.selected_edges {
							let (center, axis_align) = (input.keyboard.get(center as usize), input.keyboard.get(axis_align as usize));

							let mouse_position = input.mouse.position;

							let snapped_mouse_position = tool_data.snap_manager.snap_position(responses, document, mouse_position);

							let (_, size) = movement.new_size(snapped_mouse_position, bounds.transform, center, axis_align);
							let delta = movement.bounds_to_scale_transform(center, size);

							let selected = tool_data.layers_dragging.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.pivot, &selected, responses, &document.graphene_document);

							selected.update_transforms(delta);
						}
					}
					ResizingBounds
				}
				(RotatingBounds, PointerMove { snap_angle, .. }) => {
					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						let angle = {
							let start_offset = tool_data.drag_start - bounds.pivot;
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

						let selected = tool_data.layers_dragging.iter().collect::<Vec<_>>();
						let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.pivot, &selected, responses, &document.graphene_document);

						selected.update_transforms(delta);
					}

					RotatingBounds
				}
				(DrawingBox, PointerMove { .. }) => {
					tool_data.drag_current = input.mouse.position;

					responses.push_front(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: tool_data.drag_box_overlay_layer.clone().unwrap(),
								transform: transform_from_box(tool_data.drag_start, tool_data.drag_current, DAffine2::IDENTITY).to_cols_array(),
							}
							.into(),
						)
						.into(),
					);
					DrawingBox
				}
				(Ready, PointerMove { .. }) => {
					let cursor = tool_data.bounding_box_overlays.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true));

					// Generate the select outline (but not if the user is going to use the bound overlays)
					if cursor == MouseCursorIcon::Default {
						tool_data.path_outlines.intersect_test_hovered(input, document, responses, font_cache);
					} else {
						tool_data.path_outlines.clear_hovered(responses);
					}

					if tool_data.cursor != cursor {
						tool_data.cursor = cursor;
						responses.push_back(FrontendMessage::UpdateMouseCursor { cursor }.into());
					}

					Ready
				}
				(Dragging, DragStop) => {
					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					tool_data.snap_manager.cleanup(responses);
					responses.push_front(response.into());
					Ready
				}
				(ResizingBounds, DragStop) => {
					tool_data.snap_manager.cleanup(responses);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(RotatingBounds, DragStop) => {
					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(DrawingBox, DragStop) => {
					let quad = tool_data.selection_quad();
					responses.push_front(
						DocumentMessage::AddSelectedLayers {
							additional_layers: document.graphene_document.intersects_quad_root(quad, font_cache),
						}
						.into(),
					);
					responses.push_front(
						DocumentMessage::Overlays(
							Operation::DeleteLayer {
								path: tool_data.drag_box_overlay_layer.take().unwrap(),
							}
							.into(),
						)
						.into(),
					);
					Ready
				}
				(Dragging, Abort) => {
					tool_data.snap_manager.cleanup(responses);
					responses.push_back(DocumentMessage::Undo.into());

					tool_data.path_outlines.clear_selected(responses);

					Ready
				}
				(_, Abort) => {
					if let Some(path) = tool_data.drag_box_overlay_layer.take() {
						responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into())
					};
					if let Some(mut bounding_box_overlays) = tool_data.bounding_box_overlays.take() {
						let selected = tool_data.layers_dragging.iter().collect::<Vec<_>>();
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

					tool_data.path_outlines.clear_hovered(responses);
					tool_data.path_outlines.clear_selected(responses);

					tool_data.snap_manager.cleanup(responses);
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
					key_groups_mac: None,
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Drag Selected"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyG])],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Grab Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyR])],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Rotate Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyS])],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Scale Selected"),
						plus: false,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						key_groups_mac: None,
						mouse: Some(MouseMotion::Lmb),
						label: String::from("Select Object"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Control])],
						key_groups_mac: Some(vec![KeysGroup(vec![Key::Command])]),
						mouse: None,
						label: String::from("Innermost"),
						plus: true,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Shift])],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Grow/Shrink Selection"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						key_groups_mac: None,
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Select Area"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Shift])],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Grow/Shrink Selection"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![
							KeysGroup(vec![Key::ArrowUp]),
							KeysGroup(vec![Key::ArrowRight]),
							KeysGroup(vec![Key::ArrowDown]),
							KeysGroup(vec![Key::ArrowLeft]),
						],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Nudge Selected"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Shift])],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Big Increment Nudge"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Alt])],
						key_groups_mac: None,
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Move Duplicate"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Control, Key::KeyD])],
						key_groups_mac: Some(vec![KeysGroup(vec![Key::Command, Key::KeyD])]),
						mouse: None,
						label: String::from("Duplicate"),
						plus: false,
					},
				]),
			]),
			SelectToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Shift])],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Constrain to Axis"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Control])],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Snap to Points (coming soon)"),
					plus: false,
				},
			])]),
			SelectToolFsmState::DrawingBox => HintData(vec![]),
			SelectToolFsmState::ResizingBounds => HintData(vec![]),
			SelectToolFsmState::RotatingBounds => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::Control])],
				key_groups_mac: None,
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

impl SelectToolData {
	/// Duplicates the currently dragging layers. Called when alt is pressed and the layers have not yet been duplicated.
	fn duplicate(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::DeselectAllLayers.into());

		self.not_duplicated_layers = Some(self.layers_dragging.clone());

		// Duplicate each previously selected layer and select the new ones.
		for layer_path in Document::shallowest_unique_layers(self.layers_dragging.iter_mut()) {
			// Moves the origional back to its starting position.
			responses.push_front(
				Operation::TransformLayerInViewport {
					path: layer_path.clone(),
					transform: DAffine2::from_translation(self.drag_start - self.drag_current).to_cols_array(),
				}
				.into(),
			);

			// Copy the layers.
			// Not using the Copy message allows us to retrieve the ids of the new layers to initalise the drag.
			let layer = match document.graphene_document.layer(layer_path) {
				Ok(layer) => layer.clone(),
				Err(e) => {
					log::warn!("Could not access selected layer {:?}: {:?}", layer_path, e);
					continue;
				}
			};
			let layer_metadata = *document.layer_metadata(layer_path);
			*layer_path.last_mut().unwrap() = generate_uuid();

			responses.push_back(
				Operation::InsertLayer {
					layer,
					destination_path: layer_path.clone(),
					insert_index: -1,
				}
				.into(),
			);

			responses.push_back(
				DocumentMessage::UpdateLayerMetadata {
					layer_path: layer_path.clone(),
					layer_metadata,
				}
				.into(),
			);
		}
	}

	/// Removes the duplicated. Called when alt is released and the layers have been duplicated.
	fn undo_duplicate(&mut self, responses: &mut VecDeque<Message>) {
		let origionals = match self.not_duplicated_layers.take() {
			Some(x) => x,
			None => return,
		};

		responses.push_back(DocumentMessage::DeselectAllLayers.into());

		// Delete the duplicated layers
		for layer_path in Document::shallowest_unique_layers(self.layers_dragging.iter()) {
			responses.push_back(Operation::DeleteLayer { path: layer_path.clone() }.into());
		}

		// Move the origional to under the mouse
		for layer_path in Document::shallowest_unique_layers(origionals.iter()) {
			responses.push_front(
				Operation::TransformLayerInViewport {
					path: layer_path.clone(),
					transform: DAffine2::from_translation(self.drag_current - self.drag_start).to_cols_array(),
				}
				.into(),
			);
		}

		// Select the origionals
		responses.push_back(
			DocumentMessage::SetSelectedLayers {
				replacement_selected_layers: origionals.clone(),
			}
			.into(),
		);

		self.layers_dragging = origionals;
	}
}
