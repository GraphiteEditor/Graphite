use super::tool_prelude::*;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager};
use crate::messages::tool::common_functionality::utility_functions::should_extend;

use bezier_rs::{Bezier, BezierHandles};
use graph_craft::document::NodeId;
use graphene_core::uuid::generate_uuid;
use graphene_core::vector::{PointId, VectorModificationType};
use graphene_core::Color;
use graphene_std::vector::{HandleId, SegmentId};

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

#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PenToolMessage {
	// Standard messages
	Abort,
	SelectionChanged,
	WorkingColorChanged,
	Overlays(OverlayContext),

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove { snap_angle: Key, break_handle: Key, lock_angle: Key },
	PointerOutsideViewport { snap_angle: Key, break_handle: Key, lock_angle: Key },
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

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
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
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
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
			|color: &ColorButton| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
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
#[derive(Clone, Debug, Default)]
struct ModifierState {
	snap_angle: bool,
	lock_angle: bool,
	break_handle: bool,
}
#[derive(Clone, Debug)]
struct LastPoint {
	id: PointId,
	pos: DVec2,
	in_segment: Option<SegmentId>,
	handle_start: DVec2,
}

#[derive(Clone, Debug, Default)]
struct PenToolData {
	layer: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
	latest_points: Vec<LastPoint>,
	point_index: usize,
	handle_end: Option<DVec2>,
	next_point: DVec2,
	next_handle_start: DVec2,

	g1_continuous: bool,

	angle: f64,
	auto_panning: AutoPanning,
	modifiers: ModifierState,
}
impl PenToolData {
	fn latest_point(&self) -> Option<&LastPoint> {
		self.latest_points.get(self.point_index)
	}

	fn latest_point_mut(&mut self) -> Option<&mut LastPoint> {
		self.latest_points.get_mut(self.point_index)
	}

	fn add_point(&mut self, point: LastPoint) {
		self.point_index = (self.point_index + 1).min(self.latest_points.len());
		self.latest_points.truncate(self.point_index);
		self.latest_points.push(point);
	}

	/// If the user places the anchor on top of the previous anchor, it becomes sharp and the outgoing handle may be dragged.
	fn bend_from_previous_point(&mut self, snap_data: SnapData, transform: DAffine2) {
		self.g1_continuous = true;
		let document = snap_data.document;
		self.next_handle_start = self.next_point;

		// Break the control
		let Some(last_pos) = self.latest_point().map(|point| point.pos) else { return };
		let transform = document.metadata.document_to_viewport * transform;
		let on_top = transform.transform_point2(self.next_point).distance_squared(transform.transform_point2(last_pos)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		if on_top {
			if let Some(point) = self.latest_point_mut() {
				point.in_segment = None;
			}
			self.handle_end = None;
		}
	}

	fn finish_placing_handle(&mut self, snap_data: SnapData, transform: DAffine2, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;
		let next_handle_start = self.next_handle_start;
		let handle_start = self.latest_point()?.handle_start;
		let mouse = snap_data.input.mouse.position;
		let Some(handle_end) = self.handle_end else {
			self.handle_end = Some(next_handle_start);
			self.place_anchor(snap_data, transform, mouse, responses);
			self.latest_point_mut()?.handle_start = next_handle_start;
			return None;
		};
		let next_point = self.next_point;
		self.place_anchor(snap_data, transform, mouse, responses);
		let handles = [handle_start - self.latest_point()?.pos, handle_end - next_point].map(Some);

		// Get close path
		let mut end = None;
		let layer = self.layer?;
		let vector_data = document.metadata.compute_modified_vector(layer, &document.network)?;
		let start = self.latest_point()?.id;
		let transform = document.metadata.document_to_viewport * transform;
		for id in vector_data.single_connected_points().filter(|&point| point != start) {
			let Some(pos) = vector_data.point_domain.position_from_id(id) else { continue };
			let transformed_distance_between_squared = transform.transform_point2(pos).distance_squared(transform.transform_point2(next_point));
			let snap_point_tolerance_squared = crate::consts::SNAP_POINT_TOLERANCE.powi(2);
			if transformed_distance_between_squared < snap_point_tolerance_squared {
				end = Some(id);
			}
		}
		let close_subpath = end.is_some();

		// Generate new point if not closing
		let end = end.unwrap_or_else(|| {
			let end = PointId::generate();
			let modification_type = VectorModificationType::InsertPoint { id: end, position: next_point };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });

			end
		});

