use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::{ColorInput, WidgetHolder};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;

use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use bezier_rs::Subpath;
use document_legacy::LayerId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeInput;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::{ManipulatorPointId, SelectedType};
use graphene_core::Color;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct PenTool {
	fsm_state: PenToolFsmState,
	tool_data: PenToolData,
	options: PenOptions,
}

pub struct PenOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self {
			line_weight: 5.,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum PenToolMessage {
	// Standard messages
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	SelectionChanged,
	#[remain::unsorted]
	WorkingColorChanged,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove {
		snap_angle: Key,
		break_handle: Key,
		lock_angle: Key,
	},
	Undo,
	UpdateOptions(PenOptionsUpdate),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PenToolFsmState {
	#[default]
	Ready,
	DraggingHandle,
	PlacingAnchor,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum PenOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for PenTool {
	fn icon_name(&self) -> String {
		"VectorPenTool".into()
	}
	fn tooltip(&self) -> String {
		"Pen Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Pen
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.on_update(|number_input: &NumberInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl PropertyHolder for PenTool {
	fn properties(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			WidgetCallback::new(|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(color.value)).into()),
		);

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			WidgetCallback::new(|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(color.value)).into()),
		));
		widgets.push(WidgetHolder::unrelated_separator());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PenTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = message {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
				PenOptionsUpdate::FillColor(color) => {
					self.options.fill.custom_color = color;
					self.options.fill.color_type = ToolColorType::Custom;
				}
				PenOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
				PenOptionsUpdate::StrokeColor(color) => {
					self.options.stroke.custom_color = color;
					self.options.stroke.color_type = ToolColorType::Custom;
				}
				PenOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
				PenOptionsUpdate::WorkingColors(primary, secondary) => {
					self.options.stroke.primary_working_color = primary;
					self.options.stroke.secondary_working_color = secondary;
					self.options.fill.primary_working_color = primary;
					self.options.fill.secondary_working_color = secondary;
				}
			}

			responses.add(LayoutMessage::SendLayout {
				layout: self.properties(),
				layout_target: LayoutTarget::ToolOptions,
			});

			return;
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PenToolFsmState::Ready => actions!(PenToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				Confirm,
				Abort,
			),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => actions!(PenToolMessageDiscriminant;
				DragStart,
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
		}
	}
}

impl ToolTransition for PenTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(PenToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(PenToolMessage::Abort.into()),
			selection_changed: Some(PenToolMessage::SelectionChanged.into()),
			working_color_changed: Some(PenToolMessage::WorkingColorChanged.into()),
		}
	}
}
struct ModifierState {
	snap_angle: bool,
	lock_angle: bool,
	break_handle: bool,
}
#[derive(Clone, Debug, Default)]
struct PenToolData {
	weight: f64,
	path: Option<Vec<LayerId>>,
	subpath_index: usize,
	snap_manager: SnapManager,
	should_mirror: bool,
	// Indicates that curve extension is occurring from the first point, rather than (more commonly) the last point
	from_start: bool,
	angle: f64,
}
impl PenToolData {
	fn extend_subpath(&mut self, layer: &[LayerId], subpath_index: usize, from_start: bool, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.path = Some(layer.to_vec());
		self.from_start = from_start;
		self.subpath_index = subpath_index;

		// Stop the handles on the first point from mirroring
		let Some(subpaths) = get_subpaths(layer, document) else { return };
		let manipulator_groups = subpaths[subpath_index].manipulator_groups();
		let Some(last_handle) = (if from_start { manipulator_groups.first() } else { manipulator_groups.last() }) else { return };

		responses.add(GraphOperationMessage::Vector {
			layer: layer.to_vec(),
			modification: VectorDataModification::SetManipulatorHandleMirroring {
				id: last_handle.id,
				mirror_angle: false,
			},
		});
	}

