use super::tool_prelude::*;
use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::graph_modification_utils::get_subpaths;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager};
use crate::messages::tool::common_functionality::utility_funcitons::should_extend;

use graph_craft::document::NodeId;
use graphene_core::uuid::{generate_uuid, ManipulatorGroupId};
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::{ManipulatorPointId, SelectedType};
use graphene_core::Color;

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
	Abort,
	#[remain::unsorted]
	SelectionChanged,
	#[remain::unsorted]
	WorkingColorChanged,
	#[remain::unsorted]
	Overlays(OverlayContext),

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove {
		snap_angle: Key,
		break_handle: Key,
		lock_angle: Key,
	},
	Redo,
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
		.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for PenTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(color.value)).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(color.value)).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PenTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
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

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PenToolFsmState::Ready => actions!(PenToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				Confirm,
				Abort,
				PointerMove,
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
			tool_abort: Some(PenToolMessage::Abort.into()),
			selection_changed: Some(PenToolMessage::SelectionChanged.into()),
			working_color_changed: Some(PenToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|overlay_context| PenToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}
#[derive(Default)]
struct ModifierState {
	snap_angle: bool,
	lock_angle: bool,
	break_handle: bool,
}

#[derive(Clone, Debug, Default)]
struct PenToolData {
	weight: f64,
	layer: Option<LayerNodeIdentifier>,
	subpath_index: usize,
	snap_manager: SnapManager,
	should_mirror: bool,
	// Indicates that curve extension is occurring from the first point, rather than (more commonly) the last point
	from_start: bool,
	angle: f64,
}
impl PenToolData {
	fn extend_subpath(&mut self, layer: LayerNodeIdentifier, subpath_index: usize, from_start: bool, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.layer = Some(layer);
		self.from_start = from_start;
		self.subpath_index = subpath_index;

		// Stop the handles on the first point from mirroring
		let Some(subpaths) = get_subpaths(layer, &document.network) else {
			return;
		};
		let manipulator_groups = subpaths[subpath_index].manipulator_groups();
		let Some(last_handle) = (if from_start { manipulator_groups.first() } else { manipulator_groups.last() }) else {
			return;
		};

		responses.add(GraphOperationMessage::Vector {
			layer,
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
		let parent = document.new_layer_parent();
		// Deselect layers because we are now creating a new layer
		responses.add(DocumentMessage::DeselectAllLayers);

		// Get the position and set properties
		let transform = document.metadata().transform_to_document(parent);
		let point = SnapCandidatePoint::handle(document.metadata.document_to_viewport.inverse().transform_point2(input.mouse.position));
		let snapped = self.snap_manager.free_snap(&SnapData::new(document, input), &point, None, false);
		let start_position = transform.inverse().transform_point2(snapped.snapped_point_document);
		self.snap_manager.update_indicator(snapped);
		self.weight = line_weight;

		// Create the initial shape with a `bez_path` (only contains a moveto initially)
		let subpath = bezier_rs::Subpath::new(vec![bezier_rs::ManipulatorGroup::new(start_position, Some(start_position), Some(start_position))], false);
		let layer = graph_modification_utils::new_vector_layer(vec![subpath], NodeId(generate_uuid()), parent, responses);
		self.layer = Some(layer);

		responses.add(GraphOperationMessage::FillSet {
			layer,
			fill: if let Some(color) = fill_color { Fill::Solid(color) } else { Fill::None },
		});

		responses.add(GraphOperationMessage::StrokeSet {
			layer,
			stroke: Stroke::new(stroke_color, line_weight),
		});

		self.from_start = false;
		self.subpath_index = 0;
	}

	// TODO: tooltip / user documentation?
	/// If you place the anchor on top of the previous anchor then you break the mirror
	fn check_break(&mut self, document: &DocumentMessageHandler, transform: DAffine2, responses: &mut VecDeque<Message>) -> Option<()> {
		// Get subpath
		let layer = self.layer?;
		let subpath = &get_subpaths(layer, &document.network)?[self.subpath_index];

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
		let transform = document.metadata.document_to_viewport * transform;
		let on_top = transform.transform_point2(last_anchor).distance_squared(transform.transform_point2(previous_anchor)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		if !on_top {
			return None;
		}
		// Remove the point that has just been placed
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification: VectorDataModification::RemoveManipulatorGroup { id: last_manipulator_group.id },
		});

		// Move the in handle of the previous anchor to on top of the previous position
		let point = ManipulatorPointId::new(previous_manipulator_group.id, outwards_handle);
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification: VectorDataModification::SetManipulatorPosition { point, position: previous_anchor },
		});

		// Stop the handles on the last point from mirroring
		let id = previous_manipulator_group.id;
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification: VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle: false },
		});

		self.should_mirror = false;
		None
	}

	fn finish_placing_handle(&mut self, document: &DocumentMessageHandler, transform: DAffine2, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		// Get subpath
		let layer = self.layer?;
		let subpath = &get_subpaths(layer, &document.network)?[self.subpath_index];

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

		let transform = document.metadata.document_to_viewport * transform;
		let transformed_distance_between_squared = transform.transform_point2(last_anchor).distance_squared(transform.transform_point2(first_anchor));
		let snap_point_tolerance_squared = crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		let should_close_path = transformed_distance_between_squared < snap_point_tolerance_squared && previous_manipulator_group.is_some();
		if should_close_path {
			// Move the in handle of the first point to where the user has placed it
			let point = ManipulatorPointId::new(first_manipulator_group.id, inwards_handle);
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorPosition { point, position: last_in },
			});

			// Stop the handles on the first point from mirroring
			let id = first_manipulator_group.id;
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle: false },
			});

			// Remove the point that has just been placed
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::RemoveManipulatorGroup { id: last_manipulator_group.id },
			});

			// Push a close path node
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetClosed { index: 0, closed: true },
			});

			responses.add(DocumentMessage::CommitTransaction);

			// Clean up tool data
			self.layer = None;
			self.snap_manager.cleanup(responses);

			// Return to ready state
			return Some(PenToolFsmState::Ready);
		}
		// Add a new manipulator for the next anchor that we will place
		if let Some(out_handle) = outwards_handle.get_position(last_manipulator_group) {
			responses.add(add_manipulator_group(self.layer, self.from_start, bezier_rs::ManipulatorGroup::new_anchor(out_handle)));
		}

		Some(PenToolFsmState::PlacingAnchor)
	}

	fn drag_handle(&mut self, mut snap_data: SnapData, transform: DAffine2, mouse: DVec2, modifiers: ModifierState, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;
		// Get subpath
		let subpath = &get_subpaths(self.layer?, &document.network)?[self.subpath_index];

		// Get the last manipulator group
		let manipulator_groups = subpath.manipulator_groups();
		let last_manipulator_group = if self.from_start { manipulator_groups.first()? } else { manipulator_groups.last()? };

		// Get correct handle types
		let inwards_handle = if self.from_start { SelectedType::OutHandle } else { SelectedType::InHandle };
		let outwards_handle = if self.from_start { SelectedType::InHandle } else { SelectedType::OutHandle };

		// Get manipulator points
		let last_anchor = last_manipulator_group.anchor;

		let should_mirror = !modifiers.break_handle && self.should_mirror;

		snap_data.manipulators = vec![(self.layer?, last_manipulator_group.id)];
		let pos = self.compute_snapped_angle(snap_data, transform, modifiers.lock_angle, modifiers.snap_angle, should_mirror, mouse, Some(last_anchor), false);
		if !pos.is_finite() {
			return Some(PenToolFsmState::DraggingHandle);
		}

		// Update points on current segment (to show preview of new handle)
		let point = ManipulatorPointId::new(last_manipulator_group.id, outwards_handle);
		responses.add(GraphOperationMessage::Vector {
			layer: self.layer?,
			modification: VectorDataModification::SetManipulatorPosition { point, position: pos },
		});

		// Mirror handle of last segment
		if should_mirror {
			// Could also be written as `last_anchor.position * 2 - pos` but this way avoids overflow/underflow better
			let pos = last_anchor - (pos - last_anchor);
			let point = ManipulatorPointId::new(last_manipulator_group.id, inwards_handle);
			responses.add(GraphOperationMessage::Vector {
				layer: self.layer?,
				modification: VectorDataModification::SetManipulatorPosition { point, position: pos },
			});
		}

		// Update the mirror status of the currently modifying point
		let id = last_manipulator_group.id;
		responses.add(GraphOperationMessage::Vector {
			layer: self.layer?,
			modification: VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle: should_mirror },
		});

		Some(PenToolFsmState::DraggingHandle)
	}

	fn place_anchor(&mut self, mut snap_data: SnapData, transform: DAffine2, mouse: DVec2, modifiers: ModifierState, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;
		// Get subpath
		let layer = self.layer?;
		let subpath = &get_subpaths(layer, &document.network)?[self.subpath_index];

		// Get the last manipulator group and the one previous to that
		let mut manipulator_groups = subpath.manipulator_groups().iter();
		let last_manipulator_group = if self.from_start { manipulator_groups.next()? } else { manipulator_groups.next_back()? };
		let previous_manipulator_group = if self.from_start { manipulator_groups.next() } else { manipulator_groups.next_back() };

		// Get the first manipulator group
		let manipulator_groups = subpath.manipulator_groups();
		let first_manipulator_group = if self.from_start { manipulator_groups.last()? } else { manipulator_groups.first()? };

		// Get manipulator points
		let first_anchor = first_manipulator_group.anchor;

		let previous_anchor = previous_manipulator_group.map(|group| group.anchor);

		let pos = if let Some(last_anchor) = previous_anchor.filter(|&a| mouse.distance_squared(transform.transform_point2(a)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2)) {
			// Snap to the previously placed point (to show break control)
			last_anchor
		} else if mouse.distance_squared(transform.transform_point2(first_anchor)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2) {
			// Snap to the first point (to show close path)
			first_anchor
		} else {
			snap_data.manipulators = vec![(self.layer?, last_manipulator_group.id)];
			self.compute_snapped_angle(snap_data, transform, modifiers.lock_angle, modifiers.snap_angle, false, mouse, previous_anchor, true)
		};

		for manipulator_type in [SelectedType::Anchor, SelectedType::InHandle, SelectedType::OutHandle] {
			let point = ManipulatorPointId::new(last_manipulator_group.id, manipulator_type);
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorPosition { point, position: pos },
			});
		}
		Some(PenToolFsmState::PlacingAnchor)
	}

	/// Snap the angle of the line from relative to position if the key is pressed.
	fn compute_snapped_angle(&mut self, snap_data: SnapData, transform: DAffine2, lock_angle: bool, snap_angle: bool, mirror: bool, mouse: DVec2, relative: Option<DVec2>, neighbor: bool) -> DVec2 {
		let document = snap_data.document;
		let mut document_pos = document.metadata.document_to_viewport.inverse().transform_point2(mouse);
		let snap = &mut self.snap_manager;

		let neighbors = relative.filter(|_| neighbor).map_or(Vec::new(), |neighbor| vec![neighbor]);

		if let Some(relative) = relative.map(|layer| transform.transform_point2(layer)).filter(|_| snap_angle || lock_angle) {
			let resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
			let angle = if lock_angle {
				self.angle
			} else {
				(-(relative - document_pos).angle_between(DVec2::X) / resolution).round() * resolution
			};
			document_pos = relative - (relative - document_pos).project_onto(DVec2::new(angle.cos(), angle.sin()));

			let constraint = SnapConstraint::Line {
				origin: relative,
				direction: document_pos - relative,
			};
			let near_point = SnapCandidatePoint::handle_neighbors(document_pos, neighbors.clone());
			let far_point = SnapCandidatePoint::handle_neighbors(2. * relative - document_pos, neighbors);
			if mirror {
				let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, None);
				let snapped_far = snap.constrained_snap(&snap_data, &far_point, constraint, None);
				document_pos = if snapped_far.other_snap_better(&snapped) {
					snapped.snapped_point_document
				} else {
					2. * relative - snapped_far.snapped_point_document
				};
				snap.update_indicator(if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far });
			} else {
				let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, None);
				document_pos = snapped.snapped_point_document;
				snap.update_indicator(snapped);
			}
		} else if let Some(relative) = relative.map(|layer| transform.transform_point2(layer)).filter(|_| mirror) {
			let snapped = snap.free_snap(&snap_data, &SnapCandidatePoint::handle_neighbors(document_pos, neighbors.clone()), None, false);
			let snapped_far = snap.free_snap(&snap_data, &SnapCandidatePoint::handle_neighbors(2. * relative - document_pos, neighbors), None, false);
			document_pos = if snapped_far.other_snap_better(&snapped) {
				snapped.snapped_point_document
			} else {
				2. * relative - snapped_far.snapped_point_document
			};
			snap.update_indicator(if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far });
		} else {
			let snapped = snap.free_snap(&snap_data, &SnapCandidatePoint::handle_neighbors(document_pos, neighbors), None, false);
			document_pos = snapped.snapped_point_document;
			snap.update_indicator(snapped);
		}

		if let Some(relative) = relative.map(|layer| transform.transform_point2(layer)) {
			self.angle = -(relative - document_pos).angle_between(DVec2::X)
		}

		transform.inverse().transform_point2(document_pos)
	}

	fn finish_transaction(&mut self, fsm: PenToolFsmState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Option<DocumentMessage> {
		// Get subpath
		let subpath = &get_subpaths(self.layer?, &document.network)?[self.subpath_index];

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
			responses.add(GraphOperationMessage::Vector {
				layer: self.layer?,
				modification: VectorDataModification::RemoveManipulatorGroup { id: last_manipulator_group.id },
			});
			last_manipulator_group = previous_manipulator_group;
		}

		// Remove the out handle
		let point = ManipulatorPointId::new(last_manipulator_group.id, outwards_handle);
		let position = last_manipulator_group.anchor;
		responses.add(GraphOperationMessage::Vector {
			layer: self.layer?,
			modification: VectorDataModification::SetManipulatorPosition { point, position },
		});

		Some(DocumentMessage::CommitTransaction)
	}
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			shape_editor,
			..
		} = tool_action_data;

		let mut transform = tool_data.layer.map(|layer| document.metadata().transform_to_document(layer)).unwrap_or_default();

		if !transform.inverse().is_finite() {
			let parent_transform = tool_data
				.layer
				.and_then(|layer| layer.parent(document.metadata()))
				.map(|layer| document.metadata().transform_to_document(layer));

			transform = parent_transform.unwrap_or(DAffine2::IDENTITY);
		}

		if !transform.inverse().is_finite() {
			transform = DAffine2::IDENTITY;
		}

		let ToolMessage::Pen(event) = event else {
			return self;
		};
		match (self, event) {
			(_, PenToolMessage::SelectionChanged) => {
				responses.add(OverlaysMessage::Draw);
				self
			}
			(_, PenToolMessage::Overlays(mut overlay_context)) => {
				path_overlays(document, shape_editor, &mut overlay_context);
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

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
				responses.add(DocumentMessage::StartTransaction);
				tool_data.check_break(document, transform, responses);
				PenToolFsmState::DraggingHandle
			}
			(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => {
				tool_data.should_mirror = true;
				tool_data.finish_placing_handle(document, transform, responses).unwrap_or(PenToolFsmState::PlacingAnchor)
			}
			(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }) => {
				let modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
				};
				let snap_data = SnapData::new(document, input);
				tool_data
					.drag_handle(snap_data, transform, input.mouse.position, modifiers, responses)
					.unwrap_or(PenToolFsmState::Ready)
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }) => {
				let modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
				};
				tool_data
					.place_anchor(SnapData::new(document, input), transform, input.mouse.position, modifiers, responses)
					.unwrap_or(PenToolFsmState::Ready)
			}
			(PenToolFsmState::Ready, PenToolMessage::PointerMove { .. }) => {
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Abort | PenToolMessage::Confirm) => {
				// Abort or commit the transaction to the undo history
				let message = tool_data.finish_transaction(self, document, responses).unwrap_or(DocumentMessage::AbortTransaction);
				responses.add(message);

				tool_data.layer = None;
				tool_data.snap_manager.cleanup(responses);

				PenToolFsmState::Ready
			}
			(_, PenToolMessage::Abort) => {
				responses.add(OverlaysMessage::Draw);

				self
			}
			(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Undo) => tool_data
				.place_anchor(SnapData::new(document, input), transform, input.mouse.position, ModifierState::default(), responses)
				.unwrap_or(PenToolFsmState::PlacingAnchor),
			(_, PenToolMessage::Redo) => tool_data
				.place_anchor(SnapData::new(document, input), transform, input.mouse.position, ModifierState::default(), responses)
				.unwrap_or(PenToolFsmState::PlacingAnchor),
			_ => self,
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

/// Pushes a [ManipulatorGroup] to the current layer via a [GraphOperationMessage].
fn add_manipulator_group(layer: Option<LayerNodeIdentifier>, from_start: bool, manipulator_group: bezier_rs::ManipulatorGroup<ManipulatorGroupId>) -> Message {
	let Some(layer) = layer else {
		return Message::NoOp;
	};
	let modification = if from_start {
		VectorDataModification::AddStartManipulatorGroup { subpath_index: 0, manipulator_group }
	} else {
		VectorDataModification::AddEndManipulatorGroup { subpath_index: 0, manipulator_group }
	};
	GraphOperationMessage::Vector { layer, modification }.into()
}
