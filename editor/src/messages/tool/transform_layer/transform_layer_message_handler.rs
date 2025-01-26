use crate::consts::{ANGLE_MEASURE_RADIUS_FACTOR, ARC_MEASURE_RADIUS_FACTOR_RANGE, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_SNAP_BACKGROUND, COLOR_OVERLAY_WHITE, SLOWING_DIVISOR};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::{ToolData, ToolType};

use graphene_core::renderer::Quad;
use graphene_core::vector::ManipulatorPointId;

use glam::{DAffine2, DVec2};
use graphene_std::vector::VectorData;
use std::f64::consts::TAU;

const TRANSFORM_GRS_OVERLAY_PROVIDER: OverlayProvider = |context| TransformLayerMessage::Overlays(context).into();

#[derive(Debug, Clone, Default)]
pub struct TransformLayerMessageHandler {
	pub transform_operation: TransformOperation,

	slow: bool,
	snap: bool,
	local: bool,
	fixed_bbox: Quad,
	typing: Typing,

	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	pivot: DVec2,
	grab_target: DVec2,

	// pen-tool
	handle: DVec2,
	last_point: DVec2,
	grs_pen_handle: bool,
}

impl TransformLayerMessageHandler {
	pub fn is_transforming(&self) -> bool {
		self.transform_operation != TransformOperation::None
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>) {
		self.transform_operation.hints(responses);
	}
}

