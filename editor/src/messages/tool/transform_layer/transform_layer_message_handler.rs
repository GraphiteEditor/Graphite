use crate::consts::{ANGLE_MEASURE_RADIUS_FACTOR, ARC_MEASURE_RADIUS_FACTOR_RANGE, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_SNAP_BACKGROUND, COLOR_OVERLAY_WHITE, SLOWING_DIVISOR};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::{ToolData, ToolType};

use graphene_core::renderer::Quad;
use graphene_core::vector::ManipulatorPointId;

use glam::{DAffine2, DVec2};
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
}

impl TransformLayerMessageHandler {
	pub fn is_transforming(&self) -> bool {
		self.transform_operation != TransformOperation::None
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>) {
		self.transform_operation.hints(responses);
	}
}

type TransformData<'a> = (&'a DocumentMessageHandler, &'a InputPreprocessorMessageHandler, &'a ToolData, &'a mut ShapeState);
impl MessageHandler<TransformLayerMessage, TransformData<'_>> for TransformLayerMessageHandler {
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, (document, input, tool_data, shape_editor): TransformData) {
		let using_path_tool = tool_data.active_tool_type == ToolType::Path;
		let using_select_tool = tool_data.active_tool_type == ToolType::Select;

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
		);

		let mut begin_operation = |operation: TransformOperation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2| {
			if operation != TransformOperation::None {
				selected.revert_operation();
				typing.clear();
			}

			if using_path_tool {
				if let Some(vector_data) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
					*selected.original_transforms = OriginalTransforms::default();
					let viewspace = document.metadata().transform_to_viewport(selected_layers[0]);

					let mut point_count: usize = 0;
					let get_location = |point: &&ManipulatorPointId| point.get_position(&vector_data).map(|position| viewspace.transform_point2(position));
					let points = shape_editor.selected_points();
					let selected_points: Vec<&ManipulatorPointId> = points.collect();

					if let [point] = selected_points.as_slice() {
						if let ManipulatorPointId::PrimaryHandle(_) | ManipulatorPointId::EndHandle(_) = point {
							let anchor_position = point.get_anchor_position(&vector_data).unwrap();
							*selected.pivot = viewspace.transform_point2(anchor_position);
						} else {
							*selected.pivot = selected_points.iter().filter_map(get_location).inspect(|_| point_count += 1).sum::<DVec2>() / point_count as f64;
						}
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

				responses.add(DocumentMessage::EndTransaction);
				responses.add(ToolMessage::UpdateHints);
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(OverlaysMessage::RemoveProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
			}
			TransformLayerMessage::BeginGrab => {
				if (!using_path_tool && !using_select_tool)
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

				if (!using_path_tool && !using_select_tool)
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
					|| (!using_path_tool && !using_select_tool)
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
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(DocumentMessage::AbortTransaction);
				responses.add(ToolMessage::UpdateHints);

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
							let quad = Quad::from_box([self.pivot, self.pivot + vec_to_end]).0;
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
							let offset_angle = (quad[1] - quad[0]).to_angle();
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
							let start_offset = *selected.pivot - self.mouse_position;
							let end_offset = *selected.pivot - input.mouse.position;
							let angle = start_offset.angle_to(end_offset);

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
							let region_negate = (self.start_mouse - *selected.pivot).dot(self.mouse_position - *selected.pivot) < 0.;
							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							let change = change * scale.dragged_factor.signum();
							self.transform_operation = TransformOperation::Scaling(scale.increment_amount(change));
							if region_negate {
								let tmp_operation = TransformOperation::Scaling(scale.negate());
								tmp_operation.apply_transform_operation(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
							} else {
								self.transform_operation
									.apply_transform_operation(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport);
							}
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
					self.transform_operation.negate(&mut selected, self.snap, self.local, self.fixed_bbox, document_to_viewport)
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
