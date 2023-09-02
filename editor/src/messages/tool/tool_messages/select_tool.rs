#![allow(clippy::too_many_arguments)]
use crate::application::generate_uuid;
use crate::consts::{ROTATE_SNAP_ANGLE, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis};
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::path_outline::*;
use crate::messages::tool::common_functionality::pivot::Pivot;
use crate::messages::tool::common_functionality::snapping::{self, SnapManager};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::document::Document;
use document_legacy::intersection::Quad;
use document_legacy::layers::layer_info::{Layer, LayerDataType};
use document_legacy::LayerId;
use document_legacy::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Default)]
pub struct SelectTool {
	fsm_state: SelectToolFsmState,
	tool_data: SelectToolData,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct SelectOptions {
	nested_selection_behavior: NestedSelectionBehavior,
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum SelectOptionsUpdate {
	NestedSelectionBehavior(NestedSelectionBehavior),
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum NestedSelectionBehavior {
	#[default]
	Deepest,
	Shallowest,
}

impl fmt::Display for NestedSelectionBehavior {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			NestedSelectionBehavior::Deepest => write!(f, "Deep Select"),
			NestedSelectionBehavior::Shallowest => write!(f, "Shallow Select"),
		}
	}
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
		select_deepest: Key,
	},
	DragStop {
		remove_from_selection: Key,
	},
	EditLayer,
	Enter,
	FlipHorizontal,
	FlipVertical,
	PointerMove {
		axis_align: Key,
		snap_angle: Key,
		center: Key,
		duplicate: Key,
	},
	SelectOptions(SelectOptionsUpdate),
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

impl LayoutHolder for SelectTool {
	fn layout(&self) -> Layout {
		let layer_selection_behavior_entries = [NestedSelectionBehavior::Deepest, NestedSelectionBehavior::Shallowest]
			.iter()
			.map(|mode| {
				DropdownEntryData::new(mode.to_string())
					.value(mode.to_string())
					.on_update(move |_| SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(*mode)).into())
			})
			.collect();

