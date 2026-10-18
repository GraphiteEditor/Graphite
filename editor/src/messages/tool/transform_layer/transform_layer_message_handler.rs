use crate::consts::{ANGLE_MEASURE_RADIUS_FACTOR, ARC_MEASURE_RADIUS_FACTOR_RANGE, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_GRAY, SLOWING_DIVISOR};
use crate::messages::input_mapper::utility_types::pointer::{DocumentPosition, ViewportPosition};
use crate::messages::portfolio::document::overlays::utility_functions::text_width;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, TransformType, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::pivot::{PivotGizmo, PivotGizmoType};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::format_rounded;
use crate::messages::tool::common_functionality::snap_session::{BoxFeatures, FeatureMap, Locus, Moving, PointOptions, SnapSession, SnapView};
use crate::messages::tool::common_functionality::transformation_cage::MIN_FEATURE_TRAVEL;
use crate::messages::tool::common_functionality::utility_functions::{is_almost_colinear, push_nearest_anchors};
use crate::messages::tool::tool_messages::path_tool::MAX_SNAPPING_POINTS;
use crate::messages::tool::tool_messages::select_tool;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::{ToolData, ToolType};
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::misc::ManipulatorPointId;
use graphene_std::vector::{VectorGeometry, VectorModificationType};
use std::f64::consts::{PI, TAU};

const TRANSFORM_GRS_OVERLAY_PROVIDER: OverlayProvider = |context| TransformLayerMessage::Overlays { context }.into();

// TODO: Get these from the input mapper
const SLOW_KEY: Key = Key::Shift;
const INCREMENTS_KEY: Key = Key::Control;

#[derive(ExtractField)]
pub struct TransformLayerMessageContext<'a> {
	pub document: &'a DocumentMessageHandler,
	pub document_id: DocumentId,
	pub input: &'a InputPreprocessorMessageHandler,
	pub tool_data: &'a ToolData,
	pub shape_editor: &'a mut ShapeState,
	pub snap_session: &'a mut SnapSession,
	pub viewport: &'a ViewportMessageHandler,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct TransformationState {
	pub is_rounded_to_intervals: bool,
	pub is_transforming_in_local_space: bool,
	/// Whether the transform snaps, which the Pen's handle transforms don't.
	pub is_snapping: bool,
	pub local_transform_axes: [DVec2; 2],
	pub document_space_pivot: DocumentPosition,
}

impl TransformationState {
	pub fn pivot_viewport(&self, document: &DocumentMessageHandler) -> DVec2 {
		document.metadata().document_to_viewport.transform_point2(self.document_space_pivot)
	}

	pub fn constraint_axis(&self, axis_constraint: Axis) -> Option<DVec2> {
		match axis_constraint {
			Axis::X => Some(if self.is_transforming_in_local_space { self.local_transform_axes[0] } else { DVec2::X }),
			Axis::Y => Some(if self.is_transforming_in_local_space { self.local_transform_axes[1] } else { DVec2::Y }),
			_ => None,
		}
	}

	pub fn project_onto_constrained(&self, vector: DVec2, axis_constraint: Axis) -> DVec2 {
		self.constraint_axis(axis_constraint).map_or(vector, |direction| vector.project_onto_normalized(direction))
	}

