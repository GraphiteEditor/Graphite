use crate::consts::{ANGLE_MEASURE_RADIUS_FACTOR, ARC_MEASURE_RADIUS_FACTOR_RANGE, COLOR_OVERLAY_BLUE, SLOWING_DIVISOR};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::{ToolData, ToolType};

use graphene_core::renderer::Quad;
use graphene_core::vector::ManipulatorPointId;
use graphene_std::vector::VectorData;

use glam::{DAffine2, DVec2};
use std::f64::consts::TAU;

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
	fixed_bbox: Quad,
	typing: Typing,

	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	pivot: DVec2,
	grab_target: DVec2,

	// Pen tool (outgoing handle GRS manipulation)
	handle: DVec2,
	last_point: DVec2,
	grs_pen_handle: bool,
}

impl TransformLayerMessageHandler {
	pub fn is_transforming(&self) -> bool {
		self.transform_operation != TransformOperation::None
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>) {
		self.transform_operation.hints(responses, self.local);
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

type TransformData<'a> = (&'a DocumentMessageHandler, &'a InputPreprocessorMessageHandler, &'a ToolData, &'a mut ShapeState);
impl MessageHandler<TransformLayerMessage, TransformData<'_>> for TransformLayerMessageHandler {
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, (document, input, tool_data, shape_editor): TransformData) {
		let using_path_tool = tool_data.active_tool_type == ToolType::Path;
		let using_select_tool = tool_data.active_tool_type == ToolType::Select;
		let using_pen_tool = tool_data.active_tool_type == ToolType::Pen;

		// TODO: Add support for transforming layer not in the document network
		let selected_layers = document
			.network_interface
			.selected_nodes(&[])
			.unwrap()
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

		let mut begin_operation = |operation: TransformOperation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2| {
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
				self.grab_target = selected.mean_average_of_pivots();
			} else if let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
				*selected.original_transforms = OriginalTransforms::default();

				let viewspace = document.metadata().transform_to_viewport(selected_layers[0]);
				let selected_points = shape_editor.selected_points().collect::<Vec<_>>();

				let get_location = |point: &&ManipulatorPointId| point.get_position(&vector_data).map(|position| viewspace.transform_point2(position));
				if let Some((new_pivot, grab_target)) = calculate_pivot(&selected_points, &vector_data, viewspace, |point: &ManipulatorPointId| get_location(&point)) {
					*selected.pivot = new_pivot;

					self.grab_target = grab_target;
				} else {
					log::warn!("Failed to calculate pivot.");
				}
			}

			*mouse_position = input.mouse.position;
			*start_mouse = input.mouse.position;
			selected.original_transforms.clear();