		let selected_layers_count = self.tool_data.selected_layers_count;
		let deactivate_alignment = selected_layers_count < 2;
		let deactivate_boolean_ops = selected_layers_count < 2;
		let deactivate_flip = selected_layers_count < 1;
		let deactivate_pivot = selected_layers_count < 1;

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				DropdownInput::new(vec![layer_selection_behavior_entries])
					.selected_index(Some((self.tool_data.nested_selection_behavior == NestedSelectionBehavior::Shallowest) as u32))
					.tooltip("Choose if clicking nested layers directly selects the deepest, or selects the shallowest and deepens by double clicking")
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				// We'd like this widget to hide and show itself whenever the transformation cage is active or inactive (i.e. when no layers are selected)
				PivotAssist::new(self.tool_data.pivot.to_pivot_position())
					.disabled(deactivate_pivot)
					.on_update(|pivot_assist: &PivotAssist| SelectToolMessage::SetPivot { position: pivot_assist.position }.into())
					.widget_holder(),
				Separator::new(SeparatorType::Section).widget_holder(),
				IconButton::new("AlignLeft", 24)
					.tooltip("Align Left")
					.disabled(deactivate_alignment)
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
					.disabled(deactivate_alignment)
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
					.disabled(deactivate_alignment)
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::X,
							aggregate: AlignAggregate::Max,
						}
						.into()
					})
					.widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				IconButton::new("AlignTop", 24)
					.tooltip("Align Top")
					.disabled(deactivate_alignment)
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
					.disabled(deactivate_alignment)
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
					.disabled(deactivate_alignment)
					.on_update(|_| {
						DocumentMessage::AlignSelectedLayers {
							axis: AlignAxis::Y,
							aggregate: AlignAggregate::Max,
						}
						.into()
					})
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				PopoverButton::new("Align", "Coming soon").disabled(deactivate_alignment).widget_holder(),
				Separator::new(SeparatorType::Section).widget_holder(),
				IconButton::new("FlipHorizontal", 24)
					.tooltip("Flip Horizontal")
					.disabled(deactivate_flip)
					.on_update(|_| SelectToolMessage::FlipHorizontal.into())
					.widget_holder(),
				IconButton::new("FlipVertical", 24)
					.tooltip("Flip Vertical")
					.disabled(deactivate_flip)
					.on_update(|_| SelectToolMessage::FlipVertical.into())
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				PopoverButton::new("Flip", "Coming soon").disabled(deactivate_flip).widget_holder(),
				Separator::new(SeparatorType::Section).widget_holder(),
				IconButton::new("BooleanUnion", 24)
					.tooltip("Coming Soon: Boolean Union")
					.disabled(deactivate_boolean_ops)
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(1091) }.into())
					.widget_holder(),
				IconButton::new("BooleanSubtractFront", 24)
					.tooltip("Coming Soon: Boolean Subtract Front")
					.disabled(deactivate_boolean_ops)
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(1091) }.into())
					.widget_holder(),
				IconButton::new("BooleanSubtractBack", 24)
					.tooltip("Coming Soon: Boolean Subtract Back")
					.disabled(deactivate_boolean_ops)
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(1091) }.into())
					.widget_holder(),
				IconButton::new("BooleanIntersect", 24)
					.tooltip("Coming Soon: Boolean Intersect")
					.disabled(deactivate_boolean_ops)
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(1091) }.into())
					.widget_holder(),
				IconButton::new("BooleanDifference", 24)
					.tooltip("Coming Soon: Boolean Difference")
					.disabled(deactivate_boolean_ops)
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(1091) }.into())
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				PopoverButton::new("Boolean Operations", "Coming soon").disabled(deactivate_boolean_ops).widget_holder(),
			],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SelectTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Select(SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(nested_selection_behavior))) = message {
			self.tool_data.nested_selection_behavior = nested_selection_behavior;
			responses.add(ToolMessage::UpdateHints);
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, false);

		if self.tool_data.pivot.should_refresh_pivot_position() || self.tool_data.selected_layers_changed {
			// Notify the frontend about the updated pivot position (a bit ugly to do it here not in the fsm but that doesn't have SelectTool)
			self.send_layout(responses, LayoutTarget::ToolOptions);
			self.tool_data.selected_layers_changed = false;
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
				Enter,
			),
			_ => actions!(SelectToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
				EditLayer,
				Enter,
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
			..Default::default()
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
	layer_selected_on_start: Option<Vec<LayerId>>,
	is_dragging: bool,
	not_duplicated_layers: Option<Vec<Vec<LayerId>>>,
	drag_box_overlay_layer: Option<Vec<LayerId>>,
	path_outlines: PathOutline,
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	snap_manager: SnapManager,
	cursor: MouseCursorIcon,
	pivot: Pivot,
	nested_selection_behavior: NestedSelectionBehavior,
	selected_layers_count: usize,
	selected_layers_changed: bool,
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

	/// Duplicates the currently dragging layers. Called when Alt is pressed and the layers have not yet been duplicated.
	fn start_duplicates(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::DeselectAllLayers);

		// Take the selected layers and store them in a separate list.
		self.not_duplicated_layers = Some(self.layers_dragging.clone());

		// Duplicate each previously selected layer and select the new ones.
		for layer_path in Document::shallowest_unique_layers(self.layers_dragging.iter_mut()) {
			// Moves the original back to its starting position.
			responses.add_front(GraphOperationMessage::TransformChange {
				layer: layer_path.clone(),
				transform: DAffine2::from_translation(self.drag_start - self.drag_current),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

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

			responses.add(Operation::InsertLayer {
				layer: Box::new(layer),
				destination_path: layer_path.clone(),
				insert_index: -1,
				duplicating: false,
			});
			responses.add(DocumentMessage::UpdateLayerMetadata {
				layer_path: layer_path.clone(),
				layer_metadata,
			});
		}

		// Since the selected layers have now moved back to their original transforms before the drag began, we rerender them to be displayed as if they weren't touched.
		for layer_path in self.not_duplicated_layers.iter().flatten() {
			responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: layer_path.clone() });
		}
	}

	/// Removes the duplicated layers. Called when Alt is released and the layers have previously been duplicated.
	fn stop_duplicates(&mut self, responses: &mut VecDeque<Message>) {
		let originals = match self.not_duplicated_layers.take() {
			Some(x) => x,
			None => return,
		};

		responses.add(DocumentMessage::DeselectAllLayers);

		// Delete the duplicated layers
		for layer_path in Document::shallowest_unique_layers(self.layers_dragging.iter()) {
			responses.add(Operation::DeleteLayer { path: layer_path.clone() });
		}

		// Move the original to under the mouse
		for layer_path in Document::shallowest_unique_layers(originals.iter()) {
			responses.add_front(GraphOperationMessage::TransformChange {
				layer: layer_path.clone(),
				transform: DAffine2::from_translation(self.drag_current - self.drag_start),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});
		}

		// Select the originals
		responses.add(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: originals.clone(),
		});

		self.layers_dragging = originals;
	}
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData { document, input, render_data, .. }: &mut ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectToolFsmState::*;
		use SelectToolMessage::*;

		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, DocumentIsDirty | SelectionChanged) => {
					let selected_layers_count = document.selected_layers().count();
					let selected_layers_changed = selected_layers_count != tool_data.selected_layers_count;

					if selected_layers_changed {
						tool_data.selected_layers_count = selected_layers_count;
						tool_data.selected_layers_changed = true;
					} else {
						tool_data.selected_layers_changed = false;
					}

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
					// Setup required data for checking the clicked layer
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					// Check the last (topmost) intersection layer
					if let Some(intersect_layer_path) = document.document_legacy.intersects_quad_root(quad, render_data).last() {
						if let Ok(intersect) = document.document_legacy.layer(intersect_layer_path) {
							match tool_data.nested_selection_behavior {
								NestedSelectionBehavior::Shallowest => edit_layer_shallowest_manipulation(document, intersect_layer_path, tool_data, responses),
								NestedSelectionBehavior::Deepest => edit_layer_deepest_manipulation(intersect, responses),
							}
						}
					}

					self
				}
				(Ready, DragStart { add_to_selection, select_deepest }) => {
					tool_data.path_outlines.clear_hovered(responses);

					tool_data.drag_start = input.mouse.position;
					tool_data.drag_current = input.mouse.position;

					let dragging_bounds = tool_data.bounding_box_overlays.as_mut().and_then(|bounding_box| {
						let edges = bounding_box.check_selected_edges(input.mouse.position);

						bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
							let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
							bounding_box.opposite_pivot = selected_edges.calculate_pivot();
							selected_edges
						});

						edges
					});

					let rotating_bounds = tool_data
						.bounding_box_overlays
						.as_ref()
						.map(|bounding_box| bounding_box.check_rotate(input.mouse.position))
						.unwrap_or_default();

					let mut selected: Vec<_> = document.selected_visible_layers().map(|path| path.to_vec()).collect();
					let quad = tool_data.selection_quad();
					let mut intersection = document.document_legacy.intersects_quad_root(quad, render_data);

					// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
					// If the user is dragging the rotate trigger, go into RotatingBounds mode.
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// If the user clicks on new shape, make that layer their new selection.
					// Otherwise enter the box select mode
					let state = if tool_data.pivot.is_over(input.mouse.position) {
						responses.add(DocumentMessage::StartTransaction);

						tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, None, render_data), true, true);
						tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

						DraggingPivot
					} else if let Some(selected_edges) = dragging_bounds {
						responses.add(DocumentMessage::StartTransaction);

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
							let mut selected = Selected::new(
								&mut bounds.original_transforms,
								&mut bounds.center_of_transformation,
								selected,
								responses,
								document,
								None,
								&ToolType::Select,
							);
							bounds.center_of_transformation = selected.mean_average_of_pivots(render_data);
						}

						ResizingBounds
					} else if rotating_bounds {
						responses.add(DocumentMessage::StartTransaction);

						if let Some(bounds) = &mut tool_data.bounding_box_overlays {
							let selected = selected.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(
								&mut bounds.original_transforms,
								&mut bounds.center_of_transformation,
								&selected,
								responses,
								&document.document_legacy,
								None,
								&ToolType::Select,
							);

							bounds.center_of_transformation = selected.mean_average_of_pivots(render_data);
						}

						tool_data.layers_dragging = selected;

						RotatingBounds
					} else if intersection.last().map(|last| selected.iter().any(|selected_layer| last.starts_with(selected_layer))).unwrap_or(false)
						&& tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest
					{
						responses.add(DocumentMessage::StartTransaction);

						tool_data.layers_dragging = selected;

						tool_data
							.snap_manager
							.start_snap(document, input, document.bounding_boxes(Some(&tool_data.layers_dragging), None, render_data), true, true);

						Dragging
					} else {
						responses.add(DocumentMessage::StartTransaction);

						if !input.keyboard.get(add_to_selection as usize) && tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest {
							responses.add(DocumentMessage::DeselectAllLayers);
							tool_data.layers_dragging.clear();
						}

						if let Some(intersection) = intersection.pop() {
							tool_data.layer_selected_on_start = Some(intersection.clone());
							selected = vec![intersection.clone()];

							match tool_data.nested_selection_behavior {
								NestedSelectionBehavior::Shallowest => drag_shallowest_manipulation(document, selected, input, select_deepest, add_to_selection, tool_data, responses),
								NestedSelectionBehavior::Deepest => drag_deepest_manipulation(responses, selected, tool_data, document, input, render_data),
							}
							Dragging
						} else {
							// Deselect all layers if using shallowest selection behavior
							// Necessary since for shallowest mode, we need to know the current selected layers to determine the next
							if tool_data.nested_selection_behavior == NestedSelectionBehavior::Shallowest {
								responses.add(DocumentMessage::DeselectAllLayers);
								tool_data.layers_dragging.clear();
							}
							tool_data.drag_box_overlay_layer = Some(add_bounding_box(responses));
							DrawingBox
						}
					};
					tool_data.not_duplicated_layers = None;

					state
				}
				(Dragging, PointerMove { axis_align, duplicate, .. }) => {
					tool_data.is_dragging = true;
					// TODO: This is a cheat. Break out the relevant functionality from the handler above and call it from there and here.
					responses.add_front(SelectToolMessage::DocumentIsDirty);

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
						responses.add_front(GraphOperationMessage::TransformChange {
							layer: path.to_vec(),
							transform: DAffine2::from_translation(mouse_delta + closest_move),
							transform_in: TransformIn::Viewport,
							skip_rerender: true,
						});
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
							let (delta, mut _pivot) = movement.bounds_to_scale_transform(position, size);

							let selected = &tool_data.layers_dragging.iter().collect::<Vec<_>>();
							let mut selected = Selected::new(&mut bounds.original_transforms, &mut _pivot, selected, responses, &document.document_legacy, None, &ToolType::Select);

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
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&selected,
							responses,
							&document.document_legacy,
							None,
							&ToolType::Select,
						);

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

					responses.add_front(DocumentMessage::Overlays(
						Operation::SetLayerTransformInViewport {
							path: tool_data.drag_box_overlay_layer.clone().unwrap(),
							transform: transform_from_box(tool_data.drag_start, tool_data.drag_current, DAffine2::IDENTITY).to_cols_array(),
						}
						.into(),
					));
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
						responses.add(FrontendMessage::UpdateMouseCursor { cursor });
					}

					Ready
				}
				(Dragging, Enter) => {
					rerender_selected_layers(tool_data, responses);

					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					tool_data.snap_manager.cleanup(responses);
					responses.add_front(response);

					Ready
				}
				(Dragging, DragStop { remove_from_selection }) => {
					rerender_selected_layers(tool_data, responses);

					// Deselect layer if not snap dragging
					if !tool_data.is_dragging && input.keyboard.get(remove_from_selection as usize) && tool_data.layer_selected_on_start.is_none() {
						let quad = tool_data.selection_quad();
						let intersection = document.document_legacy.intersects_quad_root(quad, render_data);

						if let Some(path) = intersection.last() {
							let replacement_selected_layers: Vec<_> = document.selected_layers().filter(|&layer| !path.starts_with(layer)).map(|path| path.to_vec()).collect();

							tool_data.layers_dragging.clear();
							tool_data.layers_dragging.append(replacement_selected_layers.clone().as_mut());

							responses.add(DocumentMessage::SetSelectedLayers { replacement_selected_layers });
						}
					}

					tool_data.is_dragging = false;
					tool_data.layer_selected_on_start = None;

					responses.add(DocumentMessage::CommitTransaction);
					tool_data.snap_manager.cleanup(responses);

					Ready
				}
				(ResizingBounds, DragStop { .. } | Enter) => {
					rerender_selected_layers(tool_data, responses);

					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.add(response);

					tool_data.snap_manager.cleanup(responses);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(RotatingBounds, DragStop { .. } | Enter) => {
					rerender_selected_layers(tool_data, responses);

					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.add(response);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					Ready
				}
				(DraggingPivot, DragStop { .. } | Enter) => {
					let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
						true => DocumentMessage::Undo,
						false => DocumentMessage::CommitTransaction,
					};
					responses.add(response);

					tool_data.snap_manager.cleanup(responses);

					Ready
				}
				(DrawingBox, DragStop { .. } | Enter) => {
					let quad = tool_data.selection_quad();
					// For shallow select we don't update dragging layers until inside drag_start_shallowest_manipulation()
					tool_data.layers_dragging = document.document_legacy.intersects_quad_root(quad, render_data);
					responses.add_front(DocumentMessage::AddSelectedLayers {
						additional_layers: document.document_legacy.intersects_quad_root(quad, render_data),
					});
					responses.add_front(DocumentMessage::Overlays(
						Operation::DeleteLayer {
							path: tool_data.drag_box_overlay_layer.take().unwrap(),
						}
						.into(),
					));
					Ready
				}
				(Ready, Enter) => {
					let mut selected_layers = document.selected_layers();

					if let Some(layer_path) = selected_layers.next() {
						// Check that only one layer is selected
						if selected_layers.next().is_none() {
							if let Ok(layer) = document.document_legacy.layer(layer_path) {
								if let Ok(network) = layer.as_layer_network() {
									if network.nodes.values().any(|node| node.name == "Text") {
										responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
										responses.add(TextToolMessage::EditSelected);
									}
								}
							}
						}
					}

					Ready
				}
				(Dragging, Abort) => {
					rerender_selected_layers(tool_data, responses);

					tool_data.snap_manager.cleanup(responses);
					responses.add(DocumentMessage::Undo);

					tool_data.path_outlines.clear_selected(responses);
					tool_data.pivot.clear_overlays(responses);

					Ready
				}
				(_, Abort) => {
					if let Some(path) = tool_data.drag_box_overlay_layer.take() {
						responses.add_front(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()))
					};
					if let Some(mut bounding_box_overlays) = tool_data.bounding_box_overlays.take() {
						let selected = tool_data.layers_dragging.iter().collect::<Vec<_>>();
						let mut selected = Selected::new(
							&mut bounding_box_overlays.original_transforms,
							&mut bounding_box_overlays.opposite_pivot,
							&selected,
							responses,
							&document.document_legacy,
							None,
							&ToolType::Select,
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
					responses.add(DocumentMessage::AlignSelectedLayers { axis, aggregate });

					self
				}
				(_, FlipHorizontal) => {
					responses.add(DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::X });

					self
				}
				(_, FlipVertical) => {
					responses.add(DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::Y });

					self
				}
				(_, SetPivot { position }) => {
					responses.add(DocumentMessage::StartTransaction);

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

	fn standard_tool_messages(&self, message: &ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut Self::ToolData) -> bool {
		// Check for standard hits or cursor events
		match message {
			ToolMessage::UpdateHints => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
					HintGroup(vec![HintInfo::keys([Key::KeyG, Key::KeyR, Key::KeyS], "Grab/Rotate/Scale Selected")]),
					HintGroup({
						let mut hints = vec![HintInfo::mouse(MouseMotion::Lmb, "Select Object"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()];
						if tool_data.nested_selection_behavior == NestedSelectionBehavior::Shallowest {
							hints.extend([HintInfo::keys([Key::Accel], "Deepest").prepend_plus(), HintInfo::mouse(MouseMotion::LmbDouble, "Deepen Selection")]);
						}
						hints
					}),
					HintGroup(vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
						HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus(),
					]),
					HintGroup(vec![
						HintInfo::arrow_keys("Nudge Selected"),
						HintInfo::keys([Key::Shift], "10x").prepend_plus(),
						HintInfo::keys([Key::Alt], "Resize Corner").prepend_plus(),
						HintInfo::keys([Key::Control], "Opp. Corner").prepend_plus(),
					]),
					HintGroup(vec![
						HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Move Duplicate"),
						HintInfo::keys([Key::Control, Key::KeyD], "Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
					]),
				]);

				responses.add(FrontendMessage::UpdateInputHints { hint_data });
				self.update_hints(responses);
				true
			}
			ToolMessage::UpdateCursor => {
				self.update_cursor(responses);
				true
			}
			_ => false,
		}
	}

	fn update_hints(&self, _responses: &mut VecDeque<Message>) {}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn rerender_selected_layers(tool_data: &mut SelectToolData, responses: &mut VecDeque<Message>) {
	for layer_path in &tool_data.layers_dragging {
		responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: layer_path.clone() });
	}
}

