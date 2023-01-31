use crate::application::generate_uuid;
use crate::consts::{ROTATE_SNAP_ANGLE, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::assist_widgets::{PivotAssist, PivotPosition};
use crate::messages::layout::utility_types::widgets::button_widgets::{IconButton, PopoverButton};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis};
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::path_outline::*;
use crate::messages::tool::common_functionality::pivot::Pivot;
use crate::messages::tool::common_functionality::snapping::{self, SnapManager};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::boolean_ops::BooleanOperation;
use document_legacy::document::Document;
use document_legacy::intersection::Quad;
use document_legacy::layers::layer_info::LayerDataType;
use document_legacy::LayerId;
use document_legacy::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct SelectTool {
	fsm_state: SelectToolFsmState,
	tool_data: SelectToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, specta::Type)]
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
	SetPivot {
		position: PivotPosition,
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
				IconButton::new("AlignLeft", 24)
					.tooltip("Align Left")
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Min,
						}
						.into()
					})
					.widget_holder(),
				IconButton::new("AlignHorizontalCenter", 24)
					.tooltip("Align Horizontal Center")
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Center,
						}
						.into()
					})
					.widget_holder(),
				IconButton::new("AlignRight", 24)
					.tooltip("Align Right")
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Max,
						}
						.into()
					})
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				IconButton::new("AlignTop", 24)
					.tooltip("Align Top")
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Min,
						}
						.into()
					})
					.widget_holder(),
				IconButton::new("AlignVerticalCenter", 24)
					.tooltip("Align Vertical Center")
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Center,
						}
						.into()
					})
					.widget_holder(),
				IconButton::new("AlignBottom", 24)
					.tooltip("Align Bottom")
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Max,
						}
						.into()
					})
					.widget_holder(),
				WidgetHolder::related_separator(),
				PopoverButton::new("Align", "Coming soon").widget_holder(),
				Separator::new(SeparatorDirection::Horizontal, SeparatorType::Section).widget_holder(),
				IconButton::new("FlipHorizontal", 24)
					.tooltip("Flip Horizontal")
					.on_update(|_| SelectToolMessage::FlipHorizontal.into())
					.widget_holder(),
				IconButton::new("FlipVertical", 24)
					.tooltip("Flip Vertical")
					.on_update(|_| SelectToolMessage::FlipVertical.into())
					.widget_holder(),
				WidgetHolder::related_separator(),
				WidgetHolder::new(Widget::PopoverButton(PopoverButton {
					header: "Flip".into(),
					text: "Coming soon".into(),
					..Default::default()
				})),
				Separator::new(SeparatorDirection::Horizontal, SeparatorType::Section).widget_holder(),
				IconButton::new("BooleanUnion", 24)
					.tooltip("Boolean Union")
					.on_update(|_| DocumentMessage::BooleanOperation(BooleanOperation::Union).into())
					.widget_holder(),
				IconButton::new("BooleanSubtractFront", 24)
					.tooltip("Boolean Subtract Front")
					.on_update(|_| DocumentMessage::BooleanOperation(BooleanOperation::SubtractFront).into())
					.widget_holder(),
				IconButton::new("BooleanSubtractBack", 24)
					.tooltip("Boolean Subtract Back")
					.on_update(|_| DocumentMessage::BooleanOperation(BooleanOperation::SubtractBack).into())
					.widget_holder(),
				IconButton::new("BooleanIntersect", 24)
					.tooltip("Boolean Intersect")
					.on_update(|_| DocumentMessage::BooleanOperation(BooleanOperation::Intersection).into())
					.widget_holder(),
				IconButton::new("BooleanDifference", 24)
					.tooltip("Boolean Difference")
					.on_update(|_| DocumentMessage::BooleanOperation(BooleanOperation::Difference).into())
					.widget_holder(),
				WidgetHolder::related_separator(),
				PopoverButton::new("Boolean", "Coming soon").widget_holder(),
				Separator::new(SeparatorDirection::Horizontal, SeparatorType::Section).widget_holder(),
				// We'd like this widget to hide and show itself whenever the transformation cage is active or inactive (i.e. when no layers are selected)
				PivotAssist::new(self.tool_data.pivot.to_pivot_position())
					.on_update(|pivot_assist: &PivotAssist| SelectToolMessage::SetPivot { position: pivot_assist.position }.into())
					.widget_holder(),
			],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for SelectTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, false);

		if self.tool_data.pivot.should_refresh_pivot_position() {
			// Notify the frontend about the updated pivot position (a bit ugly to do it here not in the fsm but that doesn't have SelectTool)
			self.register_properties(responses, LayoutTarget::ToolOptions);
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	#[default]
	Ready,
	Dragging,
	DrawingBox,
	ResizingBounds,
	RotatingBounds,
	DraggingPivot,
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
	pivot: Pivot,
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
		(document, _document_id, _global_tool_data, input, render_data): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectToolFsmState::*;
		use SelectToolMessage::*;

		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, DocumentIsDirty | SelectionChanged) => {
					match (document.selected_visible_layers_bounding_box(render_data), tool_data.bounding_box_overlays.take()) {
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

					tool_data.path_outlines.update_selected(document.selected_visible_layers(), document, responses, render_data);
					tool_data.path_outlines.intersect_test_hovered(input, document, responses, render_data);
					tool_data.pivot.update_pivot(document, render_data, responses);

					self
				}
				(_, EditLayer) => {
					// On double click with select tool we sometimes want to edit the double clicked layers

					// Setup required data for checking the clicked layer
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					// Check the last (top most) intersection layer.
					if let Some(intersect_layer_path) = document.document_legacy.intersects_quad_root(quad, render_data).last() {
						if let Ok(intersect) = document.document_legacy.layer(intersect_layer_path) {
							match intersect.data {
								LayerDataType::Text(_) => {
									responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Text }.into());
									responses.push_back(TextToolMessage::Interact.into());
								}
								LayerDataType::Shape(_) => {
									responses.push_front(ToolMessage::ActivateTool { tool_type: ToolType::Path }.into());
								}
								LayerDataType::NodeGraphFrame(_) => {
									let replacement_selected_layers = vec![intersect_layer_path.clone()];
									let layer_path = intersect_layer_path.clone();
									responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());
									responses.push_back(NodeGraphMessage::OpenNodeGraph { layer_path }.into());
								}
								_ => {}
							}
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
							bounding_box.opposite_pivot = edges.calculate_pivot();
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
					let mut intersection = document.document_legacy.intersects_quad_root(quad, render_data);
					// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
					// If the user is dragging the rotate trigger, go into RotatingBounds mode.
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// If the user clicks on new shape, make that layer their new selection.
					// Otherwise enter the box select mode
					let state = if tool_data.pivot.is_over(input.mouse.position) {
						responses.push_back(DocumentMessage::StartTransaction.into());

						tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, None, render_data), true, true);
						tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

						DraggingPivot
					} else if let Some(selected_edges) = dragging_bounds {
						responses.push_back(DocumentMessage::StartTransaction.into());

						let snap_x = selected_edges.2 || selected_edges.3;
						let snap_y = selected_edges.0 || selected_edges.1;

						tool_data
							.snap_manager
							.start_snap(document, input, document.bounding_boxes(Some(&selected), None, render_data), snap_x, snap_y);
						tool_data
							.snap_manager
							.add_all_document_handles(document, input, &[], &selected.iter().map(|x| x.as_slice()).collect::<Vec<_>>(), &[]);

						tool_data.layers_dragging = selected;

						if let Some(bounds) = &mut tool_data.bounding_box_overlays {
							let document = &document.document_legacy;

							let selected = &tool_data.layers_dragging.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.center_of_transformation, selected, responses, document);
							bounds.center_of_transformation = selected.mean_average_of_pivots(render_data);
						}

						ResizingBounds
					} else if rotating_bounds {
						responses.push_back(DocumentMessage::StartTransaction.into());

						if let Some(bounds) = &mut tool_data.bounding_box_overlays {
							let selected = selected.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.center_of_transformation, &selected, responses, &document.document_legacy);

							bounds.center_of_transformation = selected.mean_average_of_pivots(render_data);
						}

						tool_data.layers_dragging = selected;

						RotatingBounds
					} else if intersection.last().map(|last| selected.contains(last)).unwrap_or(false) {
						responses.push_back(DocumentMessage::StartTransaction.into());

						tool_data.layers_dragging = selected;

						tool_data
							.snap_manager
							.start_snap(document, input, document.bounding_boxes(Some(&tool_data.layers_dragging), None, render_data), true, true);

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
								.start_snap(document, input, document.bounding_boxes(Some(&tool_data.layers_dragging), None, render_data), true, true);

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
						.filter_map(|path| document.document_legacy.viewport_bounding_box(path, render_data).ok()?)
						.flat_map(snapping::expand_bounds)
						.collect();

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

					if input.keyboard.get(duplicate as usize) && tool_data.not_duplicated_layers.is_none() {
						tool_data.start_duplicates(document, responses);
					} else if !input.keyboard.get(duplicate as usize) && tool_data.not_duplicated_layers.is_some() {
						tool_data.stop_duplicates(responses);
					}

					Dragging
				}
				(ResizingBounds, PointerMove { axis_align, center, .. }) => {
					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						if let Some(movement) = &mut bounds.selected_edges {
							let (center, axis_align) = (input.keyboard.get(center as usize), input.keyboard.get(axis_align as usize));

							let mouse_position = input.mouse.position;

							let snapped_mouse_position = tool_data.snap_manager.snap_position(responses, document, mouse_position);

							let (position, size) = movement.new_size(snapped_mouse_position, bounds.transform, center, bounds.center_of_transformation, axis_align);
							let (delta, mut pivot) = movement.bounds_to_scale_transform(position, size);

							let selected = &tool_data.layers_dragging.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut pivot, selected, responses, &document.document_legacy);

							selected.update_transforms(delta);
						}
					}
					ResizingBounds
				}
				(RotatingBounds, PointerMove { snap_angle, .. }) => {
					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						let angle = {
							let start_offset = tool_data.drag_start - bounds.center_of_transformation;
							let end_offset = input.mouse.position - bounds.center_of_transformation;

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
						let mut selected = Selected::new(&mut bounds.original_transforms, &mut bounds.center_of_transformation, &selected, responses, &document.document_legacy);

						selected.update_transforms(delta);
					}

					RotatingBounds
				}
				(DraggingPivot, PointerMove { .. }) => {
					let mouse_position = input.mouse.position;
					let snapped_mouse_position = tool_data.snap_manager.snap_position(responses, document, mouse_position);
					tool_data.pivot.set_viewport_position(snapped_mouse_position, document, render_data, responses);

					DraggingPivot
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
					let mut cursor = tool_data.bounding_box_overlays.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true));

					// Dragging the pivot overrules the other operations
					if tool_data.pivot.is_over(input.mouse.position) {
						cursor = MouseCursorIcon::Move;
					}

					// Generate the select outline (but not if the user is going to use the bound overlays)
					if cursor == MouseCursorIcon::Default {
						tool_data.path_outlines.intersect_test_hovered(input, document, responses, render_data);
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
					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.push_back(response.into());

					tool_data.snap_manager.cleanup(responses);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(RotatingBounds, DragStop) => {
					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.push_back(response.into());

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(DraggingPivot, DragStop) => {
					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.push_back(response.into());

					tool_data.snap_manager.cleanup(responses);

					Ready
				}
				(DrawingBox, DragStop) => {
					let quad = tool_data.selection_quad();
					responses.push_front(
						DocumentMessage::AddSelectedLayers {
							additional_layers: document.document_legacy.intersects_quad_root(quad, render_data),
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
					tool_data.pivot.clear_overlays(responses);

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
							&mut bounding_box_overlays.opposite_pivot,
							&selected,
							responses,
							&document.document_legacy,
						);

						selected.revert_operation();

						bounding_box_overlays.delete(responses);
					}

					tool_data.path_outlines.clear_hovered(responses);
					tool_data.path_outlines.clear_selected(responses);
					tool_data.pivot.clear_overlays(responses);

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
				(_, SetPivot { position }) => {
					responses.push_back(DocumentMessage::StartTransaction.into());

					let pos: Option<DVec2> = position.into();
					tool_data.pivot.set_normalized_position(pos.unwrap(), document, render_data, responses);

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
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
				HintGroup(vec![
					HintInfo::keys([Key::KeyG], "Grab Selected"),
					HintInfo::keys([Key::KeyR], "Rotate Selected"),
					HintInfo::keys([Key::KeyS], "Scale Selected"),
				]),
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Lmb, "Select Object"),
					HintInfo::keys([Key::Control], "Innermost").add_mac_keys([Key::Command]).prepend_plus(),
					HintInfo::keys([Key::Shift], "Grow/Shrink Selection").prepend_plus(),
				]),
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
					HintInfo::keys([Key::Shift], "Grow/Shrink Selection").prepend_plus(),
				]),
				HintGroup(vec![HintInfo::arrow_keys("Nudge Selected"), HintInfo::keys([Key::Shift], "Big Increment Nudge").prepend_plus()]),
				HintGroup(vec![
					HintInfo::keys([Key::Alt], "Move Duplicate"),
					HintInfo::keys([Key::Control, Key::KeyD], "Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
				]),
			]),
			SelectToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Shift], "Constrain to Axis"),
				HintInfo::keys([Key::Control], "Snap to Points (coming soon)"),
			])]),
			SelectToolFsmState::DrawingBox => HintData(vec![]),
			SelectToolFsmState::ResizingBounds => HintData(vec![]),
			SelectToolFsmState::RotatingBounds => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Control], "Snap 15°")])]),
			SelectToolFsmState::DraggingPivot => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

