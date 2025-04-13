use crate::consts::{ANGLE_MEASURE_RADIUS_FACTOR, ARC_MEASURE_RADIUS_FACTOR_RANGE, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_TRANSPARENT, SLOWING_DIVISOR};
use crate::messages::input_mapper::utility_types::input_mouse::{DocumentPosition, ViewportPosition};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, TransformType, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::tool_messages::path_tool::ProportionalEditData;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::{ToolData, ToolType};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::vector::ManipulatorPointId;
use graphene_std::vector::{PointId, VectorData, VectorModificationType};
use std::f64::consts::{PI, TAU};

const TRANSFORM_GRS_OVERLAY_PROVIDER: OverlayProvider = |context| TransformLayerMessage::Overlays(context).into();

// TODO: Get these from the input mapper
const SLOW_KEY: Key = Key::Shift;
const INCREMENTS_KEY: Key = Key::Control;

#[derive(Debug, Clone, Default)]
pub struct TransformLayerMessageHandler {
	pub transform_operation: TransformOperation,

	slow: bool,
	increments: bool,
	local: bool,
	layer_bounding_box: Quad,
	typing: Typing,

	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	pivot: ViewportPosition,

	local_pivot: DocumentPosition,
	local_mouse_start: DocumentPosition,
	grab_target: DocumentPosition,

	ptz: PTZ,
	initial_transform: DAffine2,

	operation_count: usize,

	// Pen tool (outgoing handle GRS manipulation)
	handle: DVec2,
	last_point: DVec2,
	grs_pen_handle: bool,

	// Path tool ( proportional edit )
	initial_positions: HashMap<LayerNodeIdentifier, HashMap<PointId, DVec2>>,
	proportional_edit_data: Option<ProportionalEditData>,
}

impl TransformLayerMessageHandler {
	pub fn is_transforming(&self) -> bool {
		self.transform_operation != TransformOperation::None
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>) {
		self.transform_operation.hints(responses, self.local);
	}
	pub fn calculate_total_transformation_vp(&self, document_to_viewport: DAffine2) -> DAffine2 {
		let pivot_vp = document_to_viewport.transform_point2(self.local_pivot);
		let local_axis_transform_angle = (self.layer_bounding_box.0[1] - self.layer_bounding_box.0[0]).to_angle();

		match self.transform_operation {
			TransformOperation::Grabbing(translation) => {
				let total_delta_doc = translation.to_dvec(self.initial_transform, self.increments);
				let translate = DAffine2::from_translation(document_to_viewport.transform_vector2(total_delta_doc));
				if self.local {
					let resolved_angle = if local_axis_transform_angle > 0. {
						local_axis_transform_angle
					} else {
						local_axis_transform_angle - std::f64::consts::PI
					};
					DAffine2::from_angle(resolved_angle) * translate * DAffine2::from_angle(-resolved_angle)
				} else {
					translate
				}
			}
			TransformOperation::Rotating(rotation) => {
				let total_angle = rotation.to_f64(self.increments);
				let pivot_transform = DAffine2::from_translation(pivot_vp);
				pivot_transform * DAffine2::from_angle(total_angle) * pivot_transform.inverse()
			}
			TransformOperation::Scaling(scale) => {
				let total_scale_vec = scale.to_dvec(self.increments);
				let pivot_transform = DAffine2::from_translation(pivot_vp);
				if self.local {
					pivot_transform
						* DAffine2::from_angle(local_axis_transform_angle)
						* DAffine2::from_scale(total_scale_vec)
						* DAffine2::from_angle(-local_axis_transform_angle)
						* pivot_transform.inverse()
				} else {
					pivot_transform * DAffine2::from_scale(total_scale_vec) * pivot_transform.inverse()
				}
			}
			TransformOperation::None => DAffine2::IDENTITY,
		}
	}

	// Apply proportional editing with the given transformation
	fn apply_proportional_editing(&mut self, total_transformation_vp: DAffine2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		if let Some(prop_data) = &self.proportional_edit_data {
			apply_proportional_edit(&self.initial_positions, prop_data, total_transformation_vp, &document.network_interface, document.metadata(), responses);
		}
	}
}

fn calculate_pivot(selected_points: &Vec<&ManipulatorPointId>, vector_data: &VectorData, viewspace: DAffine2, get_location: impl Fn(&ManipulatorPointId) -> Option<DVec2>) -> Option<(DVec2, DVec2)> {
	let [point] = selected_points.as_slice() else {
		// Handle the case where there are multiple points
		let mut point_count = 0;
		let average_position = selected_points.iter().filter_map(|p| get_location(p)).inspect(|_| point_count += 1).sum::<DVec2>() / point_count as f64;

		return Some((average_position, average_position));
	};

	match point {
		ManipulatorPointId::PrimaryHandle(_) | ManipulatorPointId::EndHandle(_) => {
			// Get the anchor position and transform it to the pivot
			let pivot_pos = point.get_anchor_position(vector_data).map(|anchor_position| viewspace.transform_point2(anchor_position))?;
			let target = viewspace.transform_point2(point.get_position(vector_data)?);
			Some((pivot_pos, target))
		}
		_ => {
			// Calculate the average position of all selected points
			let mut point_count = 0;
			let average_position = selected_points.iter().filter_map(|p| get_location(p)).inspect(|_| point_count += 1).sum::<DVec2>() / point_count as f64;
			Some((average_position, average_position))
		}
	}
}