		let points = [start, end];
		let id = SegmentId::generate();
		let modification_type = VectorModificationType::InsertSegment { id, points, handles };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Mirror
		if let Some(last_segment) = self.latest_point().and_then(|point| point.in_segment) {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::SetG1Continuous {
					handles: [HandleId::end(last_segment), HandleId::primary(id)],
					enabled: true,
				},
			});
		}
		if !close_subpath {
			self.add_point(LastPoint {
				id: end,
				pos: next_point,
				in_segment: self.g1_continuous.then_some(id),
				handle_start: next_handle_start,
			});
		}
		Some(if close_subpath { PenToolFsmState::Ready } else { PenToolFsmState::PlacingAnchor })
	}

	fn drag_handle(&mut self, snap_data: SnapData, transform: DAffine2, mouse: DVec2, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let colinear = !self.modifiers.break_handle && self.handle_end.is_some();
		self.next_handle_start = self.compute_snapped_angle(snap_data, transform, colinear, mouse, Some(self.next_point), false);
		if let Some(handle_end) = self.handle_end.as_mut().filter(|_| colinear) {
			*handle_end = self.next_point * 2. - self.next_handle_start;
			self.g1_continuous = true;
		} else {
			self.g1_continuous = false;
		}

		responses.add(OverlaysMessage::Draw);

		Some(PenToolFsmState::DraggingHandle)
	}

	fn place_anchor(&mut self, snap_data: SnapData, transform: DAffine2, mouse: DVec2, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let relative = self.latest_point().map(|point| point.pos);
		self.next_point = self.compute_snapped_angle(snap_data, transform, false, mouse, relative, true);
		if let Some(handle_end) = self.handle_end.as_mut() {
			*handle_end = self.next_point;
			self.next_handle_start = self.next_point;
		}
		responses.add(OverlaysMessage::Draw);

		Some(PenToolFsmState::PlacingAnchor)
	}

	/// Snap the angle of the line from relative to position if the key is pressed.
	fn compute_snapped_angle(&mut self, snap_data: SnapData, transform: DAffine2, colinear: bool, mouse: DVec2, relative: Option<DVec2>, neighbor: bool) -> DVec2 {
		let ModifierState { snap_angle, lock_angle, .. } = self.modifiers;
		let document = snap_data.document;
		let mut document_pos = document.metadata.document_to_viewport.inverse().transform_point2(mouse);
		let snap = &mut self.snap_manager;

		let neighbors = relative.filter(|_| neighbor).map_or(Vec::new(), |neighbor| vec![neighbor]);

		if let Some(relative) = relative
			.map(|layer| transform.transform_point2(layer))
			.filter(|&relative| (snap_angle || lock_angle) && (relative - document_pos).length_squared() > f64::EPSILON * 100.)
		{
			let resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();

			let angle = if lock_angle {
				self.angle
			} else if (relative - document_pos) != DVec2::ZERO && !lock_angle {
				(-(relative - document_pos).angle_to(DVec2::X) / resolution).round() * resolution
			} else {
				self.angle
			};
			document_pos = relative - (relative - document_pos).project_onto(DVec2::new(angle.cos(), angle.sin()));

			let constraint = SnapConstraint::Line {
				origin: relative,
				direction: document_pos - relative,
			};
			let near_point = SnapCandidatePoint::handle_neighbors(document_pos, neighbors.clone());
			let far_point = SnapCandidatePoint::handle_neighbors(2. * relative - document_pos, neighbors);
			if colinear {
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
		} else if let Some(relative) = relative.map(|layer| transform.transform_point2(layer)).filter(|_| colinear) {
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
			if (relative - document_pos) != DVec2::ZERO {
				self.angle = -(relative - document_pos).angle_to(DVec2::X)
			}
		}

		transform.inverse().transform_point2(document_pos)
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
			(PenToolFsmState::Ready, PenToolMessage::Overlays(mut overlay_context)) => {
				path_overlays(document, shape_editor, &mut overlay_context);
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(_, PenToolMessage::Overlays(mut overlay_context)) => {
				let transform = document.metadata.document_to_viewport * transform;
				if let (Some((start, handle_start)), Some(handle_end)) = (tool_data.latest_point().map(|point| (point.pos, point.handle_start)), tool_data.handle_end) {
					let handles = BezierHandles::Cubic { handle_start, handle_end };
					let bezier = Bezier {
						start,
						handles,
						end: tool_data.next_point,
					};
					overlay_context.outline_bezier(bezier, transform);
				}

				let valid = |point: DVec2, handle: DVec2| point.distance_squared(handle) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
				let next_point = transform.transform_point2(tool_data.next_point);
				let next_handle_start = transform.transform_point2(tool_data.next_handle_start);
				overlay_context.line(next_point, next_handle_start);
				let start = tool_data.latest_point().map(|point| transform.transform_point2(point.pos));

				let handle_start = tool_data.latest_point().map(|point| transform.transform_point2(point.handle_start));
				let handle_end = tool_data.handle_end.map(|point| transform.transform_point2(point));

				if let (Some(start), Some(handle_start), Some(handle_end)) = (start, handle_start, handle_end) {
					overlay_context.line(start, handle_start);
					overlay_context.line(next_point, handle_end);

					path_overlays(document, shape_editor, &mut overlay_context);

					if self == PenToolFsmState::DraggingHandle && valid(next_point, handle_end) {
						overlay_context.manipulator_handle(handle_end, false);
					}
					if valid(start, handle_start) {
						overlay_context.manipulator_handle(handle_start, false);
					}
				} else {
					path_overlays(document, shape_editor, &mut overlay_context);
				}
				if self == PenToolFsmState::DraggingHandle && valid(next_point, next_handle_start) {
					overlay_context.manipulator_handle(next_handle_start, false);
				}
				overlay_context.manipulator_anchor(next_point, false, None);
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

				let point = SnapCandidatePoint::handle(document.metadata.document_to_viewport.inverse().transform_point2(input.mouse.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, None, false);
				let viewport = document.metadata.document_to_viewport.transform_point2(snapped.snapped_point_document);

				// Perform extension of an existing path
				if let Some((layer, point, position)) = should_extend(document, viewport, crate::consts::SNAP_POINT_TOLERANCE) {
					tool_data.add_point(LastPoint {
						id: point,
						pos: position,
						in_segment: None,
						handle_start: position,
					});
					tool_data.layer = Some(layer);
					tool_data.next_point = position;
					tool_data.next_handle_start = position;
				} else {
					// New path layer
					let nodes = {
						let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
						HashMap::from([(NodeId(0), node_type.to_document_node_default_inputs([], Default::default()))])
					};

					let parent = document.new_layer_parent(true);
					let layer = graph_modification_utils::new_custom(NodeId(generate_uuid()), nodes, parent, responses);
					tool_options.fill.apply_fill(layer, responses);
					tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
					tool_data.layer = Some(layer);

					// Generate first point
					let id = PointId::generate();
					let transform = document.metadata().transform_to_document(parent);
					let pos = transform.inverse().transform_point2(snapped.snapped_point_document);
					let modification_type = VectorModificationType::InsertPoint { id, position: pos };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
					tool_data.add_point(LastPoint {
						id,
						pos,
						in_segment: None,
						handle_start: pos,
					});
					tool_data.next_point = pos;
					tool_data.next_handle_start = pos;
				}
				tool_data.handle_end = None;

				// Enter the dragging handle state while the mouse is held down, allowing the user to move the mouse and position the handle
				PenToolFsmState::DraggingHandle
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart) => {
				if tool_data.handle_end.is_some() {
					responses.add(DocumentMessage::StartTransaction);
				}
				tool_data.bend_from_previous_point(SnapData::new(document, input), transform);
				PenToolFsmState::DraggingHandle
			}
			(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => tool_data
				.finish_placing_handle(SnapData::new(document, input), transform, responses)
				.unwrap_or(PenToolFsmState::PlacingAnchor),
			(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }) => {
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
				};
				let snap_data = SnapData::new(document, input);

				let state = tool_data.drag_handle(snap_data, transform, input.mouse.position, responses).unwrap_or(PenToolFsmState::Ready);

				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport { snap_angle, break_handle, lock_angle }.into(),
					PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				state
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }) => {
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
				};
				let state = tool_data
					.place_anchor(SnapData::new(document, input), transform, input.mouse.position, responses)
					.unwrap_or(PenToolFsmState::Ready);

				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport { snap_angle, break_handle, lock_angle }.into(),
					PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				state
			}
			(PenToolFsmState::Ready, PenToolMessage::PointerMove { .. }) => {
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PenToolFsmState::DraggingHandle, PenToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				PenToolFsmState::DraggingHandle
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				PenToolFsmState::PlacingAnchor
			}
			(state, PenToolMessage::PointerOutsideViewport { snap_angle, break_handle, lock_angle }) => {
				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport { snap_angle, break_handle, lock_angle }.into(),
					PenToolMessage::PointerMove { snap_angle, break_handle, lock_angle }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Abort | PenToolMessage::Confirm) => {
				tool_data.layer = None;
				tool_data.handle_end = None;
				tool_data.latest_points.clear();
				tool_data.point_index = 0;
				tool_data.snap_manager.cleanup(responses);

				PenToolFsmState::Ready
			}
			(_, PenToolMessage::Abort) => {
				responses.add(OverlaysMessage::Draw);

				self
			}
			(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Undo) => {
				if tool_data.point_index > 0 {
					tool_data.point_index -= 1;
					tool_data
						.place_anchor(SnapData::new(document, input), transform, input.mouse.position, responses)
						.unwrap_or(PenToolFsmState::PlacingAnchor)
				} else {
					responses.add(PenToolMessage::Abort);
					self
				}
			}
			(_, PenToolMessage::Redo) => {
				tool_data.point_index = (tool_data.point_index + 1).min(tool_data.latest_points.len().saturating_sub(1));
				tool_data
					.place_anchor(SnapData::new(document, input), transform, input.mouse.position, responses)
					.unwrap_or(PenToolFsmState::PlacingAnchor)
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PenToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Draw Path")])]),
			PenToolFsmState::PlacingAnchor => HintData(vec![
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Rmb, ""),
					HintInfo::keys([Key::Escape], "").prepend_slash(),
					HintInfo::keys([Key::Enter], "End Path").prepend_slash(),
				]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Snap 15°"), HintInfo::keys([Key::Control], "Lock Angle")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Add Sharp Point"), HintInfo::mouse(MouseMotion::LmbDrag, "Add Smooth Point")]),
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Lmb, ""),
					HintInfo::mouse(MouseMotion::LmbDrag, "Bend from Prev. Point").prepend_slash(),
				]),
			]),
			PenToolFsmState::DraggingHandle => HintData(vec![
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Rmb, ""),
					HintInfo::keys([Key::Escape], "").prepend_slash(),
					HintInfo::keys([Key::Enter], "End Path").prepend_slash(),
				]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Snap 15°"), HintInfo::keys([Key::Control], "Lock Angle")]),
				// TODO: Only show this if the handle being dragged is colinear, so don't show this when bending from the previous point (by clicking and dragging from the previously placed anchor)
				HintGroup(vec![HintInfo::keys([Key::Alt], "Bend Handle")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