// TODO: Majorly clean up these next five functions

fn drag_shallowest_manipulation(
	document: &DocumentMessageHandler,
	selected: Vec<Vec<u64>>,
	input: &InputPreprocessorMessageHandler,
	select_deepest: Key,
	add_to_selection: Key,
	tool_data: &mut SelectToolData,
	responses: &mut VecDeque<Message>,
) {
	let selected_layers = document.selected_layers();
	let selected_layers_collected: Vec<_> = document.selected_layers().collect();
	let selected_layers_count = document.selected_layers().count();

	let empty_vector: &Vec<u64> = &vec![];
	let incoming_layer_path_vector: &Vec<u64> = selected.get(0).unwrap_or(empty_vector);

	let incoming_parent = *incoming_layer_path_vector.first().unwrap_or(&0);
	let previous_parents: Vec<_> = (0..selected_layers_collected.len())
		.map(|i| &selected_layers_collected.get(i).unwrap_or(&empty_vector.as_slice())[..1])
		.collect();

	// Permutations of intersected layer
	let intersected_layer_ancestors: Vec<Vec<u64>> = (1..=incoming_layer_path_vector.len()).map(|i| incoming_layer_path_vector[..i].to_vec()).rev().collect();

	// Accel+Shift click adds the deepest layer to the selection
	if input.keyboard.get(select_deepest as usize) && input.keyboard.get(add_to_selection as usize) {
		// Collect the layers unrelated to the incoming layer path's parent
		let mut layers_without_incoming_parent: Vec<Vec<u64>> = document
			.selected_layers()
			.filter(|&layer| layer.first().unwrap_or(&0) != &incoming_parent)
			.map(|path| path.to_vec())
			.collect();
		// If the incoming layer is unrelated, add shape to selected layers
		if !previous_parents.contains(&[incoming_parent].as_slice()) {
			layers_without_incoming_parent.push(incoming_layer_path_vector.to_vec());
			tool_data.layers_dragging = layers_without_incoming_parent.to_vec();
			responses.add(DocumentMessage::SetSelectedLayers {
				replacement_selected_layers: layers_without_incoming_parent.to_vec(),
			});
		}
		// If the incoming shape is related, calculate the new layer tree
		else {
			// Calculating the next selected layers depends on how many layers are currently selected
			if selected_layers_count <= 1 {
				let replace = &selected_layers
					.into_iter()
					.any(|selected_layer| intersected_layer_ancestors.iter().any(|ancestor| selected_layer == ancestor));
				if *replace {
					// If the incoming layer path is already selected, deselect
					if vec![incoming_layer_path_vector] == selected_layers_collected {
						tool_data.layer_selected_on_start = None;
					} else {
						layers_without_incoming_parent.push(incoming_layer_path_vector.to_vec());
						tool_data.layers_dragging = layers_without_incoming_parent.to_vec();
						responses.add(DocumentMessage::SetSelectedLayers {
							replacement_selected_layers: layers_without_incoming_parent.to_vec(),
						});
					}
				} else {
					layers_without_incoming_parent.push(incoming_layer_path_vector.to_vec());
					tool_data.layers_dragging.append(layers_without_incoming_parent.to_vec().as_mut());
					responses.add(DocumentMessage::AddSelectedLayers {
						additional_layers: layers_without_incoming_parent.to_vec(),
					});
				}
			}
			// Multiple layers are selected
			else {
				// If the incoming layer path is already selected, deselect
				if selected_layers_collected.contains(&incoming_layer_path_vector.as_slice()) {
					tool_data.layer_selected_on_start = None;
				} else {
					layers_without_incoming_parent = document
						.selected_layers()
						.filter(|&layer| !intersected_layer_ancestors.contains(&(*layer).to_vec()))
						.map(|path| path.to_vec())
						.collect();
					layers_without_incoming_parent.push(incoming_layer_path_vector.to_vec());
					tool_data.layers_dragging = layers_without_incoming_parent.to_vec();
					responses.add(DocumentMessage::SetSelectedLayers {
						replacement_selected_layers: layers_without_incoming_parent.to_vec(),
					});
				}
			}
		}

		tool_data.layers_dragging.append(selected.clone().as_mut());
		responses.add(DocumentMessage::AddSelectedLayers { additional_layers: selected.clone() });
	}
	// Accel click selects the deepest layer directly
	else if input.keyboard.get(select_deepest as usize) {
		tool_data.layers_dragging.clear();
		tool_data.layers_dragging = selected.to_vec();
		responses.add(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: selected.to_vec(),
		});
	}
	// Check whether a layer is selected for next selection calculations
	else if !selected_layers_collected.is_empty() {
		// Check if the intersected layer path is already selected
		let already_selected_parent = previous_parents.contains(&[incoming_parent].as_slice());
		let mut search = vec![];
		let mut recursive_found = false;

		// Only need to calculate if the incoming layer shares a parent with the selected layer
		if already_selected_parent {
			// Use the combined layers of currently selected and incoming and get shallowest common folder
			let mut combined_layers = selected_layers_collected.to_vec();
			if !combined_layers.contains(&incoming_layer_path_vector.as_slice()) && !incoming_layer_path_vector.is_empty() {
				combined_layers.push(incoming_layer_path_vector);
			}
			// Shared shallowest common folder of the combined layers
			let shallowest_common_folder = document.document_legacy.shallowest_common_folder(combined_layers.iter().copied()).unwrap_or_default().to_vec();
			let mut selected_layer_path_parent = shallowest_common_folder.to_vec();

			// Determine if the incoming layer path is already selected
			let already_selected = selected_layers
				.filter(|layer| incoming_layer_path_vector.len() >= layer.len())
				.any(|layer| &incoming_layer_path_vector[..layer.len()] == layer);

			// Recursively search through each children of the selected layer path parent, if not found move up a layer
			while !selected_layer_path_parent.is_empty() && !recursive_found {
				let selected_children_layer_paths = document.document_legacy.folder_children_paths(&selected_layer_path_parent);
				for child in selected_children_layer_paths {
					if child == *incoming_layer_path_vector {
						search = child;
						recursive_found = true;
						break;
					} else if document.document_legacy.is_folder(&child) {
						recursive_found = recursive_search(document, &child, incoming_layer_path_vector);
						if recursive_found {
							search = child;
							break;
						}
					}
				}
				selected_layer_path_parent = selected_layer_path_parent[..selected_layer_path_parent.len() - 1].to_vec();
			}

			// Check if new layer is already selected
			let already_selected_search = selected_layers_collected.contains(&search.to_vec().as_slice());

			// One layer is currently selected
			if selected_layers_count <= 1 {
				// If the shape has no parent and is not a folder use the incoming layer path
				if search.is_empty() {
					search = incoming_layer_path_vector.to_vec();
				}

				if input.keyboard.get(add_to_selection as usize) {
					if !already_selected {
						tool_data.layers_dragging.push(search.to_vec());
						responses.add(DocumentMessage::AddSelectedLayers { additional_layers: vec![search] });
					}
					// Deselect already selected layers
					else {
						tool_data.layer_selected_on_start = None;
					}
				} else {
					// Only reselect the shape if the shape is not already selected, otherwise do nothing
					if !already_selected {
						tool_data.layers_dragging.clear();
						tool_data.layers_dragging.push(search.to_vec());
						responses.add(DocumentMessage::SetSelectedLayers {
							replacement_selected_layers: vec![search],
						});
					}
				}
			} else if selected_layers_count > 1 {
				let direct_child = incoming_layer_path_vector
					.iter()
					.copied()
					.filter(|path| !shallowest_common_folder.contains(path))
					.take(1)
					.collect::<Vec<_>>();
				let already_selected_direct_child = selected_layers_collected.contains(&direct_child.clone().as_slice());

				// Update layer tree by filtering any duplicate layers (e.g. If a parent and one of its children are selected)
				let mut replacement_selected_layers: Vec<Vec<u64>> = Vec::new();
				let mut replacement_selected_layers_before_filter = selected_layers_collected.to_vec();

				// Only use the recursive search layer if a common folder between incoming layer and already selected layers exists
				// If not use the rootmost parent's layer path of the incoming layer (direct_child)
				if !search.is_empty() & !replacement_selected_layers_before_filter.contains(&search.as_slice()) {
					replacement_selected_layers_before_filter.push(&search);
				} else if search.is_empty() && !replacement_selected_layers_before_filter.contains(&direct_child.as_slice()) {
					replacement_selected_layers_before_filter.push(&direct_child);
				}

				// Using the shallowest common folder filter out any already selected layer that is related to the incoming selection
				replacement_selected_layers_before_filter
					.into_iter()
					.filter(|layer| {
						if !search.is_empty() {
							layer.len() >= search.len() && layer[..search.len()] != search
						} else {
							layer.len() >= direct_child.len() && layer[..direct_child.len()] != direct_child
						}
					})
					.for_each(|layer| replacement_selected_layers.push(layer.to_vec()));
				// After filtering any layer that is related to the incoming selection, reappend the incoming selection
				if !search.is_empty() && !replacement_selected_layers.contains(&search) {
					replacement_selected_layers.push(search.to_vec());
				} else if search.is_empty() && !replacement_selected_layers.contains(&direct_child) {
					replacement_selected_layers.push(direct_child.to_vec());
				}

				let replacement_selected_layers_response: Vec<Vec<u64>> = replacement_selected_layers.to_vec().iter().map(|slice| slice.to_vec()).collect();

				// Shift-Click: Append or Deselect layers
				if input.keyboard.get(add_to_selection as usize) {
					if search.is_empty() {
						// Deselect already selected layer
						if already_selected_direct_child {
							tool_data.layer_selected_on_start = None;
						} else {
							tool_data.layers_dragging.clear();
							tool_data.layers_dragging = replacement_selected_layers_response.to_vec();
							responses.add(DocumentMessage::SetSelectedLayers {
								replacement_selected_layers: replacement_selected_layers_response,
							});
						}
					} else {
						// Deselect already selected layer
						if already_selected_search {
							tool_data.layer_selected_on_start = None;
						} else {
							tool_data.layers_dragging.clear();
							tool_data.layers_dragging = replacement_selected_layers_response.to_vec();
							responses.add(DocumentMessage::SetSelectedLayers {
								replacement_selected_layers: replacement_selected_layers_response,
							});
						}
					}
				}
				// Normal Click: Selecting new layers
				else if !already_selected {
					if !search.is_empty() {
						tool_data.layers_dragging.clear();
						tool_data.layers_dragging = vec![search.to_vec()];
						responses.add(DocumentMessage::SetSelectedLayers {
							replacement_selected_layers: vec![search],
						});
					} else {
						tool_data.layers_dragging.clear();
						tool_data.layers_dragging = vec![direct_child.to_vec()];
						responses.add(DocumentMessage::SetSelectedLayers {
							replacement_selected_layers: vec![direct_child],
						});
					}
				}
			}
		}
		// Incoming layer path has different parent, set selected layer to shallowest parent
		else {
			if input.keyboard.get(add_to_selection as usize) {
				responses.add(DocumentMessage::AddSelectedLayers {
					additional_layers: vec![vec![incoming_parent]],
				});
			} else {
				tool_data.layers_dragging.clear();
				responses.add(DocumentMessage::SetSelectedLayers {
					replacement_selected_layers: vec![vec![incoming_parent]],
				});
			}
			tool_data.layers_dragging.push(vec![incoming_parent]);
		}
	} else {
		// Check if new layer is already selected
		tool_data.layers_dragging.push(vec![incoming_parent]);
		responses.add(DocumentMessage::AddSelectedLayers {
			additional_layers: vec![vec![incoming_parent]],
		});
	}
}

