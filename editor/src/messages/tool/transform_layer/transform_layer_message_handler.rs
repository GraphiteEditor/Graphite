use crate::consts::{ANGLE_MEASURE_RADIUS_FACTOR, ARC_MEASURE_RADIUS_FACTOR_RANGE, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_GRAY, SLOWING_DIVISOR};
use crate::messages::input_mapper::utility_types::input_mouse::{DocumentPosition, ViewportPosition};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, TransformType, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::pivot::{PivotGizmo, PivotGizmoType};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::{ToolData, ToolType};
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::misc::ManipulatorPointId;
use graphene_std::vector::{Vector, VectorModificationType};
use std::f64::consts::{PI, TAU};

const TRANSFORM_GRS_OVERLAY_PROVIDER: OverlayProvider = |context| TransformLayerMessage::Overlays(context).into();

// TODO: Get these from the input mapper
const SLOW_KEY: Key = Key::Shift;
const INCREMENTS_KEY: Key = Key::Control;

#[derive(ExtractField)]
pub struct TransformLayerMessageContext<'a> {
	pub document: &'a DocumentMessageHandler,
	pub input: &'a InputPreprocessorMessageHandler,
	pub tool_data: &'a ToolData,
	pub shape_editor: &'a mut ShapeState,
}

#[derive(Debug, Clone, Default, ExtractField)]
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
	pivot_gizmo: PivotGizmo,
	pivot: ViewportPosition,

	path_bounds: Option<[DVec2; 2]>,

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

	// Ghost outlines for Path Tool
	ghost_outline: Vec<(Vec<ClickTargetType>, DAffine2)>,
}

