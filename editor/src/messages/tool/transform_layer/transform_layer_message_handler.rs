use crate::consts::SLOWING_DIVISOR;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::{ToolData, ToolType};

use document_legacy::layers::style::RenderData;
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

type TransformData<'a> = (&'a DocumentMessageHandler, &'a InputPreprocessorMessageHandler, &'a RenderData<'a>, &'a ToolData, &'a mut ShapeState);
impl<'a> MessageHandler<TransformLayerMessage, TransformData<'a>> for TransformLayerMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, (document, ipp, render_data, tool_data, shape_editor): TransformData) {
		use TransformLayerMessage::*;

		let using_path_tool = tool_data.active_tool_type == ToolType::Path;

		let selected_layers = document.layer_metadata.iter().filter_map(|(layer_path, data)| data.selected.then_some(layer_path)).collect::<Vec<_>>();

		let mut selected = Selected::new(
			&mut self.original_transforms,
			&mut self.pivot,
			&selected_layers,
			responses,
			&document.document_legacy,
			Some(shape_editor),
			&tool_data.active_tool_type,
		);

		let mut begin_operation = |operation: TransformOperation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2| {
			if operation != TransformOperation::None {
				selected.revert_operation();
				typing.clear();
			}
			let transform = document.document_legacy.root.transform;

			if using_path_tool {
				if let Ok(layer) = document.document_legacy.layer(selected_layers[0]) {
					if let Some(vector_data) = layer.as_vector_data() {
						*selected.original_transforms = OriginalTransforms::default();
						let viewspace = &mut document.document_legacy.generate_transform_relative_to_viewport(selected_layers[0]).ok().unwrap_or_default();

						let mut point_count: usize = 0;
						let count_point = |position| {
							point_count += 1;
							position
						};
						let get_location = |point: &ManipulatorPointId| {
							vector_data
								.manipulator_from_id(point.group)
								.and_then(|manipulator_group| point.manipulator_type.get_position(manipulator_group))
								.map(|position| viewspace.transform_point2(position))
						};
						let points = shape_editor.selected_points();

						let viewport_pivot = points.filter_map(get_location).map(count_point).sum::<DVec2>() / point_count as f64;
						*selected.pivot = transform.inverse().transform_point2(viewport_pivot);
					}
				}
			} else {
				let viewport_pivot = selected.mean_average_of_pivots(render_data);
				*selected.pivot = transform.inverse().transform_point2(viewport_pivot);
			}
			*mouse_position = ipp.mouse.position;
			*start_mouse = ipp.mouse.position;
			selected.original_transforms.clear();
		};

		#[remain::sorted]
		match message {
			ApplyTransformOperation => {
				selected.original_transforms.clear();

				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(ToolMessage::UpdateHints);
				responses.add(BroadcastEvent::DocumentIsDirty);
				for layer_path in document.selected_layers() {
					responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: layer_path.to_vec() });
				}
			}
			BeginGrab => {
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
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			BeginRotate => {
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
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			BeginScale => {
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
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			CancelTransformOperation => {
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(ToolMessage::UpdateHints);
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			ConstrainX => self.transform_operation.constrain_axis(Axis::X, &mut selected, self.snap, document.grid_enabled),
			ConstrainY => self.transform_operation.constrain_axis(Axis::Y, &mut selected, self.snap, document.grid_enabled),
			PointerMove { slow_key, snap_key } => {
				let doc_transform = document.document_legacy.root.transform;
				let new_pivot = doc_transform.transform_point2(*selected.pivot);

				self.slow = ipp.keyboard.get(slow_key as usize);

				let new_snap = ipp.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					let axis_constraint = match self.transform_operation {
						TransformOperation::Grabbing(grabbing) => grabbing.constraint,
						TransformOperation::Scaling(scaling) => scaling.constraint,
						_ => Axis::Both,
					};
					self.transform_operation
						.apply_transform_operation(&mut selected, self.snap, axis_constraint, document.grid_enabled, None);
				}

				if self.typing.digits.is_empty() {
					let delta_pos = ipp.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							let axis_constraint = translation.constraint;
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.snap, axis_constraint, document.grid_enabled, Some(change));
						}
						TransformOperation::Rotating(rotation) => {
							let start_offset = new_pivot - self.mouse_position;
							let end_offset = new_pivot - ipp.mouse.position;
							let angle = start_offset.angle_between(end_offset);

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };

							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, Axis::Both, document.grid_enabled, None);
						}
						TransformOperation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - new_pivot).length();
								let current_frame_dist = (ipp.mouse.position - new_pivot).length();
								let start_transform_dist = (self.start_mouse - new_pivot).length();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};

							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							let axis_constraint = scale.constraint;
							self.transform_operation = TransformOperation::Scaling(scale.increment_amount(change));
							self.transform_operation
								.apply_transform_operation(&mut selected, self.snap, axis_constraint, document.grid_enabled, None);
						}
					};
				}
				self.mouse_position = ipp.mouse.position;
			}
			SelectionChanged => {
				let layer_paths = document.selected_visible_layers().map(|layer_path| layer_path.to_vec()).collect();
				shape_editor.set_selected_layers(layer_paths);
			}
			TypeBackspace => self.transform_operation.grs_typed(self.typing.type_backspace(), &mut selected, self.snap, document.grid_enabled),
			TypeDecimalPoint => self.transform_operation.grs_typed(self.typing.type_decimal_point(), &mut selected, self.snap, document.grid_enabled),
			TypeDigit { digit } => self.transform_operation.grs_typed(self.typing.type_number(digit), &mut selected, self.snap, document.grid_enabled),
			TypeNegate => self.transform_operation.grs_typed(self.typing.type_negate(), &mut selected, self.snap, document.grid_enabled),
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