	fn create_new_path(
		&mut self,
		document: &DocumentMessageHandler,
		line_weight: f64,
		stroke_color: Option<Color>,
		fill_color: Option<Color>,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) {
		// Deselect layers because we are now creating a new layer
		responses.add(DocumentMessage::DeselectAllLayers);

		let layer_path = document.get_path_for_new_layer();

		// Get the position and set properties
		let transform = document.document_legacy.multiply_transforms(&layer_path[..layer_path.len() - 1]).unwrap_or_default();
		let snapped_position = self.snap_manager.snap_position(responses, document, input.mouse.position);
		let start_position = transform.inverse().transform_point2(snapped_position);
		self.weight = line_weight;

		// Create the initial shape with a `bez_path` (only contains a moveto initially)
		let subpath = bezier_rs::Subpath::new(vec![bezier_rs::ManipulatorGroup::new(start_position, Some(start_position), Some(start_position))], false);
		graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);

		responses.add(GraphOperationMessage::FillSet {
			layer: layer_path.clone(),
			fill: if fill_color.is_some() { Fill::Solid(fill_color.unwrap()) } else { Fill::None },
		});

		responses.add(GraphOperationMessage::StrokeSet {
			layer: layer_path.clone(),
			stroke: Stroke::new(stroke_color, line_weight),
		});