fn drag_deepest_manipulation(
	responses: &mut VecDeque<Message>,
	mut selected: Vec<Vec<u64>>,
	tool_data: &mut SelectToolData,
	document: &DocumentMessageHandler,
	input: &InputPreprocessorMessageHandler,
	render_data: &document_legacy::layers::RenderData,
) {
	responses.add(DocumentMessage::AddSelectedLayers { additional_layers: selected.clone() });
	tool_data.layers_dragging.append(selected.as_mut());
	tool_data
		.snap_manager
		.start_snap(document, input, document.bounding_boxes(Some(&tool_data.layers_dragging), None, render_data), true, true);
}

fn edit_layer_shallowest_manipulation(document: &DocumentMessageHandler, intersect_layer_path: &Vec<u64>, tool_data: &mut SelectToolData, responses: &mut VecDeque<Message>) {
	// Double-clicking any layer within an already selected folder should select that layer
	// Add the first layer path not already included from the intersected to our new layer path
	let selected_layers_collected: Vec<_> = document.selected_layers().collect();

	let empty_vector: &Vec<u64> = &vec![];
	let incoming_parent = *intersect_layer_path.first().unwrap_or(&0);
	let previous_parents: Vec<_> = (0..selected_layers_collected.len())
		.map(|i| &selected_layers_collected.get(i).unwrap_or(&empty_vector.as_slice())[..1])
		.collect();
	let incoming_parent_selected = previous_parents.contains(&[incoming_parent].as_slice());

	if incoming_parent_selected {
		// Permutations of intersected layer
		// Set the base layer path to the deepest layer that is currently selected
		let intersected_layer_ancestors: Vec<Vec<u64>> = (1..=intersect_layer_path.len()).map(|i| intersect_layer_path[..i].to_vec()).rev().collect();
		let mut new_layer_path: Vec<u64> = intersected_layer_ancestors
			.iter()
			.filter_map(|permutation| selected_layers_collected.iter().find(|layer| **layer == *permutation).map(|_| permutation.to_vec()))
			.flatten()
			.collect();

		// Append the sub layer to the base to create the deeper layer path
		for path in intersect_layer_path {
			if !new_layer_path.contains(path) {
				new_layer_path.push(*path);
				break;
			}
		}

		tool_data.layers_dragging.clear();
		tool_data.layers_dragging.push(new_layer_path.to_vec());
		responses.add(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: vec![new_layer_path.to_vec()],
		});

		if selected_layers_collected.contains(&new_layer_path.as_slice()) {
			responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Path });
		}
	}
}

fn edit_layer_deepest_manipulation(intersect: &Layer, responses: &mut VecDeque<Message>) {
	match &intersect.data {
		LayerDataType::Shape(_) => {
			responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Path });
		}
		LayerDataType::Layer(layer) if layer.as_vector_data().is_some() => {
			if layer.network.nodes.values().any(|node| node.name == "Text") {
				responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
				responses.add(TextToolMessage::EditSelected);
			} else {
				responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Path });
			}
		}
		_ => {}
	}
}

#[allow(clippy::if_same_then_else)]
fn recursive_search(document: &DocumentMessageHandler, layer_path: &[u64], incoming_layer_path_vector: &Vec<u64>) -> bool {
	let layer_paths = document.document_legacy.folder_children_paths(layer_path);
	for path in layer_paths {
		if path == *incoming_layer_path_vector || (document.document_legacy.is_folder(&path) && recursive_search(document, &path, incoming_layer_path_vector)) {
			return true;
		}
	}
	false
}
