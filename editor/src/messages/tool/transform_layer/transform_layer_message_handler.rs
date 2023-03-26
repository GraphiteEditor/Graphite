use crate::consts::SLOWING_DIVISOR;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::{ToolData, ToolType};

use document_legacy::layers::style::RenderData;

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

		// TODO: Transform individual points when using the path tool.
		let _using_path_tool = tool_data.active_tool_type == ToolType::Path;

		let selected_layers = document.layer_metadata.iter().filter_map(|(layer_path, data)| data.selected.then_some(layer_path)).collect::<Vec<_>>();
		let mut selected = Selected::new(&mut self.original_transforms, &mut self.pivot, &selected_layers, responses, &document.document_legacy);

		let mut begin_operation = |operation: TransformOperation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2| {
			if operation != TransformOperation::None {
				selected.revert_operation();
				typing.clear();
			} else {
				*selected.pivot = selected.mean_average_of_pivots(render_data);
			}

			*mouse_position = ipp.mouse.position;
			*start_mouse = ipp.mouse.position;
		};

		#[remain::sorted]
		match message {
			ApplyTransformOperation => {
				self.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(ToolMessage::UpdateHints);
				responses.add(BroadcastEvent::DocumentIsDirty);
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

				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				self.original_transforms.clear();
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

				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				self.original_transforms.clear();
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
				self.transform_operation.apply_transform_operation(&mut selected, self.snap, Axis::Both);

				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				self.original_transforms.clear();
			}
			CancelTransformOperation => {
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.add(ToolMessage::UpdateHints);
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			ConstrainX => self.transform_operation.constrain_axis(Axis::X, &mut selected, self.snap),
			ConstrainY => self.transform_operation.constrain_axis(Axis::Y, &mut selected, self.snap),
			PointerMove { slow_key, snap_key } => {
				self.slow = ipp.keyboard.get(slow_key as usize);

				let new_snap = ipp.keyboard.get(snap_key as usize);
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
					let delta_pos = ipp.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							let axis_constraint = translation.constraint;
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, axis_constraint);
						}
						TransformOperation::Rotating(rotation) => {
							let selected_pivot = selected.mean_average_of_pivots(render_data);
							let angle = {
								let start_offset = self.mouse_position - selected_pivot;
								let end_offset = ipp.mouse.position - selected_pivot;

								start_offset.angle_between(end_offset)
							};

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };
							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, Axis::Both);
						}
						TransformOperation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - *selected.pivot).length();
								let current_frame_dist = (ipp.mouse.position - *selected.pivot).length();
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
				self.mouse_position = ipp.mouse.position;
			}
			SelectionChanged => {
				let layer_paths = document.selected_visible_layers().map(|layer_path| layer_path.to_vec()).collect();
				shape_editor.set_selected_layers(layer_paths);
			}
			TypeBackspace => self.transform_operation.handle_typed(self.typing.type_backspace(), &mut selected, self.snap),
			TypeDecimalPoint => self.transform_operation.handle_typed(self.typing.type_decimal_point(), &mut selected, self.snap),
			TypeDigit { digit } => self.transform_operation.handle_typed(self.typing.type_number(digit), &mut selected, self.snap),
			TypeNegate => self.transform_operation.handle_typed(self.typing.type_negate(), &mut selected, self.snap),
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
