use crate::consts::{SLOWING_DIVISOR, COLOR_OVERLAY_WHITE, COLOR_OVERLAY_SNAP_BACKGROUND};
use crate::messages::portfolio::document::overlays::utility_types::Pivot;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::{ToolData, ToolType, ToolTransition, EventToMessageMap};


use graphene_core::vector::ManipulatorPointId;

use glam::DVec2;

#[derive(Debug, Clone, Default)]
pub struct TransformLayerMessageHandler {
	pub transform_operation: TransformOperation,

	slow: bool,
	snap: bool,
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
		let axis_constraint = match self.transform_operation {
			TransformOperation::Grabbing(grabbing) => grabbing.constraint,
			TransformOperation::Scaling(scaling) => scaling.constraint,
			_ => Axis::Both,
		};
		self.transform_operation.hints(self.snap, axis_constraint, responses);
	}
}

impl ToolTransition for TransformLayerMessageHandler {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(TransformLayerMessage::CancelTransformOperation.into()),
			overlay_provider: Some(|overlay_context| TransformLayerMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

type TransformData<'a> = (&'a DocumentMessageHandler, &'a InputPreprocessorMessageHandler, &'a ToolData, &'a mut ShapeState);
impl MessageHandler<TransformLayerMessage, TransformData<'_>> for TransformLayerMessageHandler {
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, (document, input, tool_data, shape_editor): TransformData) {
		let using_path_tool = tool_data.active_tool_type == ToolType::Path;

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
					let get_location = |point: &ManipulatorPointId| point.get_position(&vector_data).map(|position| viewspace.transform_point2(position));
					let points = shape_editor.selected_points();

					*selected.pivot = points.filter_map(get_location).inspect(|_| point_count += 1).sum::<DVec2>() / point_count as f64;
				}
			} else {
				*selected.pivot = selected.mean_average_of_pivots();
			}

			*mouse_position = input.mouse.position;
			*start_mouse = input.mouse.position;
			selected.original_transforms.clear();

			selected.responses.add(DocumentMessage::StartTransaction);
		};

		match message {
			TransformLayerMessage::Overlays(overlay_context) => {
				debug!("We were here");
				let axis_constraint = match self.transform_operation {
					TransformOperation::Grabbing(grabbing) => grabbing.constraint,
					TransformOperation::Scaling(scaling) => scaling.constraint,
					_ => Axis::Both,
				};

				let axis_text = |vector: DVec2, separate: bool| match (axis_constraint, separate) {
					(Axis::Both, false) => format!("by {:.3}", vector.x),
					(Axis::Both, true) => format!("by {:.3}, {:.3}", vector.x, vector.y),
					(Axis::X, _) => format!("X by {:.3}", vector.x),
					(Axis::Y, _) => format!("Y by {:.3}", vector.y),
				};
				let grs_value_text = match self.transform_operation {
					TransformOperation::None => String::new(),
					// TODO: Fix that the translation is showing numbers in viewport space, not document space
					TransformOperation::Grabbing(translation) => format!("Translating {}", axis_text(translation.to_dvec(), true)),
					TransformOperation::Rotating(rotation) => format!("Rotating by {:.3}°", rotation.to_f64(self.snap) * 360. / std::f64::consts::TAU),
					TransformOperation::Scaling(scale) => format!("Scaling {}", axis_text(scale.to_dvec(self.snap), false)),
				};
				let transform = glam::DAffine2::from_translation(  DVec2::new(327.0, -35.0));
				overlay_context.text(&grs_value_text, COLOR_OVERLAY_WHITE, Some(COLOR_OVERLAY_SNAP_BACKGROUND), transform, 4., [Pivot::Start, Pivot::End]);
			}
			TransformLayerMessage::ApplyTransformOperation => {
				selected.original_transforms.clear();

				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(DocumentMessage::EndTransaction);
				responses.add(ToolMessage::UpdateHints);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			TransformLayerMessage::BeginGrab => {
				if let TransformOperation::Grabbing(_) = self.transform_operation {
					return;
				}

				// Don't allow grab with no selected layers
				if selected_layers.is_empty() {
					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Grabbing(Default::default());

				selected.original_transforms.clear();

				responses.add(OverlaysMessage::Draw);
			}
			TransformLayerMessage::BeginRotate => {
				if let TransformOperation::Rotating(_) = self.transform_operation {
					return;
				}

				// Don't allow rotate with no selected layers
				if selected_layers.is_empty() {
					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Rotating(Default::default());

				selected.original_transforms.clear();

				responses.add(OverlaysMessage::Draw);
			}
			TransformLayerMessage::BeginScale => {
				if let TransformOperation::Scaling(_) = self.transform_operation {
					return;
				}

				// Don't allow scale with no selected layers
				if selected_layers.is_empty() {
					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Scaling(Default::default());

				selected.original_transforms.clear();

				responses.add(OverlaysMessage::Draw);
			}
			TransformLayerMessage::CancelTransformOperation => {
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(DocumentMessage::AbortTransaction);
				responses.add(ToolMessage::UpdateHints);
			}
			TransformLayerMessage::ConstrainX => self.transform_operation.constrain_axis(Axis::X, &mut selected, self.snap),
			TransformLayerMessage::ConstrainY => self.transform_operation.constrain_axis(Axis::Y, &mut selected, self.snap),
			TransformLayerMessage::PointerMove { slow_key, snap_key } => {
				self.slow = input.keyboard.get(slow_key as usize);

				let new_snap = input.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					let axis_constraint = match self.transform_operation {
						TransformOperation::Grabbing(grabbing) => grabbing.constraint,
						TransformOperation::Scaling(scaling) => scaling.constraint,
						_ => Axis::Both,
					};
					self.transform_operation.apply_transform_operation(&mut selected, self.snap, axis_constraint);
				}

				if self.typing.digits.is_empty() {
					let delta_pos = input.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							let axis_constraint = translation.constraint;
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, axis_constraint);
						}
						TransformOperation::Rotating(rotation) => {
							let start_offset = *selected.pivot - self.mouse_position;
							let end_offset = *selected.pivot - input.mouse.position;
							let angle = start_offset.angle_to(end_offset);

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };

							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, Axis::Both);
						}
						TransformOperation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - *selected.pivot).length();
								let current_frame_dist = (input.mouse.position - *selected.pivot).length();
								let start_transform_dist = (self.start_mouse - *selected.pivot).length();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};

							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							let axis_constraint = scale.constraint;
							self.transform_operation = TransformOperation::Scaling(scale.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, axis_constraint);
						}
					};
				}
				self.mouse_position = input.mouse.position;
			}
			TransformLayerMessage::SelectionChanged => {
				let target_layers = document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()).collect();
				shape_editor.set_selected_layers(target_layers);
			}
			TransformLayerMessage::TypeBackspace => self.transform_operation.grs_typed(self.typing.type_backspace(), &mut selected, self.snap),
			TransformLayerMessage::TypeDecimalPoint => self.transform_operation.grs_typed(self.typing.type_decimal_point(), &mut selected, self.snap),
			TransformLayerMessage::TypeDigit { digit } => self.transform_operation.grs_typed(self.typing.type_number(digit), &mut selected, self.snap),
			TransformLayerMessage::TypeNegate => self.transform_operation.grs_typed(self.typing.type_negate(), &mut selected, self.snap),
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