impl SelectToolData {
	/// Duplicates the currently dragging layers. Called when Alt is pressed and the layers have not yet been duplicated.
	fn start_duplicates(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::DeselectAllLayers.into());

		self.not_duplicated_layers = Some(self.layers_dragging.clone());

		// Duplicate each previously selected layer and select the new ones.
		for layer_path in Document::shallowest_unique_layers(self.layers_dragging.iter_mut()) {
			// Moves the original back to its starting position.
			responses.push_front(
				Operation::TransformLayerInViewport {
					path: layer_path.clone(),
					transform: DAffine2::from_translation(self.drag_start - self.drag_current).to_cols_array(),
				}
				.into(),
			);

			// Copy the layers.
			// Not using the Copy message allows us to retrieve the ids of the new layers to initialize the drag.
			let layer = match document.document_legacy.layer(layer_path) {
				Ok(layer) => layer.clone(),
				Err(e) => {
					warn!("Could not access selected layer {:?}: {:?}", layer_path, e);
					continue;
				}
			};

			let layer_metadata = *document.layer_metadata(layer_path);
			*layer_path.last_mut().unwrap() = generate_uuid();

			responses.push_back(
				Operation::InsertLayer {
					layer: Box::new(layer),
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

	/// Removes the duplicated layers. Called when Alt is released and the layers have been duplicated.
	fn stop_duplicates(&mut self, responses: &mut VecDeque<Message>) {
		let originals = match self.not_duplicated_layers.take() {
			Some(x) => x,
			None => return,
		};

		responses.push_back(DocumentMessage::DeselectAllLayers.into());

		// Delete the duplicated layers
		for layer_path in Document::shallowest_unique_layers(self.layers_dragging.iter()) {
			responses.push_back(Operation::DeleteLayer { path: layer_path.clone() }.into());
		}

		// Move the original to under the mouse
		for layer_path in Document::shallowest_unique_layers(originals.iter()) {
			responses.push_front(
				Operation::TransformLayerInViewport {
					path: layer_path.clone(),
					transform: DAffine2::from_translation(self.drag_current - self.drag_start).to_cols_array(),
				}
				.into(),
			);
		}

		// Select the originals
		responses.push_back(
			DocumentMessage::SetSelectedLayers {
				replacement_selected_layers: originals.clone(),
			}
			.into(),
		);

		self.layers_dragging = originals;
	}
}
