use super::tool_prelude::*;
use crate::consts::{COLOR_OVERLAY_BLUE, DRAG_THRESHOLD, HIDE_HANDLE_DISTANCE, LINE_ROTATE_SNAP_ANGLE, MANIPULATOR_GROUP_MARKER_SIZE, SEGMENT_INSERTION_DISTANCE, SEGMENT_OVERLAY_SIZE};
use crate::messages::portfolio::document::overlays::utility_functions::overlay_bezier_handles;
use crate::messages::portfolio::document::overlays::utility_types::{GizmoEmphasis, OverlayContext};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration};
use graphene_std::color::SRGBA8;
use graphene_std::raster::color::Color;
use graphene_std::subpath::{BezierHandles, pathseg_points};
use graphene_std::vector::algorithms::util::pathseg_tangent;
use graphene_std::vector::misc::{dvec2_to_point, point_to_dvec2};
use graphene_std::vector::style::GradientUI;
use graphene_std::vector::{HandleId, MeshGradient, SegmentId};
use graphene_std::{ATTR_TRANSFORM, Graphic};
use kurbo::{DEFAULT_ACCURACY, ParamCurve, ParamCurveNearest};

#[derive(Default, ExtractField)]
pub struct MeshGradientTool {
	fsm_state: MeshGradientToolFsmState,
	data: MeshGradientToolData,
}

#[impl_message(Message, ToolMessage, MeshGradient)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MeshGradientToolMessage {
	// Standard messages
	Abort,
	Overlays { context: OverlayContext },
	SelectionChanged,
	WorkingColorChanged,

	// Tool-specific messages
	DeleteEdge,
	DoubleClick,
	InsertStop,
	PointerDown,
	PointerMove { constrain_axis: Key, lock_angle: Key },
	PointerOutsideViewport { constrain_axis: Key, lock_angle: Key },
	PointerUp,
	StartTransactionForColorStop,
	CommitTransactionForColorStop,
	CloseStopColorPicker,
	UpdateStopColor { color: Color },
	UpdateStops { stops: GradientUI },
}

impl ToolMetadata for MeshGradientTool {
	fn icon_name(&self) -> String {
		"GeneralGradientTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Mesh Gradient Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::MeshGradient
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for MeshGradientTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		match message {
			ToolMessage::MeshGradient(MeshGradientToolMessage::UpdateStopColor { color }) => {
				let Some(selected_mesh) = self.data.selected_mesh.as_mut() else { return };

				if let MeshGradientTarget::Corner { corner_index, .. } = selected_mesh.target
					&& self.data.color_picker_editing_color_stop == Some(corner_index)
					&& selected_mesh.gradient.set_corner_color(corner_index, color).is_some()
				{
					selected_mesh.update_gradient_in_graph(responses);
					responses.add(PropertiesPanelMessage::Refresh);
					responses.add(OverlaysMessage::Draw);
				}
			}
			_ => {
				self.fsm_state.process_event(message, &mut self.data, context, &(), responses, false);
			}
		}
	}

	fn actions(&self) -> ActionList {
		let common = actions!(MeshGradientToolMessageDiscriminant;
			PointerDown,
			PointerUp,
			PointerMove,
			DoubleClick,
			DeleteEdge,
			Abort,
		);
		common
	}
}