	pub fn local_to_viewport_transform(&self) -> DAffine2 {
		if self.is_transforming_in_local_space {
			DAffine2::from_cols(self.local_transform_axes[0], self.local_transform_axes[1], DVec2::ZERO)
		} else {
			DAffine2::IDENTITY
		}
	}
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct TransformLayerMessageHandler {
	pub transform_operation: TransformOperation,
	state: TransformationState,
	slow: bool,
	layer_bounding_box: Quad,
	typing: Typing,
	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,
	original_transforms: OriginalTransforms,
	pivot_gizmo: PivotGizmo,
	pivot: ViewportPosition,
	path_bounds: Option<[DVec2; 2]>,
	local_mouse_start: DocumentPosition,
	grab_target: DocumentPosition,
	ptz: PTZ,
	initial_transform: DAffine2,
	operation_count: usize,
	was_grabbing: bool,
	/// What the transform under way snaps by, if it began a snapping gesture.
	snapping: Option<Snapping>,

	// Pen tool (outgoing handle GRS manipulation)
	handle: DVec2,
	last_point: DVec2,
	grs_pen_handle: bool,

	// Path tool (ghost outlines showing pre-transform geometry)
	ghost_outline: Vec<(Vec<ClickTargetType>, DAffine2)>,
}

#[message_handler_data]
impl MessageHandler<TransformLayerMessage, TransformLayerMessageContext<'_>> for TransformLayerMessageHandler {
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, context: TransformLayerMessageContext) {
		let TransformLayerMessageContext {
			document,
			document_id,
			input,
			tool_data,
			shape_editor,
			snap_session,
			viewport,
		} = context;
		let view = SnapView::new(document, document_id, input);

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
				self.state.document_space_pivot = document.metadata().document_to_viewport.inverse().transform_point2(*selected.pivot);
				self.grab_target = self.state.document_space_pivot;
			}
			// TODO: Here vector data from all layers is not considered which can be a problem in pivot calculation
			else if let Some(vector) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer)) {
				*selected.original_transforms = OriginalTransforms::default();

				let viewspace = document.metadata().transform_to_viewport(selected_layers[0]);
				let selected_segments = shape_editor.selected_segments().collect::<HashSet<_>>();
				let mut affected_points = shape_editor.selected_points().copied().collect::<Vec<_>>();

				for (segment_id, _, start, end) in vector.segment_iter() {
					if selected_segments.contains(&segment_id) {
						affected_points.push(ManipulatorPointId::Anchor(start));
						affected_points.push(ManipulatorPointId::Anchor(end));
					}
				}

				let affected_point_refs = affected_points.iter().collect();

				let get_location = |point: &&ManipulatorPointId| vector.position_of(**point).map(|position| viewspace.transform_point2(position));
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

					self.state.document_space_pivot = document_to_viewport.inverse().transform_point2(*selected.pivot);
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
			TransformLayerMessage::Overlays { context: mut overlay_context } => {
				if !overlay_context.visibility_settings.transform_measurement() {
					return;
				}

				if using_path_tool {
					for (outline, transform) in &self.ghost_outline {
						overlay_context.outline(outline.iter(), *transform, Some(COLOR_OVERLAY_GRAY));
					}
				}

				let viewport_box = viewport.size().into_dvec2();
				let axis_constraint = self.transform_operation.axis_constraint();

				let format_rounded = |value: f64, precision: usize| {
					if self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
						format_rounded(value, precision)
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
						let translation_viewport = self.state.local_to_viewport_transform().matrix2 * translation.to_dvec(&self.state, document);
						let pivot = document_to_viewport.transform_point2(self.grab_target);
						let quad = Quad::from_box([pivot, pivot + translation_viewport]);

						responses.add(SelectToolMessage::PivotShift {
							offset: Some(translation_viewport),
							flush: false,
						});

						let typed_string = (!self.typing.digits.is_empty() && self.transform_operation.can_begin_typing()).then(|| self.typing.string.clone());
						overlay_context.translation_box(translation_viewport / document_to_viewport.matrix2.y_axis.length(), quad, typed_string);
					}
					TransformOperation::Scaling(scale) => {
						let scale = scale.to_f64(self.state.is_rounded_to_intervals);
						let text = format!("{}x", format_rounded(scale, 3));
						let start_mouse = document_to_viewport.transform_point2(self.local_mouse_start);
						let local_edge = start_mouse - self.state.pivot_viewport(document);
						let local_edge = self.state.project_onto_constrained(local_edge, axis_constraint);
						let boundary_point = self.state.pivot_viewport(document) + local_edge * scale.min(1.);
						let end_point = self.state.pivot_viewport(document) + local_edge * scale.max(1.);

						if scale > 0. {
							overlay_context.dashed_line(self.state.pivot_viewport(document), boundary_point, None, None, Some(2.), Some(2.), Some(0.5));
						}
						overlay_context.line(boundary_point, end_point, None, None);

						let transform = DAffine2::from_translation(
							boundary_point.midpoint(self.state.pivot_viewport(document)) + local_edge.perp().normalize_or(DVec2::X) * local_edge.element_product().signum() * 24.,
						);
						overlay_context.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
					}
					TransformOperation::Rotating(rotation) => {
						let angle = rotation.to_f64(self.state.is_rounded_to_intervals, document.snapping_state.angle_step_radians());
						let start_mouse = document_to_viewport.transform_point2(self.local_mouse_start);
						let offset_angle = if self.grs_pen_handle {
							self.handle - self.last_point
						} else if using_path_tool {
							start_mouse - self.state.pivot_viewport(document)
						} else {
							// TODO: This is always zero breaking the `.to_angle()` below?
							self.layer_bounding_box.top_right() - self.layer_bounding_box.top_right()
						};
						let tilt_offset = document.document_ptz.unmodified_tilt();
						let offset_angle = offset_angle.to_angle() + tilt_offset;
						let width = viewport_box.max_element();
						let radius = start_mouse.distance(self.state.pivot_viewport(document));
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
						const FONT_SIZE: f64 = 12.;

						let text_texture_width = text_width(&text, FONT_SIZE) / 2.;
						let text_texture_height = 12.;
						let text_angle_on_unit_circle = DVec2::from_angle((angle % TAU) / 2. + offset_angle);
						let text_texture_position = DVec2::new(
							(arc_radius + 4. + text_texture_width) * text_angle_on_unit_circle.x,
							(arc_radius + text_texture_height) * text_angle_on_unit_circle.y,
						);
						let transform = DAffine2::from_translation(text_texture_position + self.state.pivot_viewport(document));
						overlay_context.draw_angle(self.state.pivot_viewport(document), radius, arc_radius, offset_angle, angle);
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
					self.was_grabbing = false;
					responses.add(OverlaysMessage::RemoveProvider {
						provider: TRANSFORM_GRS_OVERLAY_PROVIDER,
					});
					self.end_snapping(snap_session, responses);
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
				let bounding_box = select_tool::create_bounding_box_transform(document);
				self.state.local_transform_axes = [bounding_box.x_axis, bounding_box.y_axis].map(|axis| axis.normalize_or_zero());

				// The Pen's handle is left unsnapped, since the Pen snaps the anchor it previews through the same session
				let grip = self.local_mouse_start;
				let snapping = if using_pen_tool {
					None
				} else if using_path_tool {
					Some(Snapping::of_points(document, &selected_layers, shape_editor, grip))
				} else if matches!(selected.original_transforms, OriginalTransforms::Layer(_)) {
					let moving = Moving {
						layers: selected_layers.clone(),
						..Default::default()
					};
					Some((Snapping::of_layers(document, &selected_layers, self.layer_bounding_box, grip), moving))
				} else {
					None
				};

				match snapping {
					Some((mut snapping, moving)) => {
						snapping.build(&self.transform_operation, &self.state, document);
						snap_session.begin_gesture(view, moving);
						self.snapping = Some(snapping);
						self.state.is_snapping = true;
					}
					None => self.end_snapping(snap_session, responses),
				}
			}
			TransformLayerMessage::BeginGrabPen { last_point, handle } | TransformLayerMessage::BeginRotatePen { last_point, handle } | TransformLayerMessage::BeginScalePen { last_point, handle } => {
				self.typing.clear();

				self.last_point = last_point;
				self.handle = handle;
				self.grs_pen_handle = true;
				self.mouse_position = input.mouse.position;
				self.start_mouse = input.mouse.position;
				self.initial_transform = document_to_viewport;

				let top_left = DVec2::new(last_point.x, handle.y);
				let bottom_right = DVec2::new(handle.x, last_point.y);
				self.state.is_transforming_in_local_space = false;
				self.layer_bounding_box = Quad::from_box([top_left, bottom_right]);
				let normalized_along = (handle - last_point).normalize_or_zero();
				self.state.local_transform_axes = [normalized_along, normalized_along.perp()];
				self.grab_target = document.metadata().document_to_viewport.inverse().transform_point2(handle);
				let pivot = last_point;
				self.state.document_space_pivot = document.metadata().document_to_viewport.inverse().transform_point2(pivot);
				self.local_mouse_start = document.metadata().document_to_viewport.inverse().transform_point2(self.start_mouse);
				self.handle = handle;

				// Operation-specific logic
				self.transform_operation = match message {
					TransformLayerMessage::BeginGrabPen { .. } => TransformOperation::Grabbing(Default::default()),
					TransformLayerMessage::BeginRotatePen { .. } => TransformOperation::Rotating(Default::default()),
					TransformLayerMessage::BeginScalePen { .. } => TransformOperation::Scaling(Default::default()),
					_ => unreachable!(), // Safe because the match arms are exhaustive
				};

				responses.add(OverlaysMessage::AddProvider {
					provider: TRANSFORM_GRS_OVERLAY_PROVIDER,
				});
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
						|| (transform_type.equivalent_to(self.transform_operation) && !self.was_grabbing)
					{
						return;
					}

					if using_path_tool && transform_type == TransformType::Grab {
						// Check if a single point is selected and it's a colinear point
						let single_anchor_selected = shape_editor.selected_points().count() == 1 && shape_editor.selected_points().any(|point| point.as_anchor().is_some());

						if single_anchor_selected && transform_type.equivalent_to(self.transform_operation) && self.was_grabbing {
							let selected_nodes = &document.network_interface.selected_nodes();

							let Some(layer) = selected_nodes.selected_layers(document.metadata()).next() else { return };
							let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else { return };
							let Some(anchor) = shape_editor.selected_points().next() else { return };

							if vector_data.colinear(*anchor) {
								responses.add(TransformLayerMessage::CancelTransformOperation);

								// Start sliding point
								responses.add(DeferMessage::AfterGraphRun {
									messages: vec![PathToolMessage::StartSlidingPoint.into()],
								});
							}

							return;
						}

						if transform_type.equivalent_to(self.transform_operation) {
							return;
						}

						self.was_grabbing = true;
					}
				}

				if let Some(vector) = selected_layers.first().and_then(|&layer| document.network_interface.compute_modified_vector(layer))
					&& let [point] = selected_points.as_slice()
				{
					if matches!(point, ManipulatorPointId::Anchor(_)) {
						if let Some([handle1, handle2]) = vector.handle_pair_of(**point) {
							let handle1_length = vector.handle_length(handle1);
							let handle2_length = vector.handle_length(handle2);

							if (handle1_length == 0. && handle2_length == 0. && !using_select_tool) || (handle1_length == f64::MAX && handle2_length == f64::MAX && !using_select_tool) {
								// G should work for this point but not R and S
								if matches!(transform_type, TransformType::Rotate | TransformType::Scale) {
									selected.original_transforms.clear();
									return;
								}
							}
						}
					} else {
						let handle_length = point.as_handle().map(|handle| vector.handle_length(handle));

						if handle_length == Some(0.) {
							selected.original_transforms.clear();
							return;
						}
					}
				}

				self.state.is_transforming_in_local_space = false;
				self.operation_count += 1;

				let chain_operation = self.transform_operation != TransformOperation::None;
				if chain_operation {
					responses.add(TransformLayerMessage::ApplyTransformOperation { final_transform: false });
				} else {
					responses.add(OverlaysMessage::AddProvider {
						provider: TRANSFORM_GRS_OVERLAY_PROVIDER,
					});
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
					self.was_grabbing = false;
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
				responses.add(OverlaysMessage::RemoveProvider {
					provider: TRANSFORM_GRS_OVERLAY_PROVIDER,
				});
				self.end_snapping(snap_session, responses);
			}
			TransformLayerMessage::ConstrainX | TransformLayerMessage::ConstrainY => {
				let axis = if matches!(message, TransformLayerMessage::ConstrainX) { Axis::X } else { Axis::Y };
				self.state.is_transforming_in_local_space = self.transform_operation.constrain_axis(axis, &mut selected, &self.state, document);

				// A scale snaps along different axes once constrained, so its features are built again
				if let Some(snapping) = &mut self.snapping {
					snapping.build(&self.transform_operation, &self.state, document);
					snapping.snap(&mut self.transform_operation, &self.state, view, snap_session);
				}

				self.transform_operation.grs_typed(self.typing.evaluate(), &mut selected, &self.state, document);
			}
			TransformLayerMessage::PointerMove { slow_key, increments_key } => {
				self.slow = input.keyboard.get(slow_key as usize);
				let old_ptz = self.ptz;
				self.ptz = document.document_ptz;
				if old_ptz != self.ptz {
					self.mouse_position = input.mouse.position;
					return;
				}

				let new_increments = input.keyboard.get(increments_key as usize);
				if new_increments != self.state.is_rounded_to_intervals {
					self.state.is_rounded_to_intervals = new_increments;
					snap_and_apply(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
				}

				if self.typing.digits.is_empty() || !self.transform_operation.can_begin_typing() {
					match self.transform_operation {
						TransformOperation::None => {}
						TransformOperation::Grabbing(translation) => {
							let delta_pos = input.mouse.position - self.mouse_position;
							let delta_pos = (self.initial_transform * document_to_viewport.inverse()).transform_vector2(delta_pos);
							let delta_viewport = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							let delta_scaled = delta_viewport / document_to_viewport.y_axis.length(); // Values are local to the viewport but scaled so values are relative to the current scale.
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(delta_scaled));
							snap_and_apply(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
						}
						TransformOperation::Rotating(rotation) => {
							let start_offset = self.state.pivot_viewport(document) - self.mouse_position;
							let end_offset = self.state.pivot_viewport(document) - input.mouse.position;
							if let Some(angle) = start_offset.try_angle_to(end_offset) {
								let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };

								self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
								snap_and_apply(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
							}
						}
						TransformOperation::Scaling(mut scale) => {
							let axis_constraint = scale.constraint;
							let to_mouse_final = self.mouse_position - self.state.pivot_viewport(document);
							let to_mouse_final_old = input.mouse.position - self.state.pivot_viewport(document);
							let to_mouse_start = self.start_mouse - self.state.pivot_viewport(document);

							let to_mouse_final = self.state.project_onto_constrained(to_mouse_final, axis_constraint);
							let to_mouse_final_old = self.state.project_onto_constrained(to_mouse_final_old, axis_constraint);
							let to_mouse_start = self.state.project_onto_constrained(to_mouse_start, axis_constraint);

							// Starting on the pivot leaves no distance to measure the scale against
							let start_transform_dist = to_mouse_start.length_squared();
							if start_transform_dist > f64::EPSILON {
								let previous_frame_dist = to_mouse_final.dot(to_mouse_start);
								let current_frame_dist = to_mouse_final_old.dot(to_mouse_start);
								let change = (current_frame_dist - previous_frame_dist) / start_transform_dist;
								let change = if self.slow { change / SLOWING_DIVISOR } else { change };

								scale = scale.increment_amount(change);
								self.transform_operation = TransformOperation::Scaling(scale);
								snap_and_apply(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
							}
						}
					};
				}

				self.mouse_position = input.mouse.position;
			}
			TransformLayerMessage::SelectionChanged => {
				let target_layers = document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface).collect();
				shape_editor.set_selected_layers(target_layers);
			}
			TransformLayerMessage::TypeBackspace => {
				if self.typing.digits.is_empty() && self.typing.negative {
					self.transform_operation.negate(&mut selected, &self.state, document);
					self.typing.type_negate();
				}
				self.transform_operation.grs_typed(self.typing.type_backspace(), &mut selected, &self.state, document);
				snap_again(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
			}
			TransformLayerMessage::TypeDecimalPoint => {
				if self.transform_operation.can_begin_typing() {
					self.transform_operation.grs_typed(self.typing.type_decimal_point(), &mut selected, &self.state, document);
					snap_again(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
				}
			}
			TransformLayerMessage::TypeDigit { digit } => {
				if self.transform_operation.can_begin_typing() {
					self.transform_operation.grs_typed(self.typing.type_number(digit), &mut selected, &self.state, document);
					snap_again(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
				}
			}
			TransformLayerMessage::TypeNegate => {
				if self.typing.digits.is_empty() {
					self.transform_operation.negate(&mut selected, &self.state, document);
				}
				self.transform_operation.grs_typed(self.typing.type_negate(), &mut selected, &self.state, document);
				snap_again(&mut self.transform_operation, self.snapping.as_ref(), &self.state, view, snap_session, &mut selected);
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
		self.transform_operation.hints(responses, self.state.is_transforming_in_local_space, self.state.is_snapping);
	}

	/// Ends the snapping gesture of the transform, if it began one, and takes down what it showed.
	fn end_snapping(&mut self, snap_session: &mut SnapSession, responses: &mut VecDeque<Message>) {
		self.state.is_snapping = false;
		if self.snapping.take().is_some() {
			snap_session.end_gesture();
			responses.add(OverlaysMessage::Draw);
		}
	}

	fn set_ghost_outline(ghost_outline: &mut Vec<(Vec<ClickTargetType>, DAffine2)>, shape_editor: &ShapeState, document: &DocumentMessageHandler) {
		ghost_outline.clear();
		for &layer in shape_editor.selected_shape_state.keys() {
			// Mirror the Path tool's view (e.g. pre-solidified centerline) when an upstream Path node exists,
			// otherwise fall back to the layer's recorded outline geometry
			let (outline, transform) = if let Some(targets) = document.network_interface.path_aware_outline_targets(layer) {
				(targets, document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface))
			} else {
				(
					document.metadata().layer_with_free_points_outline(layer).cloned().collect(),
					document.metadata().transform_to_viewport(layer),
				)
			};
			ghost_outline.push((outline, transform));
		}
	}
}

/// What a transform snaps by, built as it begins so that an engaged snap holds from move to move.
#[derive(Clone, Debug, Default)]
struct Snapping {
	/// Where the pointer took hold, in document space.
	grip: DVec2,
	/// Where each feature begins in document space: the corners, edge middles, and center of the box around the transformed layers, then their nearest anchors, or else the selected points.
	starts: Vec<DVec2>,
	/// Which starts make up the box around the transformed layers, if they have one.
	box_features: Option<BoxFeatures>,
	/// The box corner nearest the grip, or without a box, the start nearest it.
	nearest: usize,
	/// The features the operation under way snaps by.
	features: Vec<FeatureMap>,
	/// The feature the pointer carries.
	lead: usize,
	/// What the features are snapped with, apart from the track a constraint holds them to.
	options: PointOptions,
}

impl Snapping {
	/// Gathers what transformed layers snap by, given their box in the viewport and the grip.
	fn of_layers(document: &DocumentMessageHandler, layers: &[LayerNodeIdentifier], viewport_box: Quad, grip: DVec2) -> Self {
		let metadata = document.metadata();
		let mut snapping = Self { grip, ..Default::default() };

		// The box is measured in the view the latest render was made with, which can lag a zoom or tilt
		if layers.iter().any(|&layer| metadata.bounding_box_viewport(layer).is_some()) {
			let corners = (metadata.rendered_document_to_viewport.inverse() * viewport_box).0;
			let middles = [0, 1, 2, 3].map(|i| (corners[i] + corners[(i + 1) % 4]) / 2.);
			snapping.starts.extend(corners.into_iter().chain(middles));
			snapping.starts.push(corners.iter().sum::<DVec2>() / 4.);

			// Along each document axis, the corner furthest toward either end carries that side of the box
			let side = |axis: usize, toward_high: bool| {
				let along = |&index: &usize| if toward_high { corners[index][axis] } else { -corners[index][axis] };
				(0..4).max_by(|a, b| along(a).total_cmp(&along(b)))
			};
			snapping.box_features = Some(BoxFeatures {
				corners: [0, 1, 2, 3],
				sides: [0, 1].map(|axis| [side(axis, false), side(axis, true)]),
			});

			let distance_to_grip = |index: &usize| corners[*index].distance_squared(grip);
			snapping.nearest = (0..4).min_by(|a, b| distance_to_grip(a).total_cmp(&distance_to_grip(b))).unwrap_or_default();
		}

		push_nearest_anchors(&mut snapping.starts, document, layers, grip);
		snapping
	}

	/// Gathers what the selected points of transformed layers snap by, and what moves with them, given the grip.
	fn of_points(document: &DocumentMessageHandler, layers: &[LayerNodeIdentifier], shape_editor: &ShapeState, grip: DVec2) -> (Self, Moving) {
		let mut moving = Moving::default();
		let mut starts = Vec::new();

		for &layer in layers {
			let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
			let to_document = document.metadata().transform_to_document_as_rendered_if_feeds(layer, &document.network_interface);

			// A selected segment moves the anchors at its ends
			let selected_segments = shape_editor.selected_segments_in_layer(layer);
			let segment_ends = vector
				.segment_iter()
				.filter(|(segment, ..)| selected_segments.is_some_and(|selected| selected.contains(segment)))
				.flat_map(|(_, _, start, end)| [ManipulatorPointId::Anchor(start), ManipulatorPointId::Anchor(end)]);
			let points: Vec<ManipulatorPointId> = shape_editor.selected_points_in_layer(layer).into_iter().flatten().copied().chain(segment_ends).collect();
			if points.is_empty() {
				continue;
			}

			moving.edited.push(layer);
			for point in points {
				if let Some(anchor) = vector.anchor_of(point) {
					moving.anchors.push((layer, anchor));
				}
				if let Some(position) = vector.position_of(point) {
					starts.push(to_document.transform_point2(position));
				}
			}
		}

		// Points far from the grip seldom decide where a transform lands, so only the nearest snap. O(n log n) in the selected points.
		starts.retain(|start| start.is_finite());
		starts.sort_by(|a, b| a.distance_squared(grip).total_cmp(&b.distance_squared(grip)));
		starts.dedup_by(|a, b| a.abs_diff_eq(*b, 1e-6));
		starts.truncate(MAX_SNAPPING_POINTS);

		(Self { grip, starts, ..Default::default() }, moving)
	}

	/// Builds the features an operation snaps by from the starts, which stay the same from move to move while its constraint does.
	fn build(&mut self, operation: &TransformOperation, state: &TransformationState, document: &DocumentMessageHandler) {
		let (pivot, grip) = (state.document_space_pivot, self.grip);
		self.features.clear();
		self.lead = 0;
		self.options = PointOptions::default();

		match operation {
			TransformOperation::None => {}
			TransformOperation::Grabbing(_) => {
				self.features = self.starts.iter().copied().map(FeatureMap::sliding_from).collect();
				self.lead = self.nearest;
				self.options.box_features = self.box_features;
			}
			TransformOperation::Rotating(_) => {
				let moves = |start: &DVec2| start.distance(pivot) > MIN_FEATURE_TRAVEL * grip.distance(pivot);
				let turned = |start: DVec2| FeatureMap::turning_about(start, grip, pivot);
				self.features = self.starts.iter().copied().filter(moves).chain([grip]).filter_map(turned).collect();
			}
			TransformOperation::Scaling(scale) => {
				// The axes the scale is applied along, in document space
				let to_document = document.metadata().document_to_viewport.inverse() * state.local_to_viewport_transform();
				let [Some(x_axis), Some(y_axis)] = [DVec2::X, DVec2::Y].map(|axis| to_document.transform_vector2(axis).try_normalize()) else {
					return;
				};
				let axes = [x_axis, y_axis];

				// A scale held at 1 by a constraint gets a direction of its own, so features level with the pivot along that axis can still snap
				let held = match scale.constraint {
					Axis::Both => None,
					Axis::X => Some(1),
					Axis::Y => Some(0),
				};
				let stretched = |start: DVec2| {
					let mut feature = FeatureMap::stretching(start, pivot, axes);
					if let Some(axis) = held {
						let column = feature.matrix.col_mut(axis);
						feature.offset += *column - axes[axis];
						*column = axes[axis];
					}
					feature
				};

				self.features = self.starts.iter().copied().chain([grip]).map(stretched).collect();
			}
		}

		// A turn or a scale is carried by the grip, which comes last and only follows
		if matches!(operation, TransformOperation::Rotating(_) | TransformOperation::Scaling(_)) && !self.features.is_empty() {
			self.lead = self.features.len() - 1;
			self.options.snapping = Some(self.lead);
			self.options.ignores_grid = true;
		}
	}

	/// Snaps the amount the pointer asks of an operation, which keeps that amount and applies the snapped one in its place.
	fn snap(&self, operation: &mut TransformOperation, state: &TransformationState, view: SnapView, snap_session: &mut SnapSession) {
		operation.clear_snapped();

		// A typed value or a step of the increments is applied as it is
		if operation.is_typing() || state.is_rounded_to_intervals || self.features.is_empty() {
			snap_session.forget();
			return;
		}

		let document_to_viewport = view.document.metadata().document_to_viewport;
		let viewport_to_document = document_to_viewport.inverse();
		let mut snap = |free: DVec2, required: Option<Locus>| {
			let options = PointOptions { required, ..self.options };
			snap_session.snap_features(view, &self.features, self.lead, free, options)
		};

		match operation {
			TransformOperation::None => {}
			TransformOperation::Grabbing(translation) => {
				// The displacement is taken into document space the way it is applied, and the snapped one is brought back the same way
				let free = state.local_to_viewport_transform().transform_vector2(translation.to_dvec(state, view.document));
				let along_constraint = state.constraint_axis(translation.constraint);
				let required = along_constraint.and_then(|axis| Locus::line(DVec2::ZERO, viewport_to_document.transform_vector2(axis)));

				let slid = snap(viewport_to_document.transform_vector2(free), required);
				translation.snapped = Some(document_to_viewport.transform_vector2(slid) / document_to_viewport.matrix2.y_axis.length());
			}
			TransformOperation::Rotating(rotation) => {
				// Measured in the viewport, where the turn is applied, which a flipped canvas mirrors
				let seen = |point: DVec2| document_to_viewport.transform_point2(point);
				let (pivot, grip) = (state.document_space_pivot, self.grip);
				let free = seen(pivot) + DVec2::from_angle(rotation.dragged_angle).rotate(seen(grip) - seen(pivot));

				let turned = snap(viewport_to_document.transform_point2(free), Locus::circle(pivot, pivot.distance(grip)));
				let Some(angle) = (seen(grip) - seen(pivot)).try_angle_to(seen(turned) - seen(pivot)) else {
					return;
				};

				// The snapped angle keeps the whole turns the pointer has wound through
				rotation.snapped = Some(rotation.dragged_angle + (angle - rotation.dragged_angle + PI).rem_euclid(TAU) - PI);
			}
			TransformOperation::Scaling(scale) => {
				// The factors along both axes, with a constraint holding the other one at 1
				let factor = scale.dragged_factor;
				let (free, required) = match scale.constraint {
					Axis::Both => (DVec2::splat(factor), Locus::line(DVec2::ZERO, DVec2::ONE)),
					Axis::X => (DVec2::new(factor, 1.), Locus::line(DVec2::new(0., 1.), DVec2::X)),
					Axis::Y => (DVec2::new(1., factor), Locus::line(DVec2::new(1., 0.), DVec2::Y)),
				};

				let stretched = snap(free, required);
				scale.snapped = Some(if scale.constraint == Axis::Y { stretched.y } else { stretched.x });
			}
		}
	}
}

/// Snaps what the pointer asks of the operation, if the transform is snapping, then applies it.
fn snap_and_apply(operation: &mut TransformOperation, snapping: Option<&Snapping>, state: &TransformationState, view: SnapView, snap_session: &mut SnapSession, selected: &mut Selected) {
	if let Some(snapping) = snapping {
		snapping.snap(operation, state, view, snap_session);
	}
	operation.apply_transform_operation(selected, state, view.document);
}

/// Snaps the operation again once a key has changed its amount, if the transform is snapping, and applies the snapped amount. A typed amount, which the key already applied, is left as it is.
fn snap_again(operation: &mut TransformOperation, snapping: Option<&Snapping>, state: &TransformationState, view: SnapView, snap_session: &mut SnapSession, selected: &mut Selected) {
	let Some(snapping) = snapping else { return };
	snapping.snap(operation, state, view, snap_session);

	if !operation.is_typing() {
		operation.apply_transform_operation(selected, state, view.document);
	}
}

fn calculate_pivot(
	document: &DocumentMessageHandler,
	selected_points: &Vec<&ManipulatorPointId>,
	vector: &VectorGeometry,
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
		(if gizmo.state.enabled {
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
				vector.anchor_position_of(**point).map(|anchor_position| viewspace.transform_point2(anchor_position)),
				vector.position_of(**point),
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

fn update_colinear_handles(selected_layers: &[LayerNodeIdentifier], document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	for &layer in selected_layers {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

		for [handle1, handle2] in vector.colinear_pairs() {
			let manipulator1 = handle1.to_manipulator_point();
			let manipulator2 = handle2.to_manipulator_point();

			let Some(anchor_pos) = vector.anchor_position_of(manipulator1) else { continue };
			let Some(handle1_pos) = vector.position_of(manipulator1) else { continue };
			let Some(handle2_pos) = vector.position_of(manipulator2) else { continue };

			// Check if handles are not colinear (not approximately equal to +/- PI)
			if !is_almost_colinear(anchor_pos, handle1_pos, handle2_pos) {
				let modification_type = VectorModificationType::SetG1Continuous {
					handles: [handle1, handle2],
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
	use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
	use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
	use crate::messages::portfolio::document::utility_types::transformation::TransformOperation;
	use crate::messages::prelude::Message;
	use crate::messages::tool::transform_layer::transform_layer_message_handler::{TransformLayerMessageHandler, VectorModificationType};
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::PointId;
	use std::collections::VecDeque;

	async fn get_layer_transform(editor: &mut EditorTestUtils, layer: LayerNodeIdentifier) -> Option<DAffine2> {
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		let _responses: VecDeque<Message> = VecDeque::new();
		let transform_node_id = ModifyInputsContext::locate_node_in_layer_chain(&DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER), layer, network_interface)?;
		let document_node = network_interface.document_network().nodes.get(&transform_node_id)?;
		Some(transform_utils::get_current_transform(&document_node.inputs))
	}

	/// Draws a rectangle between two viewport positions, which leaves it selected, and returns its layer.
	async fn draw_rectangle(editor: &mut EditorTestUtils, from: DVec2, to: DVec2) -> LayerNodeIdentifier {
		editor.draw_rect(from.x, from.y, to.x, to.y).await;
		let layer = editor.get_selected_layer().await.expect("the drawn rectangle should be selected");
		assert_bounds(editor, layer, [from, to]);
		layer
	}

	/// Asserts the least and greatest corners of a layer's bounding box in the viewport.
	fn assert_bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier, [expected_min, expected_max]: [DVec2; 2]) {
		let [min, max] = editor.active_document().metadata().bounding_box_viewport(layer).expect("the layer should have a bounding box");
		assert!(
			min.distance(expected_min) < 1e-6 && max.distance(expected_max) < 1e-6,
			"the layer spans {min} to {max} rather than {expected_min} to {expected_max}"
		);
	}

	/// Moves the pointer to a viewport position with no button held, which a transform under way follows.
	async fn move_pointer(editor: &mut EditorTestUtils, position: DVec2, modifier_keys: ModifierKeys) {
		editor.move_mouse(position.x, position.y, modifier_keys, MouseKeys::NONE).await;
	}

	/// Begins a transform with the pointer taking hold at a viewport position.
	async fn begin_at(editor: &mut EditorTestUtils, begin: TransformLayerMessage, grip: DVec2) {
		move_pointer(editor, grip, ModifierKeys::empty()).await;
		editor.handle_message(begin).await;
	}

	fn transform_layer_handler(editor: &EditorTestUtils) -> &TransformLayerMessageHandler {
		&editor.editor.dispatcher.message_handlers.tool_message_handler.transform_layer_handler
	}

	fn shows_a_snap(editor: &EditorTestUtils) -> bool {
		editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown().is_some()
	}

	/// A rectangle turned about its middle by a pointer taking hold just beyond its bottom right corner, which is a hundred from the middle.
	const TURNED: [DVec2; 2] = [DVec2::new(290., 270.), DVec2::new(410., 430.)];
	const TURNED_MIDDLE: DVec2 = DVec2::new(350., 350.);
	const TURN_GRIP: DVec2 = DVec2::new(424., 444.);

	/// Where the turn with a cosine of 0.6 and a sine of 0.8 carries the bottom right corner of [`TURNED`].
	const ANCHOR_IN_THE_TURN: DVec2 = DVec2::new(322., 446.);

	/// Where a turn about a pivot through an angle in radians carries a position.
	fn turned(position: DVec2, pivot: DVec2, angle: f64) -> DVec2 {
		pivot + DVec2::from_angle(angle).rotate(position - pivot)
	}

	/// The least and greatest corners of the box around a rectangle once turned about a pivot through an angle in radians.
	fn turned_bounds([from, to]: [DVec2; 2], pivot: DVec2, angle: f64) -> [DVec2; 2] {
		let corners = [from, DVec2::new(to.x, from.y), to, DVec2::new(from.x, to.y)].map(|corner| turned(corner, pivot, angle));
		[corners.into_iter().fold(DVec2::INFINITY, DVec2::min), corners.into_iter().fold(DVec2::NEG_INFINITY, DVec2::max)]
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
		assert!(translation_diff > 10., "Transform should have changed after applying transformation. Diff: {translation_diff}");
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
			"Transform neither restored to original nor reset to identity. Original: {original_translation:?}, Final: {final_translation:?}"
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
		println!("Final transform: {final_transform:?}");

		// Check matrix components have changed (rotation affects matrix2)
		let matrix_diff = (final_transform.matrix2.x_axis - original_transform.matrix2.x_axis).length();
		assert!(matrix_diff > 0.1, "Rotation should have changed the transform matrix. Diff: {matrix_diff}");
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
		assert!(translation_diff < 1., "Translation component changed too much: {translation_diff}");
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
			"Scaling should have changed the transform matrix. Diffs: x={scale_diff_x}, y={scale_diff_y}"
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
		assert!(translation_diff < 1., "Translation component changed too much: {translation_diff}");
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
			"Expected translation of {expected_translation:?}, got {actual_translation:?}"
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
			"Scale should increase the determinant of the matrix (before: {before_scale_det}, after: {after_scale_det})"
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

		assert!((scale_x - 2.).abs() < 0.1, "Expected scale factor X of 2, got: {scale_x}");
		assert!((scale_y - 2.).abs() < 0.1, "Expected scale factor Y of 2, got: {scale_y}");
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

		assert!((scale_x - 2.).abs() < 0.1, "Expected scale factor X of 2, got: {scale_x}");
		assert!((scale_y - 2.).abs() < 0.1, "Expected scale factor Y of 2, got: {scale_y}");
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
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 45_f64.to_radians() }).await;
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
		assert!((angle_change - 90.).abs() < 0.1, "Expected rotation of 90 degrees, got: {angle_change}");
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
		assert!(scale_x < 0.001, "Scale factor X should be near zero, got: {scale_x}");
		assert!(scale_y < 0.001, "Scale factor Y should be near zero, got: {scale_y}");
		assert!(scale_x > 0., "Scale factor X should not be exactly zero");
		assert!(scale_y > 0., "Scale factor Y should not be exactly zero");

		editor.handle_message(TransformLayerMessage::BeginScale).await;
		editor.handle_message(TransformLayerMessage::TypeDigit { digit: 2 }).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let final_transform = get_layer_transform(&mut editor, layer).await.unwrap();
		assert!(final_transform.is_finite(), "Transform should be finite after rescaling");

		let new_scale_x = final_transform.matrix2.x_axis.length();
		let new_scale_y = final_transform.matrix2.y_axis.length();
		assert!(new_scale_x > 0., "After rescaling, scale factor X should be nonzero");
		assert!(new_scale_y > 0., "After rescaling, scale factor Y should be nonzero");
	}

	#[tokio::test]
	async fn test_scale_starting_on_pivot() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		// The pivot is at the center of the rectangle
		editor.move_mouse(50., 50., ModifierKeys::empty(), MouseKeys::NONE).await;
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
		assert!(final_transform.is_finite(), "Scaling from the pivot should leave the transform finite, got: {final_transform:?}");
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

	#[tokio::test]
	async fn a_grabbed_rectangle_brought_near_a_corner_of_another_lands_on_it_and_follows_the_pointer_again_once_moved_on() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;

		// Its top left corner is brought three pixels from the other's bottom right corner one way, and two the other
		let grip = DVec2::new(330., 320.);
		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		move_pointer(&mut editor, grip + DVec2::new(-97., -122.), ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(260., 230.)]);

		// The distance shown is the snapped one, while the pointer's own goes on from where the pointer is
		let handler = transform_layer_handler(&editor);
		let TransformOperation::Grabbing(translation) = handler.transform_operation else {
			panic!("a grab should be under way")
		};
		assert!(translation.to_dvec(&handler.state, editor.active_document()).abs_diff_eq(DVec2::new(-100., -120.), 1e-6));
		assert!(translation.dragged_distance.abs_diff_eq(DVec2::new(-97., -122.), 1e-6));

		// Far from anything, it has moved exactly as far as the pointer
		move_pointer(&mut editor, grip + DVec2::new(180., 60.), ModifierKeys::empty()).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		assert_bounds(&editor, layer, [DVec2::new(480., 360.), DVec2::new(540., 410.)]);
	}

	#[tokio::test]
	async fn a_rectangle_grabbed_along_x_stays_level_and_still_lands_on_a_corner_along_that_line() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 180.), DVec2::new(360., 230.)).await;

		// Its top left corner is brought three pixels short of the other's bottom right corner, with the pointer straying downward
		let grip = DVec2::new(330., 200.);
		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		editor.handle_message(TransformLayerMessage::ConstrainX).await;
		move_pointer(&mut editor, grip + DVec2::new(-97., 6.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(260., 230.)]);
	}

	#[tokio::test]
	async fn a_typed_distance_and_one_stepped_by_control_are_applied_exactly_beside_a_corner_they_would_otherwise_snap_to() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(300., 100.), DVec2::new(400., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(100., 180.), DVec2::new(160., 230.)).await;
		let grip = DVec2::new(130., 200.);
		let near_the_corner = grip + DVec2::new(137., 4.);

		// Grabbed along X to three pixels short of the other's bottom left corner, its top right corner lands on it
		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		editor.handle_message(TransformLayerMessage::ConstrainX).await;
		move_pointer(&mut editor, near_the_corner, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(240., 180.), DVec2::new(300., 230.)]);
		assert!(shows_a_snap(&editor));

		// Typed, that distance is applied as it is, with nothing shown as snapped
		for digit in [1, 3, 7] {
			editor.handle_message(TransformLayerMessage::TypeDigit { digit }).await;
		}
		assert_bounds(&editor, layer, [DVec2::new(237., 180.), DVec2::new(297., 230.)]);
		assert!(!shows_a_snap(&editor));

		// With the digits deleted, the pointer's own distance snaps again
		for _ in 0..3 {
			editor.handle_message(TransformLayerMessage::TypeBackspace).await;
		}
		assert_bounds(&editor, layer, [DVec2::new(240., 180.), DVec2::new(300., 230.)]);
		assert!(shows_a_snap(&editor));
		editor.handle_message(TransformLayerMessage::CancelTransformOperation).await;

		// With Control held, it is only stepped to a whole number of pixels
		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		editor.handle_message(TransformLayerMessage::ConstrainX).await;
		move_pointer(&mut editor, near_the_corner, ModifierKeys::CONTROL).await;
		assert_bounds(&editor, layer, [DVec2::new(237., 180.), DVec2::new(297., 230.)]);
		assert!(!shows_a_snap(&editor));
	}

	#[tokio::test]
	async fn a_rectangle_rotated_until_its_corner_nears_an_anchor_in_its_path_lands_with_the_corner_on_it_at_its_own_size() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, ANCHOR_IN_THE_TURN - DVec2::new(10., 0.), ANCHOR_IN_THE_TURN + DVec2::new(0., 10.)).await;
		let layer = draw_rectangle(&mut editor, TURNED[0], TURNED[1]).await;
		let original = get_layer_transform(&mut editor, layer).await.expect("the rectangle should have a transform");

		// Two degrees short of the anchor, the bottom right corner is a few pixels from it
		let onto_the_anchor = DVec2::new(0.6, 0.8).to_angle();
		begin_at(&mut editor, TransformLayerMessage::BeginRotate, TURN_GRIP).await;
		move_pointer(&mut editor, turned(TURN_GRIP, TURNED_MIDDLE, onto_the_anchor - 2_f64.to_radians()), ModifierKeys::empty()).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		assert_bounds(&editor, layer, turned_bounds(TURNED, TURNED_MIDDLE, onto_the_anchor));

		// It only turned, keeping its size
		let rotated = get_layer_transform(&mut editor, layer).await.expect("the rectangle should have a transform");
		let size = |transform: DAffine2| DVec2::new(transform.matrix2.x_axis.length(), transform.matrix2.y_axis.length());
		assert!(size(rotated).abs_diff_eq(size(original), 1e-9), "the rectangle was scaled from {} to {}", size(original), size(rotated));
	}

	#[tokio::test]
	async fn a_rectangle_scaled_until_its_corner_nears_a_corner_of_another_lands_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(390., 360.), DVec2::new(450., 420.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 340.)).await;

		// Taken hold of by its bottom right corner and scaled about its middle by 1.9, which leaves that corner a few pixels from the other's top left corner
		begin_at(&mut editor, TransformLayerMessage::BeginScale, DVec2::new(360., 340.)).await;
		move_pointer(&mut editor, DVec2::new(387., 358.), ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(270., 280.), DVec2::new(390., 360.)]);

		// The factor shown is the snapped one, while the pointer's own stays where it asked
		let TransformOperation::Scaling(scale) = transform_layer_handler(&editor).transform_operation else {
			panic!("a scale should be under way")
		};
		assert!((scale.to_f64(false) - 2.).abs() < 1e-9 && (scale.dragged_factor - 1.9).abs() < 1e-9);
	}

	#[tokio::test]
	async fn a_rectangle_scaled_along_x_changes_only_its_width_and_still_lands_on_a_corner_along_that_line() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(390., 340.), DVec2::new(450., 400.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 340.)).await;

		// Scaled by 1.9 across its middle, its bottom right corner comes three pixels short of the other's top left corner, with the pointer straying downward
		begin_at(&mut editor, TransformLayerMessage::BeginScale, DVec2::new(360., 330.)).await;
		editor.handle_message(TransformLayerMessage::ConstrainX).await;
		move_pointer(&mut editor, DVec2::new(387., 333.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(270., 300.), DVec2::new(390., 340.)]);
	}

	#[tokio::test]
	async fn escape_puts_a_snapped_grab_back_and_neither_it_nor_a_confirmed_grab_leaves_anything_shown_as_snapped() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;
		let grip = DVec2::new(330., 320.);
		let near_the_corner = grip + DVec2::new(-97., -122.);

		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		move_pointer(&mut editor, near_the_corner, ModifierKeys::empty()).await;
		assert!(shows_a_snap(&editor));

		editor.handle_message(TransformLayerMessage::CancelTransformOperation).await;
		assert_bounds(&editor, layer, [DVec2::new(300., 300.), DVec2::new(360., 350.)]);
		assert!(!shows_a_snap(&editor));

		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		move_pointer(&mut editor, near_the_corner, ModifierKeys::empty()).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(260., 230.)]);
		assert!(!shows_a_snap(&editor));
	}

	#[tokio::test]
	async fn a_path_anchor_grabbed_near_a_corner_of_a_rectangle_lands_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(400., 300., 500., 380.).await;

		// A path is drawn with the Pen in a layer of its own
		let (first, second) = (DVec2::new(100., 100.), DVec2::new(200., 220.));
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;
		for anchor in [first, second] {
			editor.click_tool(ToolType::Pen, MouseKeys::LEFT, anchor, ModifierKeys::empty()).await;
		}
		editor.press(Key::Enter, ModifierKeys::empty()).await;
		let layer = editor.get_selected_layer().await.expect("the drawn path should be selected");

		// Its second anchor is selected with the Path tool, then grabbed to a few pixels from the rectangle's top left corner
		editor.click_tool(ToolType::Path, MouseKeys::LEFT, second, ModifierKeys::empty()).await;
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		move_pointer(&mut editor, DVec2::new(404., 297.), ModifierKeys::empty()).await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		let document = editor.active_document();
		let vector = document.network_interface.compute_modified_vector(layer).expect("the path should have a vector");
		let to_viewport = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		let anchors: Vec<DVec2> = vector.anchor_positions().iter().map(|&position| to_viewport.transform_point2(position)).collect();
		assert_eq!(anchors.len(), 2, "unexpected anchors {anchors:?}");
		assert!(
			anchors[0].distance(first) < 1e-6 && anchors[1].distance(DVec2::new(400., 300.)) < 1e-6,
			"unexpected anchors {anchors:?}"
		);
	}

	#[tokio::test]
	async fn a_quarter_turned_rectangle_grabbed_along_its_own_x_axis_moves_only_along_that_axis() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 140.)).await;

		// Turned a quarter turn about its middle, its own X axis runs down the viewport
		begin_at(&mut editor, TransformLayerMessage::BeginRotate, DVec2::new(250., 120.)).await;
		for digit in [9, 0] {
			editor.handle_message(TransformLayerMessage::TypeDigit { digit }).await;
		}
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;
		assert_bounds(&editor, layer, [DVec2::new(130., 70.), DVec2::new(170., 170.)]);

		// Pressing X twice holds the grab to that axis, so only the pointer's downward travel moves it
		let grip = DVec2::new(150., 120.);
		begin_at(&mut editor, TransformLayerMessage::BeginGrab, grip).await;
		editor.handle_message(TransformLayerMessage::ConstrainX).await;
		editor.handle_message(TransformLayerMessage::ConstrainX).await;
		move_pointer(&mut editor, grip + DVec2::new(30., 50.), ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(130., 120.), DVec2::new(170., 220.)]);
	}
}