fn project_edge_to_quad(edge: DVec2, quad: &Quad, local: bool, axis_constraint: Axis) -> DVec2 {
	match axis_constraint {
		Axis::X => {
			if local {
				edge.project_onto(quad.top_right() - quad.top_left())
			} else {
				edge.with_y(0.)
			}
		}
		Axis::Y => {
			if local {
				edge.project_onto(quad.bottom_left() - quad.top_left())
			} else {
				edge.with_x(0.)
			}
		}
		_ => edge,
	}
}
fn apply_proportional_edit(
	initial_positions: &HashMap<LayerNodeIdentifier, HashMap<PointId, DVec2>>,
	proportional_data: &ProportionalEditData,
	total_transformation_vp: DAffine2,
	network_interface: &NodeNetworkInterface,
	document_metadata: &DocumentMetadata,
	responses: &mut VecDeque<Message>,
) {
	// Iterate through layers that have initial positions
	for (layer, layer_initial_positions) in initial_positions {
		// Get current vector data for position comparison
		let Some(current_vector_data) = network_interface.compute_modified_vector(*layer) else {
			continue;
		};

		let viewspace = document_metadata.transform_to_viewport(*layer);

		// Create a lookup map for affected points
		let affected_points_map: HashMap<PointId, f64> = proportional_data
			.affected_points
			.get(layer)
			.map(|points| points.iter().map(|(id, factor)| (*id, *factor)).collect())
			.unwrap_or_default();

		// Process ALL points that were stored in initial positions
		for (point_id, initial_pos_local) in layer_initial_positions {
			let Some(current_pos_local) = current_vector_data.point_domain.position_from_id(*point_id) else {
				continue;
			};

			// Transform initial position to viewport space
			let initial_pos_vp = viewspace.transform_point2(*initial_pos_local);

			if let Some(factor) = affected_points_map.get(point_id) {
				// AFFECTED POINT: Apply proportional transformation
				let target_pos_fully_transformed_vp = total_transformation_vp.transform_point2(initial_pos_vp);
				let full_intended_delta_vp = target_pos_fully_transformed_vp - initial_pos_vp;

				let strength_divisor = (proportional_data.falloff_strength as f64).max(1.0);
				let scaled_intended_delta_vp = full_intended_delta_vp * (*factor) / strength_divisor;

				let target_pos_proportional_vp = initial_pos_vp + scaled_intended_delta_vp;
				let target_pos_proportional_local = viewspace.inverse().transform_point2(target_pos_proportional_vp);

				let final_delta_local = target_pos_proportional_local - current_pos_local;

				if final_delta_local.length_squared() > 1e-10 {
					let modification_type = VectorModificationType::ApplyPointDelta {
						point: *point_id,
						delta: final_delta_local,
					};
					responses.add(GraphOperationMessage::Vector { layer: *layer, modification_type });
				}
			} else {
				// NOT AFFECTED: Reset to original position
				let reset_delta = *initial_pos_local - current_pos_local;

				if reset_delta.length_squared() > 1e-10 {
					let modification_type = VectorModificationType::ApplyPointDelta { point: *point_id, delta: reset_delta };
					responses.add(GraphOperationMessage::Vector { layer: *layer, modification_type });
				}
			}
		}
	}
}
fn update_colinear_handles(selected_layers: &[LayerNodeIdentifier], document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	for &layer in selected_layers {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else { continue };

		for [handle1, handle2] in &vector_data.colinear_manipulators {
			let manipulator1 = handle1.to_manipulator_point();
			let manipulator2 = handle2.to_manipulator_point();

			let Some(anchor) = manipulator1.get_anchor_position(&vector_data) else { continue };
			let Some(pos1) = manipulator1.get_position(&vector_data).map(|pos| pos - anchor) else { continue };
			let Some(pos2) = manipulator2.get_position(&vector_data).map(|pos| pos - anchor) else { continue };

			let angle = pos1.angle_to(pos2);

			// Check if handles are not colinear (not approximately equal to +/- PI)
			if (angle - PI).abs() > 1e-6 && (angle + PI).abs() > 1e-6 {
				let modification_type = VectorModificationType::SetG1Continuous {
					handles: [*handle1, *handle2],
					enabled: false,
				};

				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}
	}
}

type TransformData<'a> = (&'a DocumentMessageHandler, &'a InputPreprocessorMessageHandler, &'a ToolData, &'a mut ShapeState);
impl MessageHandler<TransformLayerMessage, TransformData<'_>> for TransformLayerMessageHandler {
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, (document, input, tool_data, shape_editor): TransformData) {
		let using_path_tool = tool_data.active_tool_type == ToolType::Path;
		let using_select_tool = tool_data.active_tool_type == ToolType::Select;
		let using_pen_tool = tool_data.active_tool_type == ToolType::Pen;

		// TODO: Add support for transforming layer not in the document network
		let selected_layers = document
			.network_interface
			.selected_nodes()
			.selected_layers(document.metadata())
			.filter(|&layer| document.network_interface.is_visible(&layer.to_node(), &[]) && !document.network_interface.is_locked(&layer.to_node(), &[]))
			.collect::<Vec<_>>();

		let mut selected = Selected::new(
			&mut self.original_transforms,
			&mut self.pivot,
			&selected_layers,
			responses,
			&document.network_interface,
			Some(shape_editor),
			&tool_data.active_tool_type,
			Some(&mut self.handle),
		);

		let document_to_viewport = document.metadata().document_to_viewport;
		let mut begin_operation = |operation: TransformOperation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2, transform: &mut DAffine2| {
			if operation != TransformOperation::None {
				selected.revert_operation();
				typing.clear();
			}

			if using_pen_tool {
				selected.responses.add(PenToolMessage::GRS {
					grab: Key::KeyG,
					rotate: Key::KeyR,
					scale: Key::KeyS,
				});
				return;
			}

			if !using_path_tool {
				*selected.pivot = selected.mean_average_of_pivots();
				self.local_pivot = document.metadata().document_to_viewport.inverse().transform_point2(*selected.pivot);
				self.grab_target = document.metadata().document_to_viewport.inverse().transform_point2(selected.mean_average_of_pivots());
			} else if let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
				*selected.original_transforms = OriginalTransforms::default();

				let viewspace = document.metadata().transform_to_viewport(selected_layers[0]);
				let selected_points = shape_editor.selected_points().collect::<Vec<_>>();

				let get_location = |point: &&ManipulatorPointId| point.get_position(&vector_data).map(|position| viewspace.transform_point2(position));
				if let Some((new_pivot, grab_target)) = calculate_pivot(&selected_points, &vector_data, viewspace, |point: &ManipulatorPointId| get_location(&point)) {
					*selected.pivot = new_pivot;

					self.local_pivot = document_to_viewport.inverse().transform_point2(*selected.pivot);
					self.grab_target = document_to_viewport.inverse().transform_point2(grab_target);
				} else {
					log::warn!("Failed to calculate pivot.");
				}
			}

			*mouse_position = input.mouse.position;
			*start_mouse = input.mouse.position;
			*transform = document_to_viewport;
			self.local_mouse_start = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);

			selected.original_transforms.clear();

			selected.responses.add(DocumentMessage::StartTransaction);
		};

		match message {
			TransformLayerMessage::UpdateProportionalEditData(proportional_data) => {
				// Only update if we're in a transform operation with proportional editing
				if let Some(current_proportional_data) = &mut self.proportional_edit_data {
					// Update all fields from the new data
					current_proportional_data.center = proportional_data.center;
					current_proportional_data.affected_points = proportional_data.affected_points;
					current_proportional_data.falloff_type = proportional_data.falloff_type;
					current_proportional_data.falloff_strength = proportional_data.falloff_strength;
					current_proportional_data.radius = proportional_data.radius;

					// TODO: Essentialy a hack to trigger redraw for updated values
					responses.add(TransformLayerMessage::PointerMove {
						slow_key: SLOW_KEY,
						increments_key: INCREMENTS_KEY,
					});
					responses.add(OverlaysMessage::Draw);
				}
			}
			// Overlays
			TransformLayerMessage::Overlays(mut overlay_context) => {
				if let Some(proportional_data) = &self.proportional_edit_data {
					let viewport_center = document.metadata().document_to_viewport.transform_point2(proportional_data.center);
					let radius_viewport = document.metadata().document_to_viewport.transform_vector2(DVec2::X * proportional_data.radius as f64).x;

					overlay_context.circle(viewport_center, radius_viewport, Some(COLOR_OVERLAY_TRANSPARENT), Some(COLOR_OVERLAY_BLUE));
				}
				for layer in document.metadata().all_layers() {
					if !document.network_interface.is_artboard(&layer.to_node(), &[]) {
						continue;
					};

					let viewport_box = input.viewport_bounds.size();
					let axis_constraint = self.transform_operation.axis_constraint();

					let format_rounded = |value: f64, precision: usize| {
						if self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
							format!("{:.*}", precision, value).trim_end_matches('0').trim_end_matches('.').to_string()
						} else {
							self.typing.string.clone()
						}
					};

					// TODO: Ensure removing this and adding this doesn't change the position of layers under PTZ ops
					// responses.add(TransformLayerMessage::PointerMove {
					// 	slow_key: SLOW_KEY,
					// 	increments_key: INCREMENTS_KEY,
					// });

					match self.transform_operation {
						TransformOperation::None => (),
						TransformOperation::Grabbing(translation) => {
							let translation = translation.to_dvec(self.initial_transform, self.increments);
							let viewport_translate = document_to_viewport.transform_vector2(translation);
							let pivot = document_to_viewport.transform_point2(self.grab_target);
							let quad = Quad::from_box([pivot, pivot + viewport_translate]).0;
							let e1 = (self.layer_bounding_box.0[1] - self.layer_bounding_box.0[0]).normalize_or(DVec2::X);

							if matches!(axis_constraint, Axis::Both | Axis::X) && translation.x != 0. {
								let end = if self.local { (quad[1] - quad[0]).rotate(e1) + quad[0] } else { quad[1] };
								overlay_context.line(quad[0], end, None, None);

								let x_transform = DAffine2::from_translation((quad[0] + end) / 2.);
								overlay_context.text(&format_rounded(translation.x, 3), COLOR_OVERLAY_BLUE, None, x_transform, 4., [Pivot::Middle, Pivot::End]);
							}

							if matches!(axis_constraint, Axis::Both | Axis::Y) && translation.y != 0. {
								let end = if self.local { (quad[3] - quad[0]).rotate(e1) + quad[0] } else { quad[3] };
								overlay_context.line(quad[0], end, None, None);
								let x_parameter = viewport_translate.x.clamp(-1., 1.);
								let y_transform = DAffine2::from_translation((quad[0] + end) / 2. + x_parameter * DVec2::X * 0.);
								let pivot_selection = if x_parameter >= -1e-3 { Pivot::Start } else { Pivot::End };
								if axis_constraint != Axis::Both || self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
									overlay_context.text(&format_rounded(translation.y, 2), COLOR_OVERLAY_BLUE, None, y_transform, 3., [pivot_selection, Pivot::Middle]);
								}
							}
							if matches!(axis_constraint, Axis::Both) && translation.x != 0. && translation.y != 0. {
								overlay_context.dashed_line(quad[1], quad[2], None, None, Some(2.), Some(2.), Some(0.5));
								overlay_context.dashed_line(quad[3], quad[2], None, None, Some(2.), Some(2.), Some(0.5));
							}
						}
						TransformOperation::Scaling(scale) => {
							let scale = scale.to_f64(self.increments);
							let text = format!("{}x", format_rounded(scale, 3));
							let pivot = document_to_viewport.transform_point2(self.local_pivot);
							let start_mouse = document_to_viewport.transform_point2(self.local_mouse_start);
							let local_edge = start_mouse - pivot;
							let local_edge = project_edge_to_quad(local_edge, &self.layer_bounding_box, self.local, axis_constraint);
							let boundary_point = pivot + local_edge * scale.min(1.);
							let end_point = pivot + local_edge * scale.max(1.);

							if scale > 0. {
								overlay_context.dashed_line(pivot, boundary_point, None, None, Some(4.), Some(4.), Some(0.5));
							}
							overlay_context.line(boundary_point, end_point, None, None);

							let transform = DAffine2::from_translation(boundary_point.midpoint(pivot) + local_edge.perp().normalize_or(DVec2::X) * local_edge.element_product().signum() * 24.);
							overlay_context.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
						}
						TransformOperation::Rotating(rotation) => {
							let angle = rotation.to_f64(self.increments);
							let pivot = document_to_viewport.transform_point2(self.local_pivot);
							let start_mouse = document_to_viewport.transform_point2(self.local_mouse_start);
							let offset_angle = if self.grs_pen_handle {
								self.handle - self.last_point
							} else if using_path_tool {
								start_mouse - pivot
							} else {
								self.layer_bounding_box.top_right() - self.layer_bounding_box.top_right()
							};
							let tilt_offset = document.document_ptz.unmodified_tilt();
							let offset_angle = offset_angle.to_angle() + tilt_offset;
							let width = viewport_box.max_element();
							let radius = start_mouse.distance(pivot);
							let arc_radius = ANGLE_MEASURE_RADIUS_FACTOR * width;
							let radius = radius.clamp(ARC_MEASURE_RADIUS_FACTOR_RANGE.0 * width, ARC_MEASURE_RADIUS_FACTOR_RANGE.1 * width);
							let angle_in_degrees = angle.to_degrees();
							let display_angle = if angle_in_degrees.is_sign_positive() {
								angle_in_degrees - (angle_in_degrees / 360.).floor() * 360.
							} else if angle_in_degrees.is_sign_negative() {
								angle_in_degrees - ((angle_in_degrees / 360.).floor() + 1.) * 360.
							} else {
								angle_in_degrees
							};
							let text = format!("{}°", format_rounded(display_angle, 2));
							let text_texture_width = overlay_context.get_width(&text) / 2.;
							let text_texture_height = 12.;
							let text_angle_on_unit_circle = DVec2::from_angle((angle % TAU) / 2. + offset_angle);
							let text_texture_position = DVec2::new(
								(arc_radius + 4. + text_texture_width) * text_angle_on_unit_circle.x,
								(arc_radius + text_texture_height) * text_angle_on_unit_circle.y,
							);
							let transform = DAffine2::from_translation(text_texture_position + pivot);
							overlay_context.draw_angle(pivot, radius, arc_radius, offset_angle, angle);
							overlay_context.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
						}
					}
				}
			}

			// Messages
			TransformLayerMessage::ApplyTransformOperation { final_transform } => {
				selected.original_transforms.clear();
				self.typing.clear();
				if final_transform {
					self.transform_operation = TransformOperation::None;
					self.operation_count = 0;
				}

				if using_pen_tool {
					self.last_point = DVec2::ZERO;
					self.grs_pen_handle = false;

					selected.pen_handle = None;
					selected.responses.add(PenToolMessage::Confirm);
				} else {
					update_colinear_handles(&selected_layers, document, responses);
					responses.add(DocumentMessage::EndTransaction);
					responses.add(ToolMessage::UpdateHints);
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				if final_transform {
					self.proportional_edit_data = None;
					self.initial_positions.clear();
					responses.add(OverlaysMessage::RemoveProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				}
			}
			TransformLayerMessage::BeginGrabPen { last_point, handle } | TransformLayerMessage::BeginRotatePen { last_point, handle } | TransformLayerMessage::BeginScalePen { last_point, handle } => {
				self.typing.clear();

				self.last_point = last_point;
				self.handle = handle;
				self.grs_pen_handle = true;
				self.mouse_position = input.mouse.position;
				self.start_mouse = input.mouse.position;

				let top_left = DVec2::new(last_point.x, handle.y);
				let bottom_right = DVec2::new(handle.x, last_point.y);
				self.local = false;
				self.layer_bounding_box = Quad::from_box([top_left, bottom_right]);
				self.grab_target = document.metadata().document_to_viewport.inverse().transform_point2(handle);
				self.pivot = last_point;
				self.local_pivot = document.metadata().document_to_viewport.inverse().transform_point2(self.pivot);
				self.local_mouse_start = document.metadata().document_to_viewport.inverse().transform_point2(self.start_mouse);
				self.handle = handle;

				// Operation-specific logic
				self.transform_operation = match message {
					TransformLayerMessage::BeginGrabPen { .. } => TransformOperation::Grabbing(Default::default()),
					TransformLayerMessage::BeginRotatePen { .. } => TransformOperation::Rotating(Default::default()),
					TransformLayerMessage::BeginScalePen { .. } => TransformOperation::Scaling(Default::default()),
					_ => unreachable!(), // Safe because the match arms are exhaustive
				};

				responses.add(OverlaysMessage::AddProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				// Find a way better than this hack
				responses.add(TransformLayerMessage::PointerMove {
					slow_key: SLOW_KEY,
					increments_key: INCREMENTS_KEY,
				});
			}
			TransformLayerMessage::BeginGRS {
				transform_type,
				proportional_edit_data,
			} => {
				let selected_points: Vec<&ManipulatorPointId> = shape_editor.selected_points().collect();
				if (using_path_tool && selected_points.is_empty())
					|| (!using_path_tool && !using_select_tool && !using_pen_tool)
					|| selected_layers.is_empty()
					|| transform_type.equivalent_to(self.transform_operation)
				{
					return;
				}
				let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) else {
					selected.original_transforms.clear();
					return;
				};
				self.proportional_edit_data = proportional_edit_data;
				self.initial_positions.clear();

				if let Some(_prop_data) = &self.proportional_edit_data {
					// Store positions of ALL points in selected layers, not just affected points
					for &layer in &selected_layers {
						if let Some(vector_data) = document.network_interface.compute_modified_vector(layer) {
							let layer_initial_positions = self.initial_positions.entry(layer).or_default();

							// Get all selected points in this layer to exclude them
							let selected_points: HashSet<PointId> = shape_editor
								.selected_points_in_layer(layer)
								.map(|points| points.iter().filter_map(|p| p.as_anchor()).collect())
								.unwrap_or_default();

							// Store point positions ONLY for unselected points
							for (i, &point_id) in vector_data.point_domain.ids().iter().enumerate() {
								// Skip points that are selected by the user
								if !selected_points.contains(&point_id) {
									let pos_local = vector_data.point_domain.positions()[i];
									layer_initial_positions.insert(point_id, pos_local);
								}
							}
						}
					}
				}

				if let [point] = selected_points.as_slice() {
					if matches!(point, ManipulatorPointId::Anchor(_)) {
						if let Some([handle1, handle2]) = point.get_handle_pair(&vector_data) {
							let handle1_length = handle1.length(&vector_data);
							let handle2_length = handle2.length(&vector_data);

							if (handle1_length == 0. && handle2_length == 0. && !using_select_tool) || (handle1_length == f64::MAX && handle2_length == f64::MAX && !using_select_tool) {
								// G should work for this point but not R and S
								if matches!(transform_type, TransformType::Rotate | TransformType::Scale) {
									selected.original_transforms.clear();
									return;
								}
							}
						}
					} else {
						let handle_length = point.as_handle().map(|handle| handle.length(&vector_data));

						if handle_length == Some(0.) {
							selected.original_transforms.clear();
							return;
						}
					}
				}

				let chain_operation = self.transform_operation != TransformOperation::None;
				if chain_operation {
					responses.add(TransformLayerMessage::ApplyTransformOperation { final_transform: false });
				} else {
					responses.add(OverlaysMessage::AddProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				}

				let response = match transform_type {
					TransformType::Grab => TransformLayerMessage::BeginGrab,
					TransformType::Rotate => TransformLayerMessage::BeginRotate,
					TransformType::Scale => TransformLayerMessage::BeginScale,
				};

				self.local = false;
				self.operation_count += 1;
				responses.add(response);
				responses.add(TransformLayerMessage::PointerMove {
					slow_key: SLOW_KEY,
					increments_key: INCREMENTS_KEY,
				});
			}
			TransformLayerMessage::BeginGrab => {
				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse, &mut self.initial_transform);
				self.transform_operation = TransformOperation::Grabbing(Default::default());
				self.layer_bounding_box = selected.bounding_box();
			}
			TransformLayerMessage::BeginRotate => {
				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse, &mut self.initial_transform);
				self.transform_operation = TransformOperation::Rotating(Default::default());
				self.layer_bounding_box = selected.bounding_box();
			}
			TransformLayerMessage::BeginScale => {
				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse, &mut self.initial_transform);
				self.transform_operation = TransformOperation::Scaling(Default::default());
				self.layer_bounding_box = selected.bounding_box();
			}
			TransformLayerMessage::CancelTransformOperation => {
				if using_pen_tool {
					self.typing.clear();

					self.last_point = DVec2::ZERO;
					self.transform_operation = TransformOperation::None;
					self.handle = DVec2::ZERO;

					responses.add(PenToolMessage::Abort);
					responses.add(ToolMessage::UpdateHints);
				} else {
					selected.original_transforms.clear();
					self.typing.clear();
					self.transform_operation = TransformOperation::None;

					responses.add(DocumentMessage::RepeatedAbortTransaction { undo_count: self.operation_count });
					self.operation_count = 0;
					responses.add(ToolMessage::UpdateHints);
				}
				self.proportional_edit_data = None;
				self.initial_positions.clear();
				responses.add(OverlaysMessage::RemoveProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
			}
			TransformLayerMessage::ConstrainX => {
				let pivot = document_to_viewport.transform_point2(self.local_pivot);
				self.local = self.transform_operation.constrain_axis(
					Axis::X,
					&mut selected,
					self.increments,
					self.local,
					self.layer_bounding_box,
					document_to_viewport,
					pivot,
					self.initial_transform,
				);
				self.transform_operation.grs_typed(
					self.typing.evaluate(),
					&mut selected,
					self.increments,
					self.local,
					self.layer_bounding_box,
					document_to_viewport,
					pivot,
					self.initial_transform,
				);
			}
			TransformLayerMessage::ConstrainY => {
				let pivot = document_to_viewport.transform_point2(self.local_pivot);
				self.local = self.transform_operation.constrain_axis(
					Axis::Y,
					&mut selected,
					self.increments,
					self.local,
					self.layer_bounding_box,
					document_to_viewport,
					pivot,
					self.initial_transform,
				);
				self.transform_operation.grs_typed(
					self.typing.evaluate(),
					&mut selected,
					self.increments,
					self.local,
					self.layer_bounding_box,
					document_to_viewport,
					pivot,
					self.initial_transform,
				);
			}
			TransformLayerMessage::PointerMove { slow_key, increments_key } => {
				self.slow = input.keyboard.get(slow_key as usize);
				let old_ptz = self.ptz;
				self.ptz = document.document_ptz;
				if old_ptz != self.ptz {
					self.mouse_position = input.mouse.position;
					return;
				}

				let pivot = document_to_viewport.transform_point2(self.local_pivot);

				let new_increments = input.keyboard.get(increments_key as usize);
				if new_increments != self.increments {
					self.increments = new_increments;
					self.transform_operation
						.apply_transform_operation(&mut selected, self.increments, self.local, self.layer_bounding_box, document_to_viewport, pivot, self.initial_transform);
				}

				if self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let delta_pos = input.mouse.position - self.mouse_position;
							let delta_pos = (self.initial_transform * document_to_viewport.inverse()).transform_vector2(delta_pos);
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation.apply_transform_operation(
								&mut selected,
								self.increments,
								self.local,
								self.layer_bounding_box,
								document_to_viewport,
								pivot,
								self.initial_transform,
							);
						}
						TransformOperation::Rotating(rotation) => {
							let start_offset = pivot - self.mouse_position;
							let end_offset = pivot - input.mouse.position;
							let angle = start_offset.angle_to(end_offset);

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };

							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation.apply_transform_operation(
								&mut selected,
								self.increments,
								self.local,
								self.layer_bounding_box,
								document_to_viewport,
								pivot,
								self.initial_transform,
							);
						}
						TransformOperation::Scaling(mut scale) => {
							let axis_constraint = scale.constraint;
							let to_mouse_final = self.mouse_position - pivot;
							let to_mouse_final_old = input.mouse.position - pivot;
							let to_mouse_start = self.start_mouse - pivot;

							let to_mouse_final = project_edge_to_quad(to_mouse_final, &self.layer_bounding_box, self.local, axis_constraint);
							let to_mouse_final_old = project_edge_to_quad(to_mouse_final_old, &self.layer_bounding_box, self.local, axis_constraint);
							let to_mouse_start = project_edge_to_quad(to_mouse_start, &self.layer_bounding_box, self.local, axis_constraint);

							let change = {
								let previous_frame_dist = to_mouse_final.dot(to_mouse_start);
								let current_frame_dist = to_mouse_final_old.dot(to_mouse_start);
								let start_transform_dist = to_mouse_start.length_squared();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};
							let change = if self.slow { change / SLOWING_DIVISOR } else { change };

							scale = scale.increment_amount(change);
							self.transform_operation = TransformOperation::Scaling(scale);
							self.transform_operation.apply_transform_operation(
								&mut selected,
								self.increments,
								self.local,
								self.layer_bounding_box,
								document_to_viewport,
								pivot,
								self.initial_transform,
							);
						}
					};
				}
				let pivot_vp = document_to_viewport.transform_point2(self.local_pivot);
				let local_axis_transform_angle = (self.layer_bounding_box.0[1] - self.layer_bounding_box.0[0]).to_angle();
				let total_transformation_vp = match self.transform_operation {
					TransformOperation::Grabbing(translation) => {
						let total_delta_doc = translation.to_dvec(self.initial_transform, self.increments);
						let translate = DAffine2::from_translation(document_to_viewport.transform_vector2(total_delta_doc));
						if self.local {
							let resolved_angle = if local_axis_transform_angle > 0. {
								local_axis_transform_angle
							} else {
								local_axis_transform_angle - std::f64::consts::PI
							};
							DAffine2::from_angle(resolved_angle) * translate * DAffine2::from_angle(-resolved_angle)
						} else {
							translate
						}
					}
					TransformOperation::Rotating(rotation) => {
						let total_angle = rotation.to_f64(self.increments);
						let pivot_transform = DAffine2::from_translation(pivot_vp);
						pivot_transform * DAffine2::from_angle(total_angle) * pivot_transform.inverse()
					}
					TransformOperation::Scaling(scale) => {
						let total_scale_vec = scale.to_dvec(self.increments);
						let pivot_transform = DAffine2::from_translation(pivot_vp);
						if self.local {
							pivot_transform
								* DAffine2::from_angle(local_axis_transform_angle)
								* DAffine2::from_scale(total_scale_vec)
								* DAffine2::from_angle(-local_axis_transform_angle)
								* pivot_transform.inverse()
						} else {
							pivot_transform * DAffine2::from_scale(total_scale_vec) * pivot_transform.inverse()
						}
					}
					TransformOperation::None => DAffine2::IDENTITY,
				};

				// selected.apply_transformation(total_transformation_vp, Some(self.transform_operation));

				if let Some(prop_data) = &self.proportional_edit_data {
					apply_proportional_edit(&self.initial_positions, prop_data, total_transformation_vp, &document.network_interface, document.metadata(), responses);
				}
				self.mouse_position = input.mouse.position;
			}
			TransformLayerMessage::SelectionChanged => {
				let target_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);
			}
			TransformLayerMessage::TypeDecimalPoint => {
				let pivot = document_to_viewport.transform_point2(self.local_pivot);
				if self.transform_operation.can_begin_typing() {
					self.transform_operation.grs_typed(
						self.typing.type_decimal_point(),
						&mut selected,
						self.increments,
						self.local,
						self.layer_bounding_box,
						document_to_viewport,
						pivot,
						self.initial_transform,
					);

					// Apply proportional editing
					let total_transformation_vp = self.calculate_total_transformation_vp(document_to_viewport);
					self.apply_proportional_editing(total_transformation_vp, document, responses);
				}
			}

			TransformLayerMessage::TypeBackspace => {
				let pivot = document_to_viewport.transform_point2(self.local_pivot);
				if self.typing.digits.is_empty() && self.typing.negative {
					self.transform_operation
						.negate(&mut selected, self.increments, self.local, self.layer_bounding_box, document_to_viewport, pivot, self.initial_transform);
					self.typing.type_negate();
				}
				self.transform_operation.grs_typed(
					self.typing.type_backspace(),
					&mut selected,
					self.increments,
					self.local,
					self.layer_bounding_box,
					document_to_viewport,
					pivot,
					self.initial_transform,
				);

				// Apply proportional editing
				let total_transformation_vp = self.calculate_total_transformation_vp(document_to_viewport);
				self.apply_proportional_editing(total_transformation_vp, document, responses);
			}

			TransformLayerMessage::TypeDigit { digit } => {
				if self.transform_operation.can_begin_typing() {
					let pivot = document_to_viewport.transform_point2(self.local_pivot);
					self.transform_operation.grs_typed(
						self.typing.type_number(digit),
						&mut selected,
						self.increments,
						self.local,
						self.layer_bounding_box,
						document_to_viewport,
						pivot,
						self.initial_transform,
					);

					// Calculate total transformation and apply proportional editing
					let total_transformation_vp = self.calculate_total_transformation_vp(document_to_viewport);
					self.apply_proportional_editing(total_transformation_vp, document, responses);
				}
			}
			TransformLayerMessage::TypeNegate => {
				let pivot = document_to_viewport.transform_point2(self.local_pivot);
				if self.typing.digits.is_empty() {
					self.transform_operation
						.negate(&mut selected, self.increments, self.local, self.layer_bounding_box, document_to_viewport, pivot, self.initial_transform);
				}
				self.transform_operation.grs_typed(
					self.typing.type_negate(),
					&mut selected,
					self.increments,
					self.local,
					self.layer_bounding_box,
					document_to_viewport,
					pivot,
					self.initial_transform,
				);

				// Apply proportional editing
				let total_transformation_vp = self.calculate_total_transformation_vp(document_to_viewport);
				self.apply_proportional_editing(total_transformation_vp, document, responses);
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(TransformLayerMessageDiscriminant;
			BeginGRS,
		);

		if self.transform_operation != TransformOperation::None {
			let active = actions!(TransformLayerMessageDiscriminant;
				PointerMove,
				CancelTransformOperation,
				ApplyTransformOperation,
				TypeDigit,
				TypeBackspace,
				TypeDecimalPoint,
				TypeNegate,
				ConstrainX,
				ConstrainY,
				UpdateProportionalEditData
			);
			common.extend(active);
		}

		common
	}
}