			selected.responses.add(DocumentMessage::StartTransaction);
		};
		let document_to_viewport = document.metadata().document_to_viewport;

		match message {
			// Overlays
			TransformLayerMessage::Overlays(mut overlay_context) => {
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

					match self.transform_operation {
						TransformOperation::None => (),
						TransformOperation::Grabbing(translation) => {
							let translation = translation.to_dvec(document_to_viewport, self.increments);
							let viewport_translate = document_to_viewport.transform_vector2(translation);
							let quad = Quad::from_box([self.grab_target, self.grab_target + viewport_translate]).0;
							let e1 = (self.fixed_bbox.0[1] - self.fixed_bbox.0[0]).normalize();

							if matches!(axis_constraint, Axis::Both | Axis::X) && translation.x != 0. {
								let end = if self.local {
									(quad[1] - quad[0]).length() * e1 * e1.dot(quad[1] - quad[0]).signum() + quad[0]
								} else {
									quad[1]
								};
								overlay_context.line(quad[0], end, None);

								let x_transform = DAffine2::from_translation((quad[0] + end) / 2.);
								overlay_context.text(&format_rounded(translation.x, 3), COLOR_OVERLAY_BLUE, None, x_transform, 4., [Pivot::Middle, Pivot::End]);
							}

							if matches!(axis_constraint, Axis::Both | Axis::Y) && translation.y != 0. {
								let end = if self.local {
									(quad[3] - quad[0]).length() * e1.perp() * e1.perp().dot(quad[3] - quad[0]).signum() + quad[0]
								} else {
									quad[3]
								};
								overlay_context.line(quad[0], end, None);
								let x_parameter = viewport_translate.x.clamp(-1., 1.);
								let y_transform = DAffine2::from_translation((quad[0] + end) / 2. + x_parameter * DVec2::X * 0.);
								let pivot_selection = if x_parameter >= 0. { Pivot::Start } else { Pivot::End };
								if axis_constraint != Axis::Both || self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
									overlay_context.text(&format_rounded(translation.y, 2), COLOR_OVERLAY_BLUE, None, y_transform, 3., [pivot_selection, Pivot::Middle]);
								}
							}
							if matches!(axis_constraint, Axis::Both) && translation.x != 0. && translation.y != 0. {
								overlay_context.dashed_line(quad[1], quad[2], None, Some(2.), Some(2.), Some(0.5));
								overlay_context.dashed_line(quad[3], quad[2], None, Some(2.), Some(2.), Some(0.5));
							}
						}
						TransformOperation::Scaling(scale) => {
							let scale = scale.to_f64(self.increments);
							let text = format!("{}x", format_rounded(scale, 3));
							let local_edge = self.start_mouse - self.pivot;
							let local_edge = project_edge_to_quad(local_edge, &self.fixed_bbox, self.local, axis_constraint);
							let boundary_point = self.pivot + local_edge * scale.min(1.);
							let end_point = self.pivot + local_edge * scale.max(1.);

							if scale > 0. {
								overlay_context.dashed_line(self.pivot, boundary_point, None, Some(4.), Some(4.), Some(0.5));
							}
							overlay_context.line(boundary_point, end_point, None);

							let transform = DAffine2::from_translation(boundary_point.midpoint(self.pivot) + local_edge.perp().normalize() * local_edge.element_product().signum() * 24.);
							overlay_context.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
						}
						TransformOperation::Rotating(rotation) => {
							let angle = rotation.to_f64(self.increments);
							let offset_angle = if self.grs_pen_handle {
								self.handle - self.last_point
							} else if using_path_tool {
								self.start_mouse - self.pivot
							} else {
								self.fixed_bbox.top_right() - self.fixed_bbox.top_right()
							};
							let offset_angle = offset_angle.to_angle();
							let width = viewport_box.max_element();
							let radius = self.start_mouse.distance(self.pivot);
							let arc_radius = ANGLE_MEASURE_RADIUS_FACTOR * width;
							let radius = radius.clamp(ARC_MEASURE_RADIUS_FACTOR_RANGE.0 * width, ARC_MEASURE_RADIUS_FACTOR_RANGE.1 * width);
							let text = format!("{}°", format_rounded(angle.to_degrees(), 2));
							let text_texture_width = overlay_context.get_width(&text) / 2.;
							let text_texture_height = 12.;
							let text_angle_on_unit_circle = DVec2::from_angle((angle % TAU) / 2. + offset_angle);
							let text_texture_position = DVec2::new(
								(arc_radius + 4. + text_texture_width) * text_angle_on_unit_circle.x,
								(arc_radius + text_texture_height) * text_angle_on_unit_circle.y,
							);
							let transform = DAffine2::from_translation(text_texture_position + self.pivot);
							overlay_context.draw_angle(self.pivot, radius, arc_radius, offset_angle, angle);
							overlay_context.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
						}
					}
				}
			}

			// Messages
			TransformLayerMessage::ApplyTransformOperation => {
				selected.original_transforms.clear();
				self.typing.clear();
				self.transform_operation = TransformOperation::None;

				if using_pen_tool {
					self.last_point = DVec2::ZERO;
					self.grs_pen_handle = false;

					selected.pen_handle = None;

					selected.responses.add(PenToolMessage::Confirm);
				} else {
					responses.add(DocumentMessage::EndTransaction);
					responses.add(ToolMessage::UpdateHints);
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				responses.add(OverlaysMessage::RemoveProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
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
				self.fixed_bbox = Quad::from_box([top_left, bottom_right]);
				self.grab_target = handle;
				self.pivot = last_point;
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
			TransformLayerMessage::BeginGrab => {
				if (!using_path_tool && !using_select_tool && !using_pen_tool)
					|| (using_path_tool && shape_editor.selected_points().next().is_none())
					|| selected_layers.is_empty()
					|| matches!(self.transform_operation, TransformOperation::Grabbing(_))
				{
					selected.original_transforms.clear();

					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Grabbing(Default::default());
				self.local = false;
				self.fixed_bbox = selected.bounding_box();

				selected.original_transforms.clear();

				responses.add(OverlaysMessage::AddProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				responses.add(TransformLayerMessage::PointerMove {
					slow_key: SLOW_KEY,
					increments_key: INCREMENTS_KEY,
				});
			}
			TransformLayerMessage::BeginRotate => {
				let selected_points: Vec<&ManipulatorPointId> = shape_editor.selected_points().collect();

				if (!using_path_tool && !using_select_tool && !using_pen_tool)
					|| (using_path_tool && selected_points.is_empty())
					|| selected_layers.is_empty()
					|| matches!(self.transform_operation, TransformOperation::Rotating(_))
				{
					selected.original_transforms.clear();
					return;
				}

				let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) else {
					selected.original_transforms.clear();
					return;
				};

				if let [point] = selected_points.as_slice() {
					if matches!(point, ManipulatorPointId::Anchor(_)) {
						if let Some([handle1, handle2]) = point.get_handle_pair(&vector_data) {
							let handle1_length = handle1.length(&vector_data);
							let handle2_length = handle2.length(&vector_data);

							if (handle1_length == 0. && handle2_length == 0.) || (handle1_length == f64::MAX && handle2_length == f64::MAX) {
								return;
							}
						}
					} else {
						// TODO: Fix handle snap to anchor issue, see <https://discord.com/channels/731730685944922173/1217752903209713715>

						let handle_length = point.as_handle().map(|handle| handle.length(&vector_data));

						if handle_length == Some(0.) {
							selected.original_transforms.clear();
							return;
						}
					}
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Rotating(Default::default());

				self.local = false;
				self.fixed_bbox = selected.bounding_box();

				selected.original_transforms.clear();

				responses.add(OverlaysMessage::AddProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				responses.add(TransformLayerMessage::PointerMove {
					slow_key: SLOW_KEY,
					increments_key: INCREMENTS_KEY,
				});
			}
			TransformLayerMessage::BeginScale => {
				let selected_points: Vec<&ManipulatorPointId> = shape_editor.selected_points().collect();

				if (using_path_tool && selected_points.is_empty())
					|| (!using_path_tool && !using_select_tool && !using_pen_tool)
					|| selected_layers.is_empty()
					|| matches!(self.transform_operation, TransformOperation::Scaling(_))
				{
					selected.original_transforms.clear();
					return;
				}

				let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) else {
					selected.original_transforms.clear();
					return;
				};

				if let [point] = selected_points.as_slice() {
					if matches!(point, ManipulatorPointId::Anchor(_)) {
						if let Some([handle1, handle2]) = point.get_handle_pair(&vector_data) {
							let handle1_length = handle1.length(&vector_data);
							let handle2_length = handle2.length(&vector_data);

							if (handle1_length == 0. && handle2_length == 0.) || (handle1_length == f64::MAX && handle2_length == f64::MAX) {
								selected.original_transforms.clear();
								return;
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

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Scaling(Default::default());

				self.local = false;
				self.fixed_bbox = selected.bounding_box();

				selected.original_transforms.clear();

				responses.add(OverlaysMessage::AddProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				responses.add(TransformLayerMessage::PointerMove {
					slow_key: SLOW_KEY,
					increments_key: INCREMENTS_KEY,
				});
			}
			TransformLayerMessage::CancelTransformOperation => {
				if using_pen_tool {
					self.typing.clear();

					self.last_point = DVec2::ZERO;
					self.transform_operation = TransformOperation::None;
					self.handle = DVec2::ZERO;

					responses.add(PenToolMessage::Abort);
				} else {
					selected.revert_operation();
					selected.original_transforms.clear();
					self.typing.clear();
					self.transform_operation = TransformOperation::None;

					responses.add(DocumentMessage::AbortTransaction);
					responses.add(ToolMessage::UpdateHints);
				}

				responses.add(OverlaysMessage::RemoveProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
			}
			TransformLayerMessage::ConstrainX => {
				self.local = self
					.transform_operation
					.constrain_axis(Axis::X, &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
				self.transform_operation
					.grs_typed(self.typing.evaluate(), &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
			}
			TransformLayerMessage::ConstrainY => {
				self.local = self
					.transform_operation
					.constrain_axis(Axis::Y, &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
				self.transform_operation
					.grs_typed(self.typing.evaluate(), &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
			}
			TransformLayerMessage::PointerMove { slow_key, increments_key } => {
				self.slow = input.keyboard.get(slow_key as usize);

				let new_increments = input.keyboard.get(increments_key as usize);
				if new_increments != self.increments {
					self.increments = new_increments;
					self.transform_operation
						.apply_transform_operation(&mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
				}

				if self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
					let delta_pos = input.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
						}
						TransformOperation::Rotating(rotation) => {
							let start_offset = *selected.pivot - self.mouse_position;
							let end_offset = *selected.pivot - input.mouse.position;
							let angle = start_offset.angle_to(end_offset);

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };

							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
						}
						TransformOperation::Scaling(mut scale) => {
							let axis_constraint = scale.constraint;
							let to_mouse_final = self.mouse_position - *selected.pivot;
							let to_mouse_final_old = input.mouse.position - *selected.pivot;
							let to_mouse_start = self.start_mouse - *selected.pivot;

							let to_mouse_final = project_edge_to_quad(to_mouse_final, &self.fixed_bbox, self.local, axis_constraint);
							let to_mouse_final_old = project_edge_to_quad(to_mouse_final_old, &self.fixed_bbox, self.local, axis_constraint);
							let to_mouse_start = project_edge_to_quad(to_mouse_start, &self.fixed_bbox, self.local, axis_constraint);

							let change = {
								let previous_frame_dist = to_mouse_final.dot(to_mouse_start);
								let current_frame_dist = to_mouse_final_old.dot(to_mouse_start);
								let start_transform_dist = to_mouse_start.length_squared();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};
							let change = if self.slow { change / SLOWING_DIVISOR } else { change };

							scale = scale.increment_amount(change);
							self.transform_operation = TransformOperation::Scaling(scale);
							self.transform_operation
								.apply_transform_operation(&mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
						}
					};
				}

				self.mouse_position = input.mouse.position;
			}
			TransformLayerMessage::SelectionChanged => {
				let target_layers = document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);
			}
			TransformLayerMessage::TypeBackspace => {
				if self.typing.digits.is_empty() && self.typing.negative {
					self.transform_operation.negate(&mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
					self.typing.type_negate();
				}
				self.transform_operation
					.grs_typed(self.typing.type_backspace(), &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
			}
			TransformLayerMessage::TypeDecimalPoint => {
				if self.transform_operation.can_begin_typing() {
					self.transform_operation
						.grs_typed(self.typing.type_decimal_point(), &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport)
				}
			}
			TransformLayerMessage::TypeDigit { digit } => {
				if self.transform_operation.can_begin_typing() {
					self.transform_operation
						.grs_typed(self.typing.type_number(digit), &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport)
				}
			}
			TransformLayerMessage::TypeNegate => {
				if self.typing.digits.is_empty() {
					self.transform_operation.negate(&mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport);
				}
				self.transform_operation
					.grs_typed(self.typing.type_negate(), &mut selected, self.increments, self.local, self.fixed_bbox, document_to_viewport)
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(TransformLayerMessageDiscriminant;
			BeginGrab,
			BeginScale,
			BeginRotate,
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
			);
			common.extend(active);
		}

		common
	}
}
