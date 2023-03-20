use crate::consts::SLOWING_DIVISOR;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::transformation::{Axis, OriginalTransforms, Selected, TransformOperation, Typing};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeEditor;
use crate::messages::tool::utility_types::{ToolData, ToolType};

use document_legacy::layers::style::RenderData;

use glam::DVec2;

#[derive(Debug, Clone, Default)]
pub struct TransformLayerMessageHandler {
	transform_operation: TransformOperation,

	slow: bool,
	snap: bool,
	typing: Typing,

	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	pivot: DVec2,

	shape_editor: ShapeEditor,
}

type TransformData<'a> = (&'a DocumentMessageHandler, &'a InputPreprocessorMessageHandler, &'a RenderData<'a>, &'a ToolData);
impl<'a> MessageHandler<TransformLayerMessage, TransformData<'a>> for TransformLayerMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: TransformLayerMessage, responses: &mut VecDeque<Message>, (document, ipp, render_data, tool_data): TransformData) {
		use TransformLayerMessage::*;

		// TODO: Transform individual points when using the path tool.
		let _using_path_tool = tool_data.active_tool_type == ToolType::Path;

		// You may also want the shape editor here? If not, then feel free to remove.
		let _shape_editor = &self.shape_editor;
		let mut transform = HashMap::new();
		let selected_layers = document.layer_metadata.iter().filter_map(|(layer_path, data)| data.selected.then_some(layer_path)).collect::<Vec<_>>();
		let mut selected = Selected::new(&mut transform, &mut self.pivot, &selected_layers, responses, &document.document_legacy);

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

				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
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
				self.transform_operation.apply_transform_operation(&mut selected, self.snap, _using_path_tool);

				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				self.original_transforms.clear();
			}
			CancelTransformOperation => {
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.transform_operation = TransformOperation::None;

				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
			}
			ConstrainX => self.transform_operation.constrain_axis(Axis::X, &mut selected, self.snap, _using_path_tool),
			ConstrainY => self.transform_operation.constrain_axis(Axis::Y, &mut selected, self.snap, _using_path_tool),
			PointerMove { slow_key, snap_key } => {
				self.slow = ipp.keyboard.get(slow_key as usize);

				let new_snap = ipp.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					self.transform_operation.apply_transform_operation(&mut selected, self.snap, _using_path_tool);
				}

				if self.typing.digits.is_empty() {
					let delta_pos = ipp.mouse.position - self.mouse_position;

					match self.transform_operation {
						TransformOperation::None => unreachable!(),
						TransformOperation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.transform_operation = TransformOperation::Grabbing(translation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, _using_path_tool);
						}
						TransformOperation::Rotating(rotation) => {
							// if ToolType is Path then calculate pivot differently
							let mut selected_pivot = selected.mean_average_of_pivots(render_data);

							if _using_path_tool {
								let path = _shape_editor.selected_layers_ref();
								let viewspace = &mut document.document_legacy.generate_transform_relative_to_viewport(path[0]).ok().unwrap();
								let points = _shape_editor.selected_points(&document.document_legacy);
								let mut count: usize = 0;
								selected_pivot = points
									.map(|point| {
										count += 1;
										viewspace.transform_point2(point.position)
									})
									.sum::<DVec2>() / count as f64;
								debug!("pivot {}", selected_pivot);
								*selected.pivot = selected_pivot;
							}
							debug!("pivot {}", selected_pivot);
							let start_offset = selected_pivot - self.mouse_position;
							let end_offset = selected_pivot - ipp.mouse.position;
							let angle = start_offset.angle_between(end_offset);
							// let start_offset = selected_pivot - self.mouse_position;
							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };
							self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, _using_path_tool);
						}
						TransformOperation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - *selected.pivot).length();
								let current_frame_dist = (ipp.mouse.position - *selected.pivot).length();
								let start_transform_dist = (self.start_mouse - *selected.pivot).length();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};

							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							debug!("Change {}", change);
							self.transform_operation = TransformOperation::Scaling(scale.increment_amount(change));
							self.transform_operation.apply_transform_operation(&mut selected, self.snap, _using_path_tool);
						}
					};
				}
				self.mouse_position = ipp.mouse.position;
			}
			SelectionChanged => {
				let layer_paths = document.selected_visible_layers().map(|layer_path| layer_path.to_vec()).collect();
				self.shape_editor.set_selected_layers(layer_paths);
			}
			TypeBackspace => self.transform_operation.handle_typed(self.typing.type_backspace(), &mut selected, self.snap, _using_path_tool),
			TypeDecimalPoint => self.transform_operation.handle_typed(self.typing.type_decimal_point(), &mut selected, self.snap, _using_path_tool),
			TypeDigit { digit } => self.transform_operation.handle_typed(self.typing.type_number(digit), &mut selected, self.snap, _using_path_tool),
			TypeNegate => self.transform_operation.handle_typed(self.typing.type_negate(), &mut selected, self.snap, _using_path_tool),
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