#[cfg(test)]
mod test_transform_layer {
	use crate::messages::{
		portfolio::document::graph_operation::{
			transform_utils,
			utility_types::{ModifyInputsContext, TransformIn},
		},
		prelude::Message,
		tool::transform_layer::transform_layer_message_handler::VectorModificationType,
	};
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_core::vector::PointId;
	use std::collections::VecDeque;

	async fn get_layer_transform(editor: &mut EditorTestUtils, layer: LayerNodeIdentifier) -> Option<DAffine2> {
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		let _responses: VecDeque<Message> = VecDeque::new();
		let transform_node_id = ModifyInputsContext::locate_node_in_layer_chain("Transform", layer, network_interface)?;
		let document_node = network_interface.document_network().nodes.get(&transform_node_id)?;
		Some(transform_utils::get_current_transform(&document_node.inputs))
	}

	#[tokio::test]
	async fn test_grab_apply() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(TransformLayerMessage::BeginGrab).await;

		let translation = DVec2::new(50.0, 50.0);
		editor.move_mouse(translation.x, translation.y, ModifierKeys::empty(), MouseKeys::NONE).await;

		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let translation_diff = (final_transform.translation - original_transform.translation).length();
		assert!(translation_diff > 10.0, "Transform should have changed after applying transformation. Diff: {}", translation_diff);
	}

	#[tokio::test]
	async fn test_grab_cancel() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();
		let original_transform = get_layer_transform(&mut editor, layer).await.expect("Should be able to get the layer transform");

		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(50.0, 50.0, ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		let during_transform = get_layer_transform(&mut editor, layer).await.expect("Should be able to get the layer transform during operation");

		assert!(original_transform != during_transform, "Transform should change during operation");

		editor.handle_message(TransformLayerMessage::CancelTransformOperation).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.expect("Should be able to get the final transform");
		let final_translation = final_transform.translation;
		let original_translation = original_transform.translation;

		// Verify transform is either restored to original OR reset to identity
		assert!(
			(final_translation - original_translation).length() < 5.0 || final_translation.length() < 0.001,
			"Transform neither restored to original nor reset to identity. Original: {:?}, Final: {:?}",
			original_translation,
			final_translation
		);
	}

	#[tokio::test]
	async fn test_rotate_apply() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(TransformLayerMessage::BeginRotate).await;

		editor.move_mouse(150.0, 50.0, ModifierKeys::empty(), MouseKeys::NONE).await;

		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		println!("Final transform: {:?}", final_transform);

		// Check matrix components have changed (rotation affects matrix2)
		let matrix_diff = (final_transform.matrix2.x_axis - original_transform.matrix2.x_axis).length();
		assert!(matrix_diff > 0.1, "Rotation should have changed the transform matrix. Diff: {}", matrix_diff);
	}

	#[tokio::test]
	async fn test_rotate_cancel() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();
		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(TransformLayerMessage::BeginRotate).await;
		editor.handle_message(TransformLayerMessage::CancelTransformOperation).await;

		let after_cancel = get_layer_transform(&mut editor, layer).await.unwrap();

		assert!(!after_cancel.translation.x.is_nan(), "Transform is NaN after cancel");
		assert!(!after_cancel.translation.y.is_nan(), "Transform is NaN after cancel");

		let translation_diff = (after_cancel.translation - original_transform.translation).length();
		assert!(translation_diff < 1.0, "Translation component changed too much: {}", translation_diff);
	}

	#[tokio::test]
	async fn test_scale_apply() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(TransformLayerMessage::BeginScale).await;

		editor.move_mouse(150.0, 150.0, ModifierKeys::empty(), MouseKeys::NONE).await;

		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		// Check scaling components have changed
		let scale_diff_x = (final_transform.matrix2.x_axis.x - original_transform.matrix2.x_axis.x).abs();
		let scale_diff_y = (final_transform.matrix2.y_axis.y - original_transform.matrix2.y_axis.y).abs();

		assert!(
			scale_diff_x > 0.1 || scale_diff_y > 0.1,
			"Scaling should have changed the transform matrix. Diffs: x={}, y={}",
			scale_diff_x,
			scale_diff_y
		);
	}

	#[tokio::test]
	async fn test_scale_cancel() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();
		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(TransformLayerMessage::BeginScale).await;

		// Cancel immediately without moving to ensure proper reset
		editor.handle_message(TransformLayerMessage::CancelTransformOperation).await;

		let after_cancel = get_layer_transform(&mut editor, layer).await.unwrap();

		// The scale factor is represented in the matrix2 part, so check those components
		assert!(
			(after_cancel.matrix2.x_axis.x - original_transform.matrix2.x_axis.x).abs() < 0.1 && (after_cancel.matrix2.y_axis.y - original_transform.matrix2.y_axis.y).abs() < 0.1,
			"Matrix scale components should be restored after cancellation"
		);

		// Also check translation component is similar
		let translation_diff = (after_cancel.translation - original_transform.translation).length();
		assert!(translation_diff < 1.0, "Translation component changed too much: {}", translation_diff);
	}

	#[tokio::test]
	async fn test_grab_rotate_scale_chained() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;
		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(150.0, 130.0, ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		let after_grab_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		let expected_translation = DVec2::new(50.0, 30.0);
		let actual_translation = after_grab_transform.translation - original_transform.translation;
		assert!(
			(actual_translation - expected_translation).length() < 1e-5,
			"Expected translation of {:?}, got {:?}",
			expected_translation,
			actual_translation
		);

		// 2. Chain to rotation - from current position to create ~45 degree rotation
		editor.handle_message(TransformLayerMessage::BeginRotate).await;
		editor.move_mouse(190.0, 90.0, ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		let after_rotate_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		// Checking for off-diagonal elements close to 0.707, which corresponds to cos(45°) and sin(45°)
		assert!(
			!after_rotate_transform.matrix2.abs_diff_eq(after_grab_transform.matrix2, 1e-5) &&
			(after_rotate_transform.matrix2.x_axis.y.abs() - 0.707).abs() < 0.1 &&  // Check for off-diagonal elements close to 0.707
			(after_rotate_transform.matrix2.y_axis.x.abs() - 0.707).abs() < 0.1, // that would indicate ~45° rotation
			"Rotation should change matrix components with approximately 45° rotation"
		);

		// 3. Chain to scaling - scale(area) up by 2x
		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.move_mouse(250.0, 200.0, ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		let after_scale_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		let before_scale_det = after_rotate_transform.matrix2.determinant();
		let after_scale_det = after_scale_transform.matrix2.determinant();
		assert!(
			after_scale_det >= 2.0 * before_scale_det,
			"Scale should increase the determinant of the matrix (before: {}, after: {})",
			before_scale_det,
			after_scale_det
		);

		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		assert!(final_transform.abs_diff_eq(after_scale_transform, 1e-5), "Final transform should match the transform before committing");
		assert!(!final_transform.abs_diff_eq(original_transform, 1e-5), "Final transform should be different from original transform");
	}

	#[tokio::test]
	async fn test_scale_with_panned_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let pan_amount = DVec2::new(200.0, 150.0);
		editor.handle_message(NavigationMessage::CanvasPan { delta: pan_amount }).await;

		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let scale_x = final_transform.matrix2.x_axis.length() / original_transform.matrix2.x_axis.length();
		let scale_y = final_transform.matrix2.y_axis.length() / original_transform.matrix2.y_axis.length();

		assert!((scale_x - 2.0).abs() < 0.1, "Expected scale factor X of 2.0, got: {}", scale_x);
		assert!((scale_y - 2.0).abs() < 0.1, "Expected scale factor Y of 2.0, got: {}", scale_y);
	}

	#[tokio::test]
	async fn test_scale_with_zoomed_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		editor.handle_message(NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }).await;
		editor.handle_message(NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }).await;

		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let scale_x = final_transform.matrix2.x_axis.length() / original_transform.matrix2.x_axis.length();
		let scale_y = final_transform.matrix2.y_axis.length() / original_transform.matrix2.y_axis.length();

		assert!((scale_x - 2.0).abs() < 0.1, "Expected scale factor X of 2.0, got: {}", scale_x);
		assert!((scale_y - 2.0).abs() < 0.1, "Expected scale factor Y of 2.0, got: {}", scale_y);
	}

	#[tokio::test]
	async fn test_rotate_with_rotated_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let original_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		// Rotate the document view (45 degrees)
		editor.handle_message(NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: false }).await;
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 45.0_f64.to_radians() }).await;
		editor.handle_message(TransformLayerMessage::BeginRotate).await;

		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 9 }).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 0 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let original_angle = original_transform.to_scale_angle_translation().1;
		let final_angle = final_transform.to_scale_angle_translation().1;
		let angle_change = (final_angle - original_angle).to_degrees();

		// Normalize angle between 0 and 360
		let angle_change = ((angle_change % 360.0) + 360.0) % 360.0;
		assert!((angle_change - 90.0).abs() < 0.1, "Expected rotation of 90 degrees, got: {}", angle_change);
	}

	#[tokio::test]
	async fn test_grs_single_anchor() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let point_id = PointId::generate();
		let modification_type = VectorModificationType::InsertPoint {
			id: point_id,
			position: DVec2::new(100.0, 100.0),
		};
		editor.handle_message(GraphOperationMessage::Vector { layer, modification_type }).await;
		editor.handle_message(ToolMessage::ActivateTool { tool_type: ToolType::Select }).await;

		// Testing grab operation - just checking that it doesn't crash.
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(150.0, 150.0, ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await;
		assert!(final_transform.is_some(), "Transform node should exist after grab operation");
	}
	#[tokio::test]
	async fn test_scale_to_zero_then_rescale() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		// First scale to near-zero
		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 0 }).await;
		editor.handle_message(TransformLayerMessage::TypeDecimalPoint).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 0 }).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 0 }).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 0 }).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 1 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let near_zero_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		// Verify scale is near zero.
		let scale_x = near_zero_transform.matrix2.x_axis.length();
		let scale_y = near_zero_transform.matrix2.y_axis.length();
		assert!(scale_x < 0.001, "Scale factor X should be near zero, got: {}", scale_x);
		assert!(scale_y < 0.001, "Scale factor Y should be near zero, got: {}", scale_y);
		assert!(scale_x > 0.0, "Scale factor X should not be exactly zero");
		assert!(scale_y > 0.0, "Scale factor Y should not be exactly zero");

		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		assert!(final_transform.is_finite(), "Transform should be finite after rescaling");

		let new_scale_x = final_transform.matrix2.x_axis.length();
		let new_scale_y = final_transform.matrix2.y_axis.length();
		assert!(new_scale_x > 0.0, "After rescaling, scale factor X should be non-zero");
		assert!(new_scale_y > 0.0, "After rescaling, scale factor Y should be non-zero");
	}
}