		self.path = Some(layer_path);
		self.from_start = false;
		self.subpath_index = 0;
	}

	// TODO: tooltip / user documentation?
	/// If you place the anchor on top of the previous anchor then you break the mirror
	fn check_break(&mut self, document: &DocumentMessageHandler, transform: DAffine2, shape_overlay: &mut OverlayRenderer, responses: &mut VecDeque<Message>) -> Option<()> {
		// Get subpath
		let layer_path = self.path.as_ref()?;
		let subpath = &get_subpaths(layer_path, document)?[self.subpath_index];

		// Get the last manipulator group and the one previous to that
		let mut manipulator_groups = subpath.manipulator_groups().iter();
		let last_manipulator_group = if self.from_start { manipulator_groups.next()? } else { manipulator_groups.next_back()? };
		let previous_manipulator_group = if self.from_start { manipulator_groups.next()? } else { manipulator_groups.next_back()? };

		// Get correct handle types
		let outwards_handle = if self.from_start { SelectedType::InHandle } else { SelectedType::OutHandle };

		// Get manipulator points
		let last_anchor = last_manipulator_group.anchor;
		let previous_anchor = previous_manipulator_group.anchor;

		// Break the control
		let on_top = transform.transform_point2(last_anchor).distance_squared(transform.transform_point2(previous_anchor)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		if !on_top {
			return None;
		}
		// Remove the point that has just been placed
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_vec(),
			modification: VectorDataModification::RemoveManipulatorGroup { id: last_manipulator_group.id },
		});

		// Move the in handle of the previous anchor to on top of the previous position
		let point = ManipulatorPointId::new(previous_manipulator_group.id, outwards_handle);
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_vec(),
			modification: VectorDataModification::SetManipulatorPosition { point, position: previous_anchor },
		});

		// Stop the handles on the last point from mirroring
		let id = previous_manipulator_group.id;
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_vec(),
			modification: VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle: false },
		});

		// The overlay system cannot detect deleted points so we must just delete all the overlays
		for layer_path in document.all_layers() {
			shape_overlay.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
		}

		self.should_mirror = false;
		None
	}

	fn finish_placing_handle(&mut self, document: &DocumentMessageHandler, transform: DAffine2, shape_overlay: &mut OverlayRenderer, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		// Get subpath
		let layer_path = self.path.as_ref()?;
		let subpath = &get_subpaths(layer_path, document)?[self.subpath_index];

		// Get the last manipulator group and the one previous to that
		let mut manipulator_groups = subpath.manipulator_groups().iter();
		let last_manipulator_group = if self.from_start { manipulator_groups.next()? } else { manipulator_groups.next_back()? };
		let previous_manipulator_group = if self.from_start { manipulator_groups.next() } else { manipulator_groups.next_back() };

		// Get the first manipulator group
		let first_manipulator_group = if self.from_start {
			subpath.manipulator_groups().last()?
		} else {
			subpath.manipulator_groups().first()?
		};

		// Get correct handle types
		let inwards_handle = if self.from_start { SelectedType::OutHandle } else { SelectedType::InHandle };
		let outwards_handle = if self.from_start { SelectedType::InHandle } else { SelectedType::OutHandle };

		// Get manipulator points
		let last_anchor = last_manipulator_group.anchor;
		let first_anchor = first_manipulator_group.anchor;
		let last_in = inwards_handle.get_position(last_manipulator_group)?;

		let transformed_distance_between_squared = transform.transform_point2(last_anchor).distance_squared(transform.transform_point2(first_anchor));
		let snap_point_tolerance_squared = crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		let should_close_path = transformed_distance_between_squared < snap_point_tolerance_squared && previous_manipulator_group.is_some();
		if should_close_path {
			// Move the in handle of the first point to where the user has placed it
			let point = ManipulatorPointId::new(first_manipulator_group.id, inwards_handle);
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::SetManipulatorPosition { point, position: last_in },
			});

			// Stop the handles on the first point from mirroring
			let id = first_manipulator_group.id;
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle: false },
			});

			// Remove the point that has just been placed
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::RemoveManipulatorGroup { id: last_manipulator_group.id },
			});

			// Push a close path node
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::SetClosed { index: 0, closed: true },
			});

			responses.add(DocumentMessage::CommitTransaction);

			// Clean up overlays
			for layer_path in document.all_layers() {
				shape_overlay.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
			}

			// Clean up tool data
			self.path = None;
			self.snap_manager.cleanup(responses);

			// Return to ready state
			return Some(PenToolFsmState::Ready);
		}
		// Add a new manipulator for the next anchor that we will place
		if let Some(out_handle) = outwards_handle.get_position(last_manipulator_group) {
			responses.add(add_manipulator_group(&self.path, self.from_start, bezier_rs::ManipulatorGroup::new_anchor(out_handle)));
		}

		Some(PenToolFsmState::PlacingAnchor)
	}

	fn drag_handle(&mut self, document: &DocumentMessageHandler, transform: DAffine2, mouse: DVec2, modifiers: ModifierState, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		// Get subpath
		let layer_path = self.path.as_ref()?;
		let subpath = &get_subpaths(layer_path, document)?[self.subpath_index];

		// Get the last manipulator group
		let manipulator_groups = subpath.manipulator_groups();
		let last_manipulator_group = if self.from_start { manipulator_groups.first()? } else { manipulator_groups.last()? };

		// Get correct handle types
		let inwards_handle = if self.from_start { SelectedType::OutHandle } else { SelectedType::InHandle };
		let outwards_handle = if self.from_start { SelectedType::InHandle } else { SelectedType::OutHandle };

		// Get manipulator points
		let last_anchor = last_manipulator_group.anchor;

		let mouse = self.snap_manager.snap_position(responses, document, mouse);
		let pos = transform.inverse().transform_point2(mouse);

		let pos = compute_snapped_angle(&mut self.angle, modifiers.lock_angle, modifiers.snap_angle, pos, last_anchor);
		if !pos.is_finite() {
			return Some(PenToolFsmState::DraggingHandle);
		}

		// Update points on current segment (to show preview of new handle)
		let point = ManipulatorPointId::new(last_manipulator_group.id, outwards_handle);
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_vec(),
			modification: VectorDataModification::SetManipulatorPosition { point, position: pos },
		});

		let should_mirror = !modifiers.break_handle && self.should_mirror;
		// Mirror handle of last segment
		if should_mirror {
			// Could also be written as `last_anchor.position * 2 - pos` but this way avoids overflow/underflow better
			let pos = last_anchor - (pos - last_anchor);
			let point = ManipulatorPointId::new(last_manipulator_group.id, inwards_handle);
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::SetManipulatorPosition { point, position: pos },
			});
		}

		// Update the mirror status of the currently modifying point
		let id = last_manipulator_group.id;
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_vec(),
			modification: VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle: should_mirror },
		});

		Some(PenToolFsmState::DraggingHandle)
	}

	fn place_anchor(&mut self, document: &DocumentMessageHandler, transform: DAffine2, mouse: DVec2, modifiers: ModifierState, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		// Get subpath
		let layer_path = self.path.as_ref()?;
		let subpath = &get_subpaths(layer_path, document)?[self.subpath_index];

		// Get the last manipulator group and the one previous to that
		let mut manipulator_groups = subpath.manipulator_groups().iter();
		let last_manipulator_group = if self.from_start { manipulator_groups.next()? } else { manipulator_groups.next_back()? };
		let previous_manipulator_group = if self.from_start { manipulator_groups.next() } else { manipulator_groups.next_back() };

		// Get the first manipulator group
		let manipulator_groups = subpath.manipulator_groups();
		let first_manipulator_group = if self.from_start { manipulator_groups.last()? } else { manipulator_groups.first()? };

		// Get manipulator points
		let first_anchor = first_manipulator_group.anchor;

		let mouse = self.snap_manager.snap_position(responses, document, mouse);
		let mut pos = transform.inverse().transform_point2(mouse);

		// Snap to the first point (to show close path)
		let show_close_path = mouse.distance_squared(transform.transform_point2(first_anchor)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		if show_close_path {
			pos = first_anchor;
		}

		if let Some(relative_previous_anchor) = previous_manipulator_group.map(|group| group.anchor) {
			// Snap to the previously placed point (to show break control)
			if mouse.distance_squared(transform.transform_point2(relative_previous_anchor)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2) {
				pos = relative_previous_anchor;
			} else {
				pos = compute_snapped_angle(&mut self.angle, modifiers.lock_angle, modifiers.snap_angle, pos, relative_previous_anchor);
			}
		}

		for manipulator_type in [SelectedType::Anchor, SelectedType::InHandle, SelectedType::OutHandle] {
			let point = ManipulatorPointId::new(last_manipulator_group.id, manipulator_type);
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::SetManipulatorPosition { point, position: pos },
			});
		}
		Some(PenToolFsmState::PlacingAnchor)
	}

	fn finish_transaction(&mut self, fsm: PenToolFsmState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Option<DocumentMessage> {
		// Get subpath
		let layer_path = self.path.as_ref()?;
		let subpath = &get_subpaths(layer_path, document)?[self.subpath_index];

		// Abort if only one manipulator group has been placed
		if fsm == PenToolFsmState::PlacingAnchor && subpath.len() < 3 {
			return None;
		}

		// Get the last manipulator group and the one previous to that
		let mut manipulator_groups = subpath.manipulator_groups().iter();
		let mut last_manipulator_group = if self.from_start { manipulator_groups.next()? } else { manipulator_groups.next_back()? };
		let previous_manipulator_group = if self.from_start { manipulator_groups.next() } else { manipulator_groups.next_back() };

		// Get correct handle types
		let outwards_handle = if self.from_start { SelectedType::InHandle } else { SelectedType::OutHandle };

		// If placing anchor we should abort if there are less than three manipulators (as the last one gets deleted)
		let Some(previous_manipulator_group) = previous_manipulator_group else {
			return Some(DocumentMessage::AbortTransaction);
		};

		// Clean up if there are two or more manipulators
		// Remove the unplaced anchor if in anchor placing mode
		if fsm == PenToolFsmState::PlacingAnchor {
			let layer_path = layer_path.clone();
			responses.add(GraphOperationMessage::Vector {
				layer: layer_path.to_vec(),
				modification: VectorDataModification::RemoveManipulatorGroup { id: last_manipulator_group.id },
			});
			last_manipulator_group = previous_manipulator_group;
		}

		// Remove the out handle
		let point = ManipulatorPointId::new(last_manipulator_group.id, outwards_handle);
		let position = last_manipulator_group.anchor;
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_vec(),
			modification: VectorDataModification::SetManipulatorPosition { point, position },
		});

		Some(DocumentMessage::CommitTransaction)
	}
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			render_data,
			shape_editor,
			shape_overlay,
			..
		}: &mut ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let mut transform = tool_data.path.as_ref().and_then(|path| document.document_legacy.multiply_transforms(path).ok()).unwrap_or_default();

		if !transform.inverse().is_finite() {
			let parent_transform = tool_data
				.path
				.as_ref()
				.and_then(|layer_path| document.document_legacy.multiply_transforms(&layer_path[..layer_path.len() - 1]).ok());

			transform = parent_transform.unwrap_or(DAffine2::IDENTITY);
		}

		if !transform.inverse().is_finite() {
			transform = DAffine2::IDENTITY;
		}

		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(_, PenToolMessage::DocumentIsDirty) => {
					// When the document has moved / needs to be redraw, re-render the overlays
					// TODO the overlay system should probably receive this message instead of the tool
					for layer_path in document.selected_visible_layers() {
						shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer_path.to_vec(), responses);
					}
					self
				}
				(_, PenToolMessage::SelectionChanged) => {
					// Set the previously selected layers to invisible
					for layer_path in document.all_layers() {
						shape_overlay.layer_overlay_visibility(&document.document_legacy, layer_path.to_vec(), false, responses);
					}

					// Redraw the overlays of the newly selected layers
					for layer_path in document.selected_visible_layers() {
						shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer_path.to_vec(), responses);
					}
					self
				}
				(_, PenToolMessage::WorkingColorChanged) => {
					responses.add(PenToolMessage::UpdateOptions(PenOptionsUpdate::WorkingColors(
						Some(global_tool_data.primary_color),
						Some(global_tool_data.secondary_color),
					)));
					self
				}
				(PenToolFsmState::Ready, PenToolMessage::DragStart) => {
					responses.add(DocumentMessage::StartTransaction);

					// Initialize snapping
					tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, None, render_data), true, true);
					tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

					// Disable this tool's mirroring
					tool_data.should_mirror = false;

					// Perform extension of an existing path
					if let Some((layer, subpath_index, from_start)) = should_extend(document, input.mouse.position, crate::consts::SNAP_POINT_TOLERANCE) {
						tool_data.extend_subpath(layer, subpath_index, from_start, document, responses);
					} else {
						tool_data.create_new_path(
							document,
							tool_options.line_weight,
							tool_options.stroke.active_color(),
							tool_options.fill.active_color(),
							input,
							responses,
						);
					}

					// Enter the dragging handle state while the mouse is held down, allowing the user to move the mouse and position the handle
					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart) => {
					tool_data.check_break(document, transform, shape_overlay, responses);
					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => {
					tool_data.should_mirror = true;
					tool_data.finish_placing_handle(document, transform, shape_overlay, responses).unwrap_or(PenToolFsmState::PlacingAnchor)
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }) => {
					let modifiers = ModifierState {
						snap_angle: input.keyboard.key(snap_angle),
						lock_angle: input.keyboard.key(lock_angle),
						break_handle: input.keyboard.key(break_handle),
					};
					tool_data.drag_handle(document, transform, input.mouse.position, modifiers, responses).unwrap_or(PenToolFsmState::Ready)
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }) => {
					let modifiers = ModifierState {
						snap_angle: input.keyboard.key(snap_angle),
						lock_angle: input.keyboard.key(lock_angle),
						break_handle: input.keyboard.key(break_handle),
					};
					tool_data
						.place_anchor(document, transform, input.mouse.position, modifiers, responses)
						.unwrap_or(PenToolFsmState::Ready)
				}
				(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Abort | PenToolMessage::Confirm) => {
					// Abort or commit the transaction to the undo history
					let message = tool_data.finish_transaction(self, document, responses).unwrap_or(DocumentMessage::AbortTransaction);
					responses.add(message);

					// Clean up overlays
					for layer_path in document.all_layers() {
						shape_overlay.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}
					tool_data.path = None;
					tool_data.snap_manager.cleanup(responses);

					PenToolFsmState::Ready
				}
				(_, PenToolMessage::Abort) => {
					// Clean up overlays
					for layer_path in document.all_layers() {
						shape_overlay.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}
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
			PenToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Draw Path")])]),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Add Anchor"), HintInfo::mouse(MouseMotion::LmbDrag, "Add Handle")]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Snap 15°"), HintInfo::keys([Key::Control], "Lock Angle")]),
				HintGroup(vec![HintInfo::keys([Key::Alt], "Break Handle")]), // TODO: Show this only when dragging a handle
				HintGroup(vec![HintInfo::keys([Key::Enter], "End Path")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

/// Snap the angle of the line from relative to position if the key is pressed.
fn compute_snapped_angle(cached_angle: &mut f64, lock_angle: bool, snap_angle: bool, position: DVec2, relative: DVec2) -> DVec2 {
	let delta = relative - position;
	let mut angle = -delta.angle_between(DVec2::X);

	if lock_angle {
		angle = *cached_angle;
	}

	if snap_angle {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;
	}

	*cached_angle = angle;

	if snap_angle || lock_angle {
		let length = delta.length();
		let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
		relative - rotated
	} else {
		position
	}
}

/// Pushes a [ManipulatorGroup] to the current layer via a [GraphOperationMessage].
fn add_manipulator_group(layer_path: &Option<Vec<LayerId>>, from_start: bool, manipulator_group: bezier_rs::ManipulatorGroup<ManipulatorGroupId>) -> Message {
	let Some(layer) = layer_path.clone() else {
		return Message::NoOp;
	};
	let modification = if from_start {
		VectorDataModification::AddStartManipulatorGroup { subpath_index: 0, manipulator_group }
	} else {
		VectorDataModification::AddEndManipulatorGroup { subpath_index: 0, manipulator_group }
	};
	GraphOperationMessage::Vector { layer, modification }.into()
}

/// Determines if a path should be extended. Returns the path and if it is extending from the start, if applicable.
fn should_extend(document: &DocumentMessageHandler, pos: DVec2, tolerance: f64) -> Option<(&[LayerId], usize, bool)> {
	let mut best = None;
	let mut best_distance_squared = tolerance * tolerance;

	for layer_path in document.selected_layers() {
		let Ok(viewspace) = document.document_legacy.generate_transform_relative_to_viewport(layer_path) else { continue };

		let subpaths = get_subpaths(layer_path, document)?;
		for (subpath_index, subpath) in subpaths.iter().enumerate() {
			if subpath.closed() {
				continue;
			}

			for (manipulator_group, from_start) in [(subpath.manipulator_groups().first(), true), (subpath.manipulator_groups().last(), false)] {
				let Some(manipulator_group) = manipulator_group else { break };

				let distance_squared = viewspace.transform_point2(manipulator_group.anchor).distance_squared(pos);

				if distance_squared < best_distance_squared {
					best = Some((layer_path, subpath_index, from_start));
					best_distance_squared = distance_squared;
				}
			}
		}
	}

	best
}

fn get_subpaths<'a>(layer_path: &[LayerId], document: &'a DocumentMessageHandler) -> Option<&'a Vec<Subpath<ManipulatorGroupId>>> {
	let layer = document.document_legacy.layer(layer_path).ok().and_then(|layer| layer.as_layer().ok())?;
	let network = &layer.network;
	for (node, _node_id) in network.primary_flow() {
		if node.name == "Path Generator" {
			let subpaths_input = node.inputs.get(0)?;
			let NodeInput::Value { tagged_value: TaggedValue::Subpaths(subpaths), .. } = subpaths_input else {
				continue;
			};

			return Some(subpaths);
		}
	}
	None
}