impl LayoutHolder for MeshGradientTool {
	fn layout(&self) -> Layout {
		Layout::default()
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MeshGradientToolFsmState {
	Ready {
		hovering: MeshGradientHoverTarget,
		selected: MeshGradientSelectedTarget,
	},
	Dragging,
}

impl Default for MeshGradientToolFsmState {
	fn default() -> Self {
		Self::Ready {
			hovering: MeshGradientHoverTarget::None,
			selected: MeshGradientSelectedTarget::None,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
struct SelectedMeshGradient {
	layer: LayerNodeIdentifier,
	mesh_index: usize,
	gradient: MeshGradient,
	mesh_to_document: DAffine2,
	target: MeshGradientTarget,
}

impl SelectedMeshGradient {
	pub fn update_gradient_in_graph(&mut self, responses: &mut VecDeque<Message>) {
		responses.add(GraphOperationMessage::MeshGradientValueSet {
			layer: self.layer,
			mesh_gradient: self.gradient.clone(),
		});
	}
}

fn approximate_valid_region_bounds(initial_position: DVec2, [min, max]: [DVec2; 2], mut is_valid: impl FnMut(DVec2) -> bool) -> Option<[DVec2; 2]> {
	const SUBDIVISIONS: usize = 12;

	let mut x_samples = (0..=SUBDIVISIONS)
		.map(|index| min.x + (max.x - min.x) * index as f64 / SUBDIVISIONS as f64)
		.collect::<Vec<_>>();
	let mut y_samples = (0..=SUBDIVISIONS)
		.map(|index| min.y + (max.y - min.y) * index as f64 / SUBDIVISIONS as f64)
		.collect::<Vec<_>>();
	x_samples.push(initial_position.x);
	y_samples.push(initial_position.y);
	x_samples.sort_by(f64::total_cmp);
	y_samples.sort_by(f64::total_cmp);
	x_samples.dedup();
	y_samples.dedup();

	let columns = x_samples.len();
	let rows = y_samples.len();
	let seed_column = x_samples.iter().position(|&x| x == initial_position.x)?;
	let seed_row = y_samples.iter().position(|&y| y == initial_position.y)?;
	let seed_index = seed_row * columns + seed_column;

	let valid_samples = y_samples
		.iter()
		.flat_map(|&y| x_samples.iter().map(move |&x| DVec2::new(x, y)))
		.map(&mut is_valid)
		.collect::<Vec<_>>();

	if !valid_samples[seed_index] {
		return None;
	}

	let mut visited = vec![false; rows * columns];
	let mut queue = VecDeque::from([seed_index]);
	let mut bounds_min = initial_position;
	let mut bounds_max = initial_position;

	while let Some(index) = queue.pop_front() {
		if visited[index] || !valid_samples[index] {
			continue;
		}
		visited[index] = true;

		let row = index / columns;
		let column = index % columns;
		let position = DVec2::new(x_samples[column], y_samples[row]);
		bounds_min = bounds_min.min(position);
		bounds_max = bounds_max.max(position);

		if row > 0 {
			queue.push_back(index - columns);
		}
		if row + 1 < rows {
			queue.push_back(index + columns);
		}
		if column > 0 {
			queue.push_back(index - 1);
		}
		if column + 1 < columns {
			queue.push_back(index + 1);
		}
	}

	Some([bounds_min, bounds_max])
}

fn constrain_to_valid_region(target: DVec2, valid_region_center: DVec2, candidate: impl Fn(DVec2) -> Option<MeshGradient>) -> Option<MeshGradient> {
	candidate(target).or_else(|| {
		const BINARY_SEARCH_ITERATIONS: usize = 12;
		let mut valid_t = 0.;
		let mut invalid_t = 1.;
		let mut valid_gradient = candidate(valid_region_center)?;

		for _ in 0..BINARY_SEARCH_ITERATIONS {
			let mid_t = (valid_t + invalid_t) / 2.;
			let mid_position = valid_region_center.lerp(target, mid_t);

			if let Some(gradient) = candidate(mid_position) {
				valid_t = mid_t;
				valid_gradient = gradient;
			} else {
				invalid_t = mid_t;
			}
		}

		Some(valid_gradient)
	})
}

#[derive(Clone, Debug, PartialEq)]
enum MeshGradientTarget {
	Corner {
		corner_index: usize,
		initial_mouse: DVec2,
		initial_corner: DVec2,
		valid_region_center: DVec2,
	},
	Segment {
		segment_id: SegmentId,
		initial_mouse: DVec2,
		initial_handles: [DVec2; 2],
		valid_region_center: DVec2,
	},
	Handle {
		handle_id: HandleId,
		initial_mouse: DVec2,
		initial_handle: DVec2,
		valid_region_center: DVec2,
	},
}

impl ToolTransition for MeshGradientTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(MeshGradientToolMessage::Abort.into()),
			selection_changed: Some(MeshGradientToolMessage::SelectionChanged.into()),
			working_color_changed: Some(MeshGradientToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|context| MeshGradientToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct MeshGradientToolData {
	selected_mesh: Option<SelectedMeshGradient>,
	snap_manager: SnapManager,
	drag_start: DVec2,
	/// The pointer-down position before snapping (document space), used to detect whether the mouse moved between the press and a double-click.
	drag_start_unsnapped: DVec2,
	auto_panning: AutoPanning,
	auto_pan_shift: DVec2,
	color_picker_editing_color_stop: Option<usize>,
}

impl Fsm for MeshGradientToolFsmState {
	type ToolData = MeshGradientToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, viewport, .. } = tool_action_data;
		let ToolMessage::MeshGradient(event) = event else { return self };

		match (self, event) {
			(_, MeshGradientToolMessage::Overlays { context: mut overlay_context }) => {
				let metadata = document.metadata();
				let mut hovered_segment: Option<(f64, DVec2, DVec2)> = None;
				let mut hovering_corner = false;

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(fill) = metadata.layer_fill_attributes.get(&layer) else {
						continue;
					};

					let layer_to_viewport = metadata.transform_to_viewport(layer);

					for graphic in fill.iter_element_values() {
						let Graphic::MeshGradient(meshes) = graphic else {
							continue;
						};

						for index in 0..meshes.len() {
							let Some(mesh) = meshes.element(index) else {
								continue;
							};

							let mesh_to_layer: DAffine2 = meshes.attribute_cloned_or_default(ATTR_TRANSFORM, index);
							let mesh_to_viewport = layer_to_viewport * mesh_to_layer;
							let geometry = mesh.geometry();

							// Render the mesh geometry's outline in the same manner as the path tool does
							if overlay_context.visibility_settings.path() {
								overlay_context.outline_vector(geometry, mesh_to_viewport);
							}

							if let Some(selected_segment_id) = tool_data.selected_mesh.as_ref().and_then(|selected_mesh| {
								if selected_mesh.layer != layer || selected_mesh.mesh_index != index {
									return None;
								}
								match selected_mesh.target {
									MeshGradientTarget::Segment { segment_id, .. } => Some(segment_id),
									_ => None,
								}
							}) && let Some(edge) = mesh.edges().find(|edge| edge.segment_id == selected_segment_id)
							{
								overlay_context.outline_select_bezier(edge.segment, mesh_to_viewport);
							}

							if overlay_context.visibility_settings.handles() {
								for (segment_id, bezier, _, _) in geometry.segment_bezier_iter() {
									overlay_bezier_handles(bezier, segment_id, mesh_to_viewport, |_| false, &mut overlay_context);
								}
							}

							if overlay_context.visibility_settings.anchors() {
								for &position in geometry.point_domain.positions() {
									overlay_context.manipulator_anchor(mesh_to_viewport.transform_point2(position), false, None);
								}
							}

							// Then, place the color stop gizmos for all mesh corners
							for corner in mesh.corners() {
								let position = mesh_to_viewport.transform_point2(corner.position);
								let color = SRGBA8::from(corner.color).to_css_hex();
								hovering_corner |= position.distance_squared(input.mouse.position) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

								let is_selected = tool_data.selected_mesh.as_ref().is_some_and(|selected_mesh| {
									matches!(
										selected_mesh.target,
										MeshGradientTarget::Corner{corner_index, ..}
											if selected_mesh.layer == layer
												&& selected_mesh.mesh_index == index
												&& corner_index == corner.index
									)
								});

								let emphasis = if is_selected { GizmoEmphasis::Active } else { GizmoEmphasis::Regular };

								overlay_context.gradient_color_stop(position, emphasis, &color, false);
							}

							// Display the normal line overray when the mouse is on a edge
							if !hovering_corner {
								let local_mouse = mesh_to_viewport.inverse().transform_point2(input.mouse.position);
								for edge in mesh.edges() {
									let t = edge.segment.nearest(dvec2_to_point(local_mouse), DEFAULT_ACCURACY).t.clamp(0., 1.);
									let closest_local = point_to_dvec2(edge.segment.eval(t));
									let closest_viewport = mesh_to_viewport.transform_point2(closest_local);
									let distance_squared = closest_viewport.distance_squared(input.mouse.position);

									if distance_squared > SEGMENT_INSERTION_DISTANCE.powi(2) {
										continue;
									}

									let tangent_local = pathseg_tangent(edge.segment, t);
									let Some(tangent_viewport) = mesh_to_viewport.transform_vector2(tangent_local).try_normalize() else {
										continue;
									};
									let normal_viewport = tangent_viewport.perp();
									if hovered_segment.as_ref().is_none_or(|(closest_distance, _, _)| distance_squared < *closest_distance) {
										hovered_segment = Some((distance_squared, closest_viewport, normal_viewport));
									}
								}
							}
						}
					}
				}

				if matches!(self, MeshGradientToolFsmState::Ready { .. })
					&& !hovering_corner
					&& let Some((_, point, normal)) = hovered_segment
				{
					overlay_context.line(point - normal * SEGMENT_OVERLAY_SIZE, point + normal * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
				}

				tool_data.snap_manager.draw_overlays(SnapData::new(document, input, viewport), &mut overlay_context);

				match self {
					MeshGradientToolFsmState::Ready { selected, .. } => MeshGradientToolFsmState::Ready {
						hovering: if hovering_corner {
							MeshGradientHoverTarget::Corner
						} else if hovered_segment.is_some() {
							MeshGradientHoverTarget::Segment
						} else {
							MeshGradientHoverTarget::None
						},
						selected,
					},
					_ => self,
				}
			}

			(_state @ MeshGradientToolFsmState::Ready { .. }, MeshGradientToolMessage::DeleteEdge) => {
				let Some(selected_mesh) = tool_data.selected_mesh.as_mut() else { return self };
				if let MeshGradientTarget::Segment { segment_id, .. } = selected_mesh.target {
					selected_mesh.gradient.remove_edge(segment_id);
				};

				responses.add(DocumentMessage::StartTransaction);
				selected_mesh.update_gradient_in_graph(responses);
				responses.add(DocumentMessage::EndTransaction);
				tool_data.selected_mesh = None;
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::Ready {
					hovering: MeshGradientHoverTarget::None,
					selected: MeshGradientSelectedTarget::None,
				}
			}

			(_, MeshGradientToolMessage::DoubleClick) => {
				// Ignore when dragging
				let drag_start_viewport = document.metadata().document_to_viewport.transform_point2(tool_data.drag_start_unsnapped);
				if input.mouse.position.distance(drag_start_viewport) > DRAG_THRESHOLD {
					return self;
				}

				let Some(selected_mesh) = tool_data.selected_mesh.as_mut() else { return self };
				let mesh_to_viewport = document.metadata().document_to_viewport * selected_mesh.mesh_to_document;

				match selected_mesh.target {
					// Display color picker when the mesh corner color gizmo is double clicked
					MeshGradientTarget::Corner { corner_index, .. } => {
						let Some(corner) = selected_mesh.gradient.corners().find(|corner| corner.index == corner_index) else {
							return self;
						};

						tool_data.color_picker_editing_color_stop = Some(corner.index);

						let position = mesh_to_viewport.transform_point2(corner.position).into();
						responses.add(FrontendMessage::UpdateGradientStopColorPickerPosition { color: corner.color.into(), position });
					}
					MeshGradientTarget::Segment { segment_id, .. } => {
						let Some(segment) = selected_mesh.gradient.edges().find(|edge| edge.segment_id == segment_id) else {
							return self;
						};
						let local_mouse = mesh_to_viewport.inverse().transform_point2(input.mouse.position);
						let t = segment.segment.nearest(dvec2_to_point(local_mouse), DEFAULT_ACCURACY).t.clamp(0., 1.);
						if selected_mesh.gradient.insert_grid_line(segment.segment_id, t).is_none() {
							return self;
						}

						responses.add(DocumentMessage::StartTransaction);
						selected_mesh.update_gradient_in_graph(responses);
						responses.add(DocumentMessage::EndTransaction);
						responses.add(OverlaysMessage::Draw);
					}
					_ => {}
				};

				self
			}

			(MeshGradientToolFsmState::Ready { .. }, MeshGradientToolMessage::PointerDown) => {
				let metadata = document.metadata();
				let document_to_viewport = metadata.document_to_viewport;
				let mouse = input.mouse.position;
				let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
				tool_data.drag_start = document_mouse;
				tool_data.drag_start_unsnapped = document_mouse;
				tool_data.auto_pan_shift = DVec2::ZERO;
				let tolerance_squared = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(fill) = metadata.layer_fill_attributes.get(&layer) else {
						continue;
					};

					let layer_to_viewport = metadata.transform_to_viewport(layer);

					for graphic in fill.iter_element_values() {
						let Graphic::MeshGradient(meshes) = graphic else {
							continue;
						};

						for index in 0..meshes.len() {
							let Some(gradient) = meshes.element(index) else {
								continue;
							};

							let mesh_to_layer: DAffine2 = meshes.attribute_cloned_or_default(ATTR_TRANSFORM, index);
							let mesh_to_viewport = layer_to_viewport * mesh_to_layer;
							let mesh_to_document = document_to_viewport.inverse() * mesh_to_viewport;
							let local_mouse = mesh_to_viewport.inverse().transform_point2(mouse);

							// Change the corner position. Hit check on corners should have higher priority than the segments.
							for corner in gradient.corners() {
								let corner_in_viewport = mesh_to_viewport.transform_point2(corner.position);
								let distance_squared = corner_in_viewport.distance_squared(mouse);

								if distance_squared < tolerance_squared {
									responses.add(DocumentMessage::StartTransaction);
									let valid_region_center = gradient
										.geometry()
										.bounding_box()
										.and_then(|bounds| {
											approximate_valid_region_bounds(corner.position, bounds, |position| {
												let mut candidate = gradient.clone();
												candidate.set_corner_position(corner.index, position).is_some()
													&& candidate.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()))
											})
										})
										.map(|[min, max]| min.midpoint(max))
										.unwrap_or(corner.position);

									tool_data.selected_mesh = Some(SelectedMeshGradient {
										layer,
										mesh_index: index,
										gradient: gradient.clone(),
										mesh_to_document,
										target: MeshGradientTarget::Corner {
											corner_index: corner.index,
											initial_mouse: local_mouse,
											initial_corner: corner.position,
											valid_region_center,
										},
									});

									return MeshGradientToolFsmState::Dragging;
								}
							}

							let mut closest_handle: Option<(HandleId, DVec2, f64)> = None;
							let hidden_distance_squared = HIDE_HANDLE_DISTANCE.powi(2);

							// Change the handle position.
							for (segment_id, bezier, _, _) in gradient.geometry().segment_bezier_iter() {
								let mut consider_handle = |handle_id: HandleId, handle: DVec2, anchor: DVec2, _other_anchor: Option<DVec2>| {
									let handle_viewport = mesh_to_viewport.transform_point2(handle);
									let anchor_viewport = mesh_to_viewport.transform_point2(anchor);

									// Ignore handles that is not displayed in the overlay
									if handle_viewport.distance_squared(anchor_viewport) < hidden_distance_squared {
										return;
									}

									let distance_squared = handle_viewport.distance_squared(mouse);
									if distance_squared < tolerance_squared && closest_handle.as_ref().is_none_or(|(_, _, closest_distance)| distance_squared < *closest_distance) {
										closest_handle = Some((handle_id, handle, distance_squared));
									}
								};

								match bezier.handles {
									BezierHandles::Linear => {}
									BezierHandles::Quadratic { handle } => {
										consider_handle(HandleId::primary(segment_id), handle, bezier.start, Some(bezier.end));
									}
									BezierHandles::Cubic { handle_start, handle_end } => {
										consider_handle(HandleId::primary(segment_id), handle_start, bezier.start, None);
										consider_handle(HandleId::end(segment_id), handle_end, bezier.end, None);
									}
								}

								if let Some((handle_id, initial_handle, _)) = closest_handle {
									responses.add(DocumentMessage::StartTransaction);
									let valid_region_center = gradient
										.geometry()
										.bounding_box()
										.and_then(|bounds| {
											approximate_valid_region_bounds(initial_handle, bounds, |position| {
												let mut candidate = gradient.clone();
												candidate.set_handle_position(handle_id, position).is_some()
													&& candidate.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()))
											})
										})
										.map(|[min, max]| min.midpoint(max))
										.unwrap_or(initial_handle);

									tool_data.selected_mesh = Some(SelectedMeshGradient {
										layer,
										mesh_index: index,
										gradient: gradient.clone(),
										mesh_to_document,
										target: MeshGradientTarget::Handle {
											handle_id,
											initial_mouse: local_mouse,
											initial_handle,
											valid_region_center,
										},
									});

									return MeshGradientToolFsmState::Dragging;
								}
							}

							for edge in gradient.edges() {
								// Mold the mesh edge by dragging the segment directly while keeping the corners fixed.
								let t = edge.segment.nearest(dvec2_to_point(local_mouse), DEFAULT_ACCURACY).t;
								let closest_position_in_viewport = mesh_to_viewport.transform_point2(point_to_dvec2(edge.segment.eval(t)));
								let distance_squared = closest_position_in_viewport.distance_squared(mouse);

								if distance_squared < tolerance_squared {
									let points = pathseg_points(edge.segment);

									let handles = match (points.p1, points.p2) {
										(Some(p1), Some(p2)) => [p1, p2],
										(Some(p1), None) | (None, Some(p1)) => [p1, points.p3],
										(None, None) => [points.p0 + (points.p3 - points.p0) / 3., points.p3 + (points.p0 - points.p3) / 3.],
									};

									responses.add(DocumentMessage::StartTransaction);
									let valid_region_center = gradient
										.geometry()
										.bounding_box()
										.and_then(|bounds| {
											approximate_valid_region_bounds(local_mouse, bounds, |position| {
												let delta = position - local_mouse;
												let mut candidate = gradient.clone();
												candidate
													.set_edge_handles(edge.segment_id, BezierHandles::Cubic {
														handle_start: handles[0] + delta,
														handle_end: handles[1] + delta,
													})
													.is_some()
													&& candidate.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()))
											})
										})
										.map(|[min, max]| min.midpoint(max))
										.unwrap_or(local_mouse);

									tool_data.selected_mesh = Some(SelectedMeshGradient {
										layer,
										mesh_index: index,
										gradient: gradient.clone(),
										mesh_to_document,
										target: MeshGradientTarget::Segment {
											segment_id: edge.segment_id,
											initial_mouse: local_mouse,
											initial_handles: handles,
											valid_region_center,
										},
									});

									return MeshGradientToolFsmState::Dragging;
								}
							}
						}
					}
				}