impl MessageHandler<TransformLayerMessage, TransformLayerMessageContext<'_>> for TransformLayerMessageHandler {
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, context: TransformLayerMessageContext) {
		let TransformLayerMessageContext {
			document,
			input,
			tool_data,
			shape_editor,
		} = context;

		let using_path_tool = tool_data.active_tool_type == ToolType::Path;
		let using_select_tool = tool_data.active_tool_type == ToolType::Select;
		let using_pen_tool = tool_data.active_tool_type == ToolType::Pen;
		let using_shape_tool = tool_data.active_tool_type == ToolType::Shape;

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
				self.pivot_gizmo.recalculate_transform(document);
				*selected.pivot = self.pivot_gizmo.position(document);
				self.local_pivot = document.metadata().document_to_viewport.inverse().transform_point2(*selected.pivot);
				self.grab_target = self.local_pivot;
			}
			// Here vector data from all layers is not considered which can be a problem in pivot calculation
			else if let Some(vector) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
				*selected.original_transforms = OriginalTransforms::default();

				let viewspace = document.metadata().transform_to_viewport(selected_layers[0]);
				let selected_segments = shape_editor.selected_segments().collect::<HashSet<_>>();
				let mut affected_points = shape_editor.selected_points().copied().collect::<Vec<_>>();

				for (segment_id, _, start, end) in vector.segment_bezier_iter() {
					if selected_segments.contains(&segment_id) {
						affected_points.push(ManipulatorPointId::Anchor(start));
						affected_points.push(ManipulatorPointId::Anchor(end));
					}
				}

				let affected_point_refs = affected_points.iter().collect();

				let get_location = |point: &&ManipulatorPointId| point.get_position(&vector).map(|position| viewspace.transform_point2(position));
				if let (Some((new_pivot, grab_target)), bounds) = calculate_pivot(
					document,
					&affected_point_refs,
					&vector,
					viewspace,
					|point: &ManipulatorPointId| get_location(&point),
					&mut self.pivot_gizmo,
				) {
					*selected.pivot = new_pivot;
					self.path_bounds = bounds;

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
			// Overlays
			TransformLayerMessage::Overlays(mut overlay_context) => {
				if !overlay_context.visibility_settings.transform_measurement() {
					return;
				}

				if using_path_tool {
					for (outline, transform) in &self.ghost_outline {
						overlay_context.outline(outline.iter(), *transform, Some(COLOR_OVERLAY_GRAY));
					}
				}

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
						let quad = Quad::from_box([pivot, pivot + viewport_translate]);

						responses.add(SelectToolMessage::PivotShift {
							offset: Some(viewport_translate),
							flush: false,
						});

						let typed_string = (!self.typing.digits.is_empty() && self.transform_operation.can_begin_typing()).then(|| self.typing.string.clone());
						overlay_context.translation_box(translation, quad, typed_string);
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
							overlay_context.dashed_line(pivot, boundary_point, None, None, Some(2.), Some(2.), Some(0.5));
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

				if using_path_tool {
					self.ghost_outline.clear();
				}

				responses.add(SelectToolMessage::PivotShift { offset: None, flush: true });

				if final_transform {
					responses.add(OverlaysMessage::RemoveProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				}
			}
			TransformLayerMessage::BeginTransformOperation { operation } => {
				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse, &mut self.initial_transform);
				self.transform_operation = match operation {
					TransformType::Grab => TransformOperation::Grabbing(Default::default()),
					TransformType::Rotate => TransformOperation::Rotating(Default::default()),
					TransformType::Scale => TransformOperation::Scaling(Default::default()),
				};
				self.layer_bounding_box = selected.bounding_box();
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
			TransformLayerMessage::BeginGRS { operation: transform_type } => {
				let selected_points: Vec<&ManipulatorPointId> = shape_editor.selected_points().collect();
				let selected_segments = shape_editor.selected_segments().collect::<Vec<_>>();

				if using_path_tool {
					Self::set_ghost_outline(&mut self.ghost_outline, shape_editor, document);
					if (selected_points.is_empty() && selected_segments.is_empty())
						|| (!using_path_tool && !using_select_tool && !using_pen_tool && !using_shape_tool)
						|| selected_layers.is_empty()
						|| transform_type.equivalent_to(self.transform_operation)
					{
						return;
					}
				}

				if let Some(vector) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
					if let [point] = selected_points.as_slice() {
						if matches!(point, ManipulatorPointId::Anchor(_)) {
							if let Some([handle1, handle2]) = point.get_handle_pair(&vector) {
								let handle1_length = handle1.length(&vector);
								let handle2_length = handle2.length(&vector);

								if (handle1_length == 0. && handle2_length == 0. && !using_select_tool) || (handle1_length == f64::MAX && handle2_length == f64::MAX && !using_select_tool) {
									// G should work for this point but not R and S
									if matches!(transform_type, TransformType::Rotate | TransformType::Scale) {
										selected.original_transforms.clear();
										return;
									}
								}
							}
						} else {
							let handle_length = point.as_handle().map(|handle| handle.length(&vector));

							if handle_length == Some(0.) {
								selected.original_transforms.clear();
								return;
							}
						}
					}
				}

				self.local = false;
				self.operation_count += 1;

				let chain_operation = self.transform_operation != TransformOperation::None;
				if chain_operation {
					responses.add(TransformLayerMessage::ApplyTransformOperation { final_transform: false });
				} else {
					responses.add(OverlaysMessage::AddProvider(TRANSFORM_GRS_OVERLAY_PROVIDER));
				}
				responses.add(TransformLayerMessage::BeginTransformOperation { operation: transform_type });
				responses.add(TransformLayerMessage::PointerMove {
					slow_key: SLOW_KEY,
					increments_key: INCREMENTS_KEY,
				});
			}
			TransformLayerMessage::BeginGrab => responses.add_front(TransformLayerMessage::BeginGRS { operation: TransformType::Grab }),
			TransformLayerMessage::BeginRotate => responses.add_front(TransformLayerMessage::BeginGRS { operation: TransformType::Rotate }),
			TransformLayerMessage::BeginScale => responses.add_front(TransformLayerMessage::BeginGRS { operation: TransformType::Scale }),
			TransformLayerMessage::CancelTransformOperation => {
				if using_path_tool {
					self.ghost_outline.clear();
				}

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

				responses.add(SelectToolMessage::PivotShift { offset: None, flush: false });
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

				self.mouse_position = input.mouse.position;
			}
			TransformLayerMessage::SelectionChanged => {
				let target_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);
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
					)
				}
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
					)
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
				)
			}
			TransformLayerMessage::SetPivotGizmo { pivot_gizmo } => {
				self.pivot_gizmo = pivot_gizmo;
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(TransformLayerMessageDiscriminant;
			BeginGrab,
			BeginRotate,
			BeginScale,
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

impl TransformLayerMessageHandler {
	pub fn is_transforming(&self) -> bool {
		self.transform_operation != TransformOperation::None
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>) {
		self.transform_operation.hints(responses, self.local);
	}

	fn set_ghost_outline(ghost_outline: &mut Vec<(Vec<ClickTargetType>, DAffine2)>, shape_editor: &ShapeState, document: &DocumentMessageHandler) {
		ghost_outline.clear();
		for &layer in shape_editor.selected_shape_state.keys() {
			// We probably need to collect here
			let outline = document.metadata().layer_with_free_points_outline(layer).cloned().collect();
			let transform = document.metadata().transform_to_viewport(layer);
			ghost_outline.push((outline, transform));
		}
	}
}

fn calculate_pivot(
	document: &DocumentMessageHandler,
	selected_points: &Vec<&ManipulatorPointId>,
	vector: &Vector,
	viewspace: DAffine2,
	get_location: impl Fn(&ManipulatorPointId) -> Option<DVec2>,
	gizmo: &mut PivotGizmo,
) -> (Option<(DVec2, DVec2)>, Option<[DVec2; 2]>) {
	let average_position = || {
		let mut point_count = 0_usize;
		selected_points.iter().filter_map(|p| get_location(p)).inspect(|_| point_count += 1).sum::<DVec2>() / point_count as f64
	};
	let bounds = selected_points.iter().filter_map(|p| get_location(p)).fold(None, |acc: Option<[DVec2; 2]>, point| {
		if let Some([mut min, mut max]) = acc {
			min.x = min.x.min(point.x);
			min.y = min.y.min(point.y);
			max.x = max.x.max(point.x);
			max.y = max.y.max(point.y);
			Some([min, max])
		} else {
			Some([point, point])
		}
	});
	gizmo.pivot.recalculate_pivot_for_layer(document, bounds);
	let position = || {
		(if !gizmo.state.disabled {
			match gizmo.state.gizmo_type {
				PivotGizmoType::Average => None,
				PivotGizmoType::Active => gizmo.point.and_then(|p| get_location(&p)),
				PivotGizmoType::Pivot => gizmo.pivot.pivot,
			}
		} else {
			None
		})
		.unwrap_or_else(average_position)
	};
	let [point] = selected_points.as_slice() else {
		// Handle the case where there are multiple points
		let position = position();
		return (Some((position, position)), bounds);
	};

	match point {
		ManipulatorPointId::PrimaryHandle(_) | ManipulatorPointId::EndHandle(_) => {
			// Get the anchor position and transform it to the pivot
			let (Some(pivot_position), Some(position)) = (
				point.get_anchor_position(vector).map(|anchor_position| viewspace.transform_point2(anchor_position)),
				point.get_position(vector),
			) else {
				return (None, None);
			};
			let target = viewspace.transform_point2(position);
			(Some((pivot_position, target)), None)
		}
		_ => {
			// Calculate the average position of all selected points
			let position = position();
			(Some((position, position)), bounds)
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

fn update_colinear_handles(selected_layers: &[LayerNodeIdentifier], document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	for &layer in selected_layers {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

		for [handle1, handle2] in &vector.colinear_manipulators {
			let manipulator1 = handle1.to_manipulator_point();
			let manipulator2 = handle2.to_manipulator_point();

			let Some(anchor) = manipulator1.get_anchor_position(&vector) else { continue };
			let Some(pos1) = manipulator1.get_position(&vector).map(|pos| pos - anchor) else { continue };
			let Some(pos2) = manipulator2.get_position(&vector).map(|pos| pos - anchor) else { continue };

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

#[cfg(test)]
mod test_transform_layer {
	use crate::messages::portfolio::document::graph_operation::transform_utils;
	use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
	use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
	use crate::messages::prelude::Message;
	use crate::messages::tool::transform_layer::transform_layer_message_handler::VectorModificationType;
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::PointId;
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

		let translation = DVec2::new(50., 50.);
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
		assert!(translation_diff > 10., "Transform should have changed after applying transformation. Diff: {}", translation_diff);
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
		editor.move_mouse(50., 50., ModifierKeys::empty(), MouseKeys::NONE).await;
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
			(final_translation - original_translation).length() < 5. || final_translation.length() < 0.001,
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

		editor.move_mouse(150., 50., ModifierKeys::empty(), MouseKeys::NONE).await;

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
		assert!(translation_diff < 1., "Translation component changed too much: {}", translation_diff);
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

		editor.move_mouse(150., 150., ModifierKeys::empty(), MouseKeys::NONE).await;

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
		assert!(translation_diff < 1., "Translation component changed too much: {}", translation_diff);
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
		editor.move_mouse(150., 130., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;

		let after_grab_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		let expected_translation = DVec2::new(50., 30.);
		let actual_translation = after_grab_transform.translation - original_transform.translation;
		assert!(
			(actual_translation - expected_translation).length() < 1e-5,
			"Expected translation of {:?}, got {:?}",
			expected_translation,
			actual_translation
		);

		// 2. Chain to rotation - from current position to create ~45 degree rotation
		editor.handle_message(TransformLayerMessage::BeginRotate).await;
		editor.move_mouse(190., 90., ModifierKeys::empty(), MouseKeys::NONE).await;
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
		editor.move_mouse(250., 200., ModifierKeys::empty(), MouseKeys::NONE).await;
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
			after_scale_det >= 2. * before_scale_det,
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

		let pan_amount = DVec2::new(200., 150.);
		editor.handle_message(NavigationMessage::CanvasPan { delta: pan_amount }).await;

		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let scale_x = final_transform.matrix2.x_axis.length() / original_transform.matrix2.x_axis.length();
		let scale_y = final_transform.matrix2.y_axis.length() / original_transform.matrix2.y_axis.length();

		assert!((scale_x - 2.).abs() < 0.1, "Expected scale factor X of 2, got: {}", scale_x);
		assert!((scale_y - 2.).abs() < 0.1, "Expected scale factor Y of 2, got: {}", scale_y);
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

		assert!((scale_x - 2.).abs() < 0.1, "Expected scale factor X of 2, got: {}", scale_x);
		assert!((scale_y - 2.).abs() < 0.1, "Expected scale factor Y of 2, got: {}", scale_y);
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
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				angle_radians: (45. as f64).to_radians(),
			})
			.await;
		editor.handle_message(TransformLayerMessage::BeginRotate).await;

		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 9 }).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 0 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();

		let original_angle = original_transform.to_scale_angle_translation().1;
		let final_angle = final_transform.to_scale_angle_translation().1;
		let angle_change = (final_angle - original_angle).to_degrees();

		// Normalize angle between 0 and 360
		let angle_change = ((angle_change % 360.) + 360.) % 360.;
		assert!((angle_change - 90.).abs() < 0.1, "Expected rotation of 90 degrees, got: {}", angle_change);
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
			position: DVec2::new(100., 100.),
		};
		editor.handle_message(GraphOperationMessage::Vector { layer, modification_type }).await;
		editor.handle_message(ToolMessage::ActivateTool { tool_type: ToolType::Select }).await;

		// Testing grab operation - just checking that it doesn't crash.
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(150., 150., ModifierKeys::empty(), MouseKeys::NONE).await;
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
		assert!(scale_x > 0., "Scale factor X should not be exactly zero");
		assert!(scale_y > 0., "Scale factor Y should not be exactly zero");

		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		assert!(final_transform.is_finite(), "Transform should be finite after rescaling");

		let new_scale_x = final_transform.matrix2.x_axis.length();
		let new_scale_y = final_transform.matrix2.y_axis.length();
		assert!(new_scale_x > 0., "After rescaling, scale factor X should be non-zero");
		assert!(new_scale_y > 0., "After rescaling, scale factor Y should be non-zero");
	}

	#[tokio::test]
	async fn test_transform_with_different_selections() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;
		editor.draw_rect(150., 0., 250., 100.).await;
		editor.draw_rect(0., 150., 100., 250.).await;
		editor.draw_rect(150., 150., 250., 250.).await;
		let document = editor.active_document();
		let layers: Vec<LayerNodeIdentifier> = document.metadata().all_layers().collect();

		assert!(layers.len() == 4);

		// Creating a group with two rectangles
		editor
			.handle_message(NodeGraphMessage::SelectedNodesSet {
				nodes: vec![layers[2].to_node(), layers[3].to_node()],
			})
			.await;
		editor
			.handle_message(DocumentMessage::GroupSelectedLayers {
				group_folder_type: GroupFolderType::Layer,
			})
			.await;

		// Get the group layer (should be the newest layer)
		let document = editor.active_document();
		let group_layer = document.metadata().all_layers().next().unwrap();

		// Test 1: Transform single layer
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layers[0].to_node()] }).await;
		let original_transform = get_layer_transform(&mut editor, layers[0]).await.unwrap();
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(50., 50., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		let final_transform = get_layer_transform(&mut editor, layers[0]).await.unwrap();
		assert!(!final_transform.abs_diff_eq(original_transform, 1e-5), "Transform should change for single layer");

		// Test 2: Transform multiple layers
		editor
			.handle_message(NodeGraphMessage::SelectedNodesSet {
				nodes: vec![layers[0].to_node(), layers[1].to_node()],
			})
			.await;
		let original_transform_1 = get_layer_transform(&mut editor, layers[0]).await.unwrap();
		let original_transform_2 = get_layer_transform(&mut editor, layers[1]).await.unwrap();
		editor.handle_message(TransformLayerMessage::BeginRotate).await;
		editor.move_mouse(200., 50., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		let final_transform_1 = get_layer_transform(&mut editor, layers[0]).await.unwrap();
		let final_transform_2 = get_layer_transform(&mut editor, layers[1]).await.unwrap();
		assert!(!final_transform_1.abs_diff_eq(original_transform_1, 1e-5), "Transform should change for first layer in multi-selection");
		assert!(
			!final_transform_2.abs_diff_eq(original_transform_2, 1e-5),
			"Transform should change for second layer in multi-selection"
		);

		// Test 3: Transform group
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![group_layer.to_node()] }).await;
		let original_group_transform = get_layer_transform(&mut editor, group_layer).await.unwrap();
		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		let final_group_transform = get_layer_transform(&mut editor, group_layer).await.unwrap();
		assert!(!final_group_transform.abs_diff_eq(original_group_transform, 1e-5), "Transform should change for group");

		// Test 4: Transform layers inside transformed group
		let child_layer_id = {
			let document = editor.active_document_mut();
			let group_children = document.network_interface.downstream_layers(&group_layer.to_node(), &[]);
			if !group_children.is_empty() {
				Some(LayerNodeIdentifier::new(group_children[0], &document.network_interface))
			} else {
				None
			}
		};
		assert!(child_layer_id.is_some(), "Group should have child layers");
		let child_layer_id = child_layer_id.unwrap();
		editor
			.handle_message(NodeGraphMessage::SelectedNodesSet {
				nodes: vec![child_layer_id.to_node()],
			})
			.await;
		let original_child_transform = get_layer_transform(&mut editor, child_layer_id).await.unwrap();
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(30., 30., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		let final_child_transform = get_layer_transform(&mut editor, child_layer_id).await.unwrap();
		assert!(!final_child_transform.abs_diff_eq(original_child_transform, 1e-5), "Child layer inside transformed group should change");
	}
}
