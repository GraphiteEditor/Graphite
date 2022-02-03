use super::layer_panel::LayerMetadata;
use super::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::consts::SLOWING_DIVISOR;
use crate::input::mouse::ViewportPosition;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use graphene::document::Document;

use glam::DVec2;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformLayerMessageHandler {
	transform_operation: TransformOperation,

	slow: bool,
	snap: bool,
	typing: Typing,

	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	pivot: DVec2,
}

impl MessageHandler<TransformLayerMessage, (&mut HashMap<Vec<LayerId>, LayerMetadata>, &mut Document, &InputPreprocessorMessageHandler)> for TransformLayerMessageHandler {
	#[remain::check]
	fn process_action(
		&mut self,
		message: TransformLayerMessage,
		data: (&mut HashMap<Vec<LayerId>, LayerMetadata>, &mut Document, &InputPreprocessorMessageHandler),
		responses: &mut VecDeque<Message>,
	) {
		use TransformLayerMessage::*;

		let (layer_metadata, document, ipp) = data;

		let selected_layers = layer_metadata.iter().filter_map(|(layer_path, data)| data.selected.then(|| layer_path)).collect::<Vec<_>>();
		let mut selected = Selected::new(&mut self.original_transforms, &mut self.pivot, &selected_layers, responses, document);

		let mut begin_operation = |operation: TransformOperation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2| {
			if operation != TransformOperation::None {
				selected.revert_operation();
				typing.clear();
			} else {
				*selected.pivot = selected.calculate_pivot();
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

				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			BeginGrab => {
				if let TransformOperation::Grabbing(_) = self.transform_operation {
					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Grabbing(Default::default());

				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			BeginRotate => {
				if let TransformOperation::Rotating(_) = self.transform_operation {
					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Rotating(Default::default());

				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			BeginScale => {
				if let TransformOperation::Scaling(_) = self.transform_operation {
					return;
				}

				begin_operation(self.transform_operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.transform_operation = TransformOperation::Scaling(Default::default());
				self.transform_operation.apply_transform_operation(&mut selected, self.snap);

				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			CancelTransformOperation => {
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			ConstrainX => self.transform_operation.constrain_axis(Axis::X, &mut selected, self.snap),
			ConstrainY => self.transform_operation.constrain_axis(Axis::Y, &mut selected, self.snap),
			MouseMove { slow_key, snap_key } => {
				self.slow = ipp.keyboard.get(slow_key as usize);

				let new_snap = ipp.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					self.transform_operation.apply_transform_operation(&mut selected, self.snap);
				}

				if self.typing.digits.is_empty() {
					let delta_pos = ipp.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap);
						}
						TransformOperation::Rotating(rotation) => {
							let selected_pivot = selected.calculate_pivot();
							let angle = {
								let start_offset = self.mouse_position - selected_pivot;
								let end_offset = ipp.mouse.position - selected_pivot;

								start_offset.angle_between(end_offset)
							};

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };
							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap);
						}
						TransformOperation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - *selected.pivot).length();
								let current_frame_dist = (ipp.mouse.position - *selected.pivot).length();
								let start_transform_dist = (self.start_mouse - *selected.pivot).length();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};

							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							self.transform_operation = TransformOperation::Scaling(scale.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap);
						}
					};
				}
				self.mouse_position = ipp.mouse.position;
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
				MouseMove,
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