				self
			}
			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::PointerMove { constrain_axis, lock_angle }) => {
				let MeshGradientToolData {
					selected_mesh,
					snap_manager,
					auto_panning,
					auto_pan_shift,
					..
				} = tool_data;
				let Some(selected_mesh) = selected_mesh.as_mut() else { return self };

				let document_to_viewport = document.metadata().document_to_viewport;
				let mesh_to_document = selected_mesh.mesh_to_document;
				let mut mesh_to_viewport = document_to_viewport * mesh_to_document;
				mesh_to_viewport.translation += *auto_pan_shift;
				*auto_pan_shift = DVec2::ZERO;

				let current_local_mouse = mesh_to_viewport.inverse().transform_point2(input.mouse.position);
				let snap_data = SnapData::new(document, input, viewport);
				let snap_angle = input.keyboard.get(constrain_axis as usize);
				let mut snap_local_point = |origin_local: DVec2, local_point: DVec2| {
					if snap_angle {
						snap_manager.clear_indicator();

						let origin_viewport = mesh_to_viewport.transform_point2(origin_local);
						let local_point_viewport = mesh_to_viewport.transform_point2(local_point);
						let delta = origin_viewport - local_point_viewport;
						let length = delta.length();
						if length <= f64::EPSILON {
							return local_point;
						}

						let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
						let angle = (-delta.angle_to(DVec2::X) / snap_resolution).round() * snap_resolution;
						let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
						return mesh_to_viewport.inverse().transform_point2(origin_viewport - rotated);
					}

					let document_point = mesh_to_document.transform_point2(local_point);
					let point = SnapCandidatePoint::gradient_handle(document_point);
					let snapped = snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default());
					let local_point = if snapped.is_snapped() {
						mesh_to_document.inverse().transform_point2(snapped.snapped_point_document)
					} else {
						local_point
					};
					snap_manager.update_indicator(snapped);
					local_point
				};

				match &mut selected_mesh.target {
					MeshGradientTarget::Corner {
						corner_index,
						initial_mouse,
						initial_corner,
						valid_region_center,
					} => {
						let corner_index = *corner_index;
						let initial_mouse = *initial_mouse;
						let initial_corner = *initial_corner;
						let valid_region_center = *valid_region_center;
						let desired_position = initial_corner + current_local_mouse - initial_mouse;
						let snapped_local_mouse = snap_local_point(initial_corner, desired_position);
						let candidate_gradient = |position| {
							let mut gradient = selected_mesh.gradient.clone();
							gradient.set_corner_position(corner_index, position)?;
							let is_valid = gradient.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()));
							is_valid.then_some(gradient)
						};
						let constrained_gradient = constrain_to_valid_region(snapped_local_mouse, valid_region_center, candidate_gradient);

						if let Some(gradient) = constrained_gradient {
							selected_mesh.gradient = gradient;
							selected_mesh.update_gradient_in_graph(responses);
							responses.add(OverlaysMessage::Draw);
						}
					}
					MeshGradientTarget::Segment {
						segment_id,
						initial_mouse: initial_local_mouse,
						initial_handles,
						valid_region_center,
					} => {
						let snapped_local_mouse = snap_local_point(*initial_local_mouse, current_local_mouse);
						let candidate_gradient = |mouse_position| {
							let delta = mouse_position - *initial_local_mouse;
							let mut gradient = selected_mesh.gradient.clone();
							gradient.set_edge_handles(
								*segment_id,
								BezierHandles::Cubic {
									handle_start: initial_handles[0] + delta,
									handle_end: initial_handles[1] + delta,
								},
							)?;
							let is_valid = gradient.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()));
							is_valid.then_some(gradient)
						};

						if let Some(gradient) = constrain_to_valid_region(snapped_local_mouse, *valid_region_center, candidate_gradient) {
							selected_mesh.gradient = gradient;
							selected_mesh.update_gradient_in_graph(responses);
							responses.add(OverlaysMessage::Draw);
						}
					}
					MeshGradientTarget::Handle {
						handle_id,
						initial_mouse,
						initial_handle,
						valid_region_center,
					} => {
						let delta = current_local_mouse - *initial_mouse;
						let new_handle_position = snap_local_point(*initial_handle, *initial_handle + delta);
						let candidate_gradient = |position| {
							let mut gradient = selected_mesh.gradient.clone();
							gradient.set_handle_position(*handle_id, position)?;
							let is_valid = gradient.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()));
							is_valid.then_some(gradient)
						};

						if let Some(gradient) = constrain_to_valid_region(new_handle_position, *valid_region_center, candidate_gradient) {
							selected_mesh.gradient = gradient;
							selected_mesh.update_gradient_in_graph(responses);
							responses.add(OverlaysMessage::Draw);
						}
					}
				};

				// Auto-panning
				let messages = [
					MeshGradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }.into(),
					MeshGradientToolMessage::PointerMove { constrain_axis, lock_angle }.into(),
				];
				auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				MeshGradientToolFsmState::Dragging
			}

			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::PointerUp) => {
				let Some(selected_mesh) = tool_data.selected_mesh.as_ref() else { return self };
				let selected = match selected_mesh.target {
					MeshGradientTarget::Corner { .. } => MeshGradientSelectedTarget::Corner,
					MeshGradientTarget::Segment { .. } => MeshGradientSelectedTarget::Segment,
					MeshGradientTarget::Handle { .. } => MeshGradientSelectedTarget::Handle,
				};

				responses.add(DocumentMessage::EndTransaction);
				tool_data.snap_manager.cleanup(responses);
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::Ready {
					hovering: MeshGradientHoverTarget::None,
					selected,
				}
			}
			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.selected_mesh = None;
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::default()
			}

			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses) {
					tool_data.auto_pan_shift += shift;
				}

				MeshGradientToolFsmState::Dragging
			}
			(state, MeshGradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }) => {
				let messages = [
					MeshGradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }.into(),
					MeshGradientToolMessage::PointerMove { constrain_axis, lock_angle }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}

			(state @ MeshGradientToolFsmState::Ready { .. }, MeshGradientToolMessage::PointerMove { .. }) => {
				responses.add(OverlaysMessage::Draw);
				state
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			MeshGradientToolFsmState::Ready { hovering, selected } => {
				let mut groups = match hovering {
					MeshGradientHoverTarget::None => vec![HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Edit Mesh")])],
					MeshGradientHoverTarget::Corner => vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Corner")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Edit Color")]),
					],
					MeshGradientHoverTarget::Segment => vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Segment")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Mold Segment")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Insert Grid Line")]),
					],
				};

				if matches!(selected, MeshGradientSelectedTarget::Segment) {
					groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Grid Line")]));
				}

				HintData(groups)
			}
			MeshGradientToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, _responses: &mut VecDeque<Message>) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum MeshGradientHoverTarget {
	#[default]
	None,
	Corner,
	Segment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum MeshGradientSelectedTarget {
	#[default]
	None,
	Corner,
	Segment,
	Handle,
}