fn calculate_pivot(selected_points: &Vec<&ManipulatorPointId>, vector_data: &VectorData, viewspace: DAffine2, get_location: impl Fn(&ManipulatorPointId) -> Option<DVec2>) -> Option<(DVec2, DVec2)> {
	if let [point] = selected_points.as_slice() {
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
	} else {
		// Handle the case where there are multiple points
		let mut point_count = 0;
		let average_position = selected_points.iter().filter_map(|p| get_location(p)).inspect(|_| point_count += 1).sum::<DVec2>() / point_count as f64;
		Some((average_position, average_position))
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

			if using_path_tool {
				if let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
					*selected.original_transforms = OriginalTransforms::default();
					let viewspace = document.metadata().transform_to_viewport(selected_layers[0]);
					let get_location = |point: &&ManipulatorPointId| point.get_position(&vector_data).map(|position| viewspace.transform_point2(position));
					let points = shape_editor.selected_points();
					let selected_points: Vec<&ManipulatorPointId> = points.collect();

					if let Some((new_pivot, grab_target)) = calculate_pivot(&selected_points, &vector_data, viewspace, |arg0: &ManipulatorPointId| get_location(&arg0)) {
						*selected.pivot = new_pivot;
						self.grab_target = grab_target;
					} else {
						log::warn!("Failed to calculate pivot.");
					}
				}
			} else {
				*selected.pivot = selected.mean_average_of_pivots();
			}

			*mouse_position = input.mouse.position;
			*start_mouse = input.mouse.position;
			selected.original_transforms.clear();

			selected.responses.add(DocumentMessage::StartTransaction);
		};
		let document_to_viewport = document.metadata().document_to_viewport;

		match message {
			TransformLayerMessage::ApplyTransformOperation => {
				selected.original_transforms.clear();
				self.typing.clear();
				self.transform_operation = TransformOperation::None;

				if using_pen_tool {
					self.last_point = DVec2::ZERO;
					self.grs_pen_handle = false;
					selected.responses.add(PenToolMessage::Confirm);
					selected.pen_handle = None;
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
			}
			TransformLayerMessage::BeginGrab => {
				if (!using_path_tool && !using_select_tool & !using_pen_tool)
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
			}
			TransformLayerMessage::CancelTransformOperation => {
				if using_pen_tool {
					self.last_point = DVec2::ZERO;
					self.transform_operation = TransformOperation::None;
					self.typing.clear();
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
					.constrain_axis(Axis::X, &mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport)
			}
			TransformLayerMessage::ConstrainY => {
				self.local = self
					.transform_operation
					.constrain_axis(Axis::Y, &mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport)
			}
			TransformLayerMessage::Overlays(mut overlay_context) => {
				for layer in document.metadata().all_layers() {
					if !document.network_interface.is_artboard(&layer.to_node(), &[]) {
						continue;
					};

					let viewport_box = input.viewport_bounds.size();
					let transform = DAffine2::from_translation(DVec2::new(0., viewport_box.y)) * DAffine2::from_scale(DVec2::splat(1.2));

					let axis_constraint = match self.transform_operation {
						TransformOperation::Grabbing(grabbing) => grabbing.constraint,
						TransformOperation::Scaling(scaling) => scaling.constraint,
						_ => Axis::Both,
					};

					let format_rounded = |value: f64, precision: usize| format!("{:.*}", precision, value).trim_end_matches('0').trim_end_matches('.').to_string();

					let axis_text = |vector: DVec2, separate: bool| match (axis_constraint, separate) {
						(Axis::Both, false) => format!("by {}", format_rounded(vector.x, 3)),
						(Axis::Both, true) => format!("by ({}, {})", format_rounded(vector.x, 3), format_rounded(vector.y, 3)),
						(Axis::X, _) => format!("X by {}", format_rounded(vector.x, 3)),
						(Axis::Y, _) => format!("Y by {}", format_rounded(vector.y, 3)),
					};

					let grs_value_text = match self.transform_operation {
						TransformOperation::None => String::new(),
						TransformOperation::Grabbing(translation) => format!(
							"Translating {}",
							axis_text(document_to_viewport.inverse().transform_vector2(translation.to_dvec(document_to_viewport)), true)
						),
						TransformOperation::Rotating(rotation) => format!("Rotating by {}°", format_rounded(rotation.to_f64(self.snap).to_degrees(), 3)),
						TransformOperation::Scaling(scale) => format!("Scaling {}", axis_text(scale.to_dvec(self.snap), false)),
					};

					match self.transform_operation {
						TransformOperation::None => (),
						TransformOperation::Grabbing(translation) => {
							let translation = document_to_viewport.inverse().transform_vector2(translation.to_dvec(document_to_viewport));
							let vec_to_end = self.mouse_position - self.start_mouse;
							let quad = Quad::from_box([self.grab_target, self.grab_target + vec_to_end]).0;
							let e1 = (self.fixed_bbox.0[1] - self.fixed_bbox.0[0]).normalize();

							if matches!(axis_constraint, Axis::Both | Axis::X) {
								let end = if self.local {
									(quad[1] - quad[0]).length() * e1 * e1.dot(quad[1] - quad[0]).signum() + quad[0]
								} else {
									quad[1]
								};
								overlay_context.line(quad[0], end, None);

								let x_transform = DAffine2::from_translation((quad[0] + end) / 2.);
								overlay_context.text(&format_rounded(translation.x, 3), COLOR_OVERLAY_BLUE, None, x_transform, 4., [Pivot::Middle, Pivot::End]);
							}

							if matches!(axis_constraint, Axis::Both | Axis::Y) {
								let end = if self.local {
									(quad[3] - quad[0]).length() * e1.perp() * e1.perp().dot(quad[3] - quad[0]).signum() + quad[0]
								} else {
									quad[3]
								};
								overlay_context.line(quad[0], end, None);
								let x_parameter = vec_to_end.x.clamp(-1., 1.);
								let y_transform = DAffine2::from_translation((quad[0] + end) / 2. + x_parameter * DVec2::X * 0.);
								let pivot_selection = if x_parameter > 0. {
									Pivot::Start
								} else if x_parameter == 0. {
									Pivot::Middle
								} else {
									Pivot::End
								};
								overlay_context.text(&format_rounded(translation.y, 2), COLOR_OVERLAY_BLUE, None, y_transform, 3., [pivot_selection, Pivot::Middle]);
							}
							if matches!(axis_constraint, Axis::Both) {
								overlay_context.dashed_line(quad[1], quad[2], None, Some(2.), Some(2.), Some(0.5));
								overlay_context.dashed_line(quad[3], quad[2], None, Some(2.), Some(2.), Some(0.5));
							}
						}
						TransformOperation::Scaling(scale) => {
							let scale = scale.to_f64(self.snap);
							let text = format!("{}x", format_rounded(scale, 3));
							let extension_vector = self.mouse_position - self.start_mouse;
							let local_edge = self.start_mouse - self.pivot;
							let quad = self.fixed_bbox.0;
							let local_edge = match axis_constraint {
								Axis::X => {
									if self.local {
										local_edge.project_onto(quad[1] - quad[0])
									} else {
										local_edge.with_y(0.)
									}
								}
								Axis::Y => {
									if self.local {
										local_edge.project_onto(quad[3] - quad[0])
									} else {
										local_edge.with_x(0.)
									}
								}
								_ => local_edge,
							};
							let boundary_point = local_edge + self.pivot;
							let projected_pointer = extension_vector.project_onto(local_edge);
							let dashed_till = if extension_vector.dot(local_edge) < 0. { local_edge + projected_pointer } else { local_edge };
							let lined_till = projected_pointer + boundary_point;
							if dashed_till.dot(local_edge) > 0. {
								overlay_context.dashed_line(self.pivot, self.pivot + dashed_till, None, Some(4.), Some(4.), Some(0.5));
							}
							overlay_context.line(boundary_point, lined_till, None);

							let transform = DAffine2::from_translation(boundary_point.midpoint(self.pivot) + local_edge.perp().normalize() * local_edge.element_product().signum() * 24.);
							overlay_context.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
						}
						TransformOperation::Rotating(rotation) => {
							let angle = rotation.to_f64(self.snap);
							let quad = self.fixed_bbox.0;
							let offset_angle = if self.grs_pen_handle {
								(self.handle - self.last_point).to_angle()
							} else {
								(quad[1] - quad[0]).to_angle()
							};
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

					overlay_context.text(&grs_value_text, COLOR_OVERLAY_WHITE, Some(COLOR_OVERLAY_SNAP_BACKGROUND), transform, 4., [Pivot::Start, Pivot::End]);
				}
			}
			TransformLayerMessage::PointerMove { slow_key, snap_key } => {
				self.slow = input.keyboard.get(slow_key as usize);

				let new_snap = input.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					self.transform_operation
						.apply_transform_operation(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
				}

				if self.typing.digits.is_empty() {
					let delta_pos = input.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
						}
						TransformOperation::Rotating(rotation) => {
							let angle;
							let start_offset = *selected.pivot - self.mouse_position;
							let end_offset = *selected.pivot - input.mouse.position;
							angle = start_offset.angle_to(end_offset);

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };

							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
						}
						TransformOperation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - *selected.pivot).length();
								let current_frame_dist = (input.mouse.position - *selected.pivot).length();
								let start_transform_dist = (self.start_mouse - *selected.pivot).length();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};
							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							self.transform_operation = TransformOperation::Scaling(scale.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
						}
					};
				}

				self.mouse_position = input.mouse.position;
			}
			TransformLayerMessage::SelectionChanged => {
				let target_layers = document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);
			}
			TransformLayerMessage::TypeBackspace => self
				.transform_operation
				.grs_typed(self.typing.type_backspace(), &mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport),
			TransformLayerMessage::TypeDecimalPoint => {
				self.transform_operation
					.grs_typed(self.typing.type_decimal_point(), &mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport)
			}
			TransformLayerMessage::TypeDigit { digit } => {
				self.transform_operation
					.grs_typed(self.typing.type_number(digit), &mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport)
			}
			TransformLayerMessage::TypeNegate => {
				if self.typing.digits.is_empty() {
					self.transform_operation.negate(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
				}
				self.transform_operation
					.grs_typed(self.typing.type_negate(), &mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport)
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
