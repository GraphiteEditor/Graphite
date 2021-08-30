pub use super::layer_panel::*;

use super::LayerData;

use crate::consts::{ROTATE_SNAP_ANGLE, SCALE_SNAP_INTERVAL, SLOWING_DIVISOR};
use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use glam::{DAffine2, DVec2};
use graphene::document::Document;
use graphene::Operation as DocumentOperation;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

type OriginalTransforms = HashMap<Vec<LayerId>, DAffine2>;

struct Selected<'a> {
	pub selected: Vec<Vec<LayerId>>,
	responses: &'a mut VecDeque<Message>,
	document: &'a mut Document,
	original_transforms: &'a mut OriginalTransforms,
	pivot: &'a mut DVec2,
}
impl<'a> Selected<'a> {
	pub fn new(
		original_transforms: &'a mut OriginalTransforms,
		pivot: &'a mut DVec2,
		layer_data: &'a mut HashMap<Vec<LayerId>, LayerData>,
		responses: &'a mut VecDeque<Message>,
		document: &'a mut Document,
	) -> Self {
		let selected = layer_data.iter().filter_map(|(layer_path, data)| data.selected.then(|| layer_path.to_owned())).collect();
		for path in &selected {
			if !original_transforms.contains_key::<Vec<LayerId>>(path) {
				original_transforms.insert(path.clone(), document.layer(path).unwrap().transform);
			}
		}
		Self {
			selected,
			responses,
			document,
			original_transforms,
			pivot,
		}
	}

	pub fn calculate_pivot(&mut self) -> DVec2 {
		let xy_summation = self
			.selected
			.iter()
			.map(|path| {
				let multiplied_transform = self.document.multiply_transforms(path).unwrap();

				let bounds = self
					.document
					.layer(path)
					.unwrap()
					.current_bounding_box_with_transform(multiplied_transform)
					.unwrap_or([multiplied_transform.translation; 2]);

				(bounds[0] + bounds[1]) / 2.
			})
			.fold(DVec2::ZERO, |summation, next| summation + next);

		xy_summation / self.selected.len() as f64
	}

	pub fn update_transforms(&mut self, delta: DAffine2) {
		if !self.selected.is_empty() {
			let pivot = DAffine2::from_translation(*self.pivot);
			let transformation = pivot * delta * pivot.inverse();

			for layer_path in &self.selected {
				let parent_folder_path = &layer_path[..layer_path.len() - 1];
				let original_layer_transforms = *self.original_transforms.get(layer_path).unwrap();

				let to = self.document.generate_transform_across_scope(parent_folder_path, None).unwrap();
				let new = to.inverse() * transformation * to * original_layer_transforms;

				self.responses.push_back(
					DocumentOperation::SetLayerTransform {
						path: layer_path.to_vec(),
						transform: new.to_cols_array(),
					}
					.into(),
				);
			}

			self.responses.push_back(ToolMessage::SelectedLayersChanged.into());
		}
	}

	pub fn revert_operation(&mut self) {
		for path in &self.selected {
			self.responses.push_back(
				DocumentOperation::SetLayerTransform {
					path: path.to_vec(),
					transform: (*self.original_transforms.get(path).unwrap()).to_cols_array(),
				}
				.into(),
			);
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum Axis {
	Both,
	X,
	Y,
}

impl Default for Axis {
	fn default() -> Self {
		Self::Both
	}
}

impl Axis {
	pub fn set_or_toggle(&mut self, target: Axis) {
		// If constrained to an axis and target is requesting the same axis, toggle back to Both
		if *self == target {
			*self = Axis::Both;
		}
		// If current axis is different from the target axis, switch to the target
		else {
			*self = target;
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
struct Translation {
	pub dragged_distance: DVec2,
	pub typed_distance: Option<f64>,
	pub constraint: Axis,
}

impl Translation {
	pub fn to_dvec(self) -> DVec2 {
		if let Some(value) = self.typed_distance {
			if self.constraint == Axis::Y {
				return DVec2::new(0., value);
			} else {
				return DVec2::new(value, 0.);
			}
		}

		match self.constraint {
			Axis::Both => self.dragged_distance,
			Axis::X => DVec2::new(self.dragged_distance.x, 0.),
			Axis::Y => DVec2::new(0., self.dragged_distance.y),
		}
	}

	pub fn increment_amount(self, delta: DVec2) -> Self {
		Self {
			dragged_distance: self.dragged_distance + delta,
			typed_distance: None,
			constraint: self.constraint,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
struct Scale {
	pub dragged_factor: f64,
	pub typed_factor: Option<f64>,
	pub constraint: Axis,
}

impl Default for Scale {
	fn default() -> Self {
		Self {
			dragged_factor: 1.,
			typed_factor: None,
			constraint: Axis::default(),
		}
	}
}

impl Scale {
	pub fn to_dvec(self, snap: bool) -> DVec2 {
		let factor = if let Some(value) = self.typed_factor { value } else { self.dragged_factor };
		let factor = if snap { (factor / SCALE_SNAP_INTERVAL).round() * SCALE_SNAP_INTERVAL } else { factor };

		match self.constraint {
			Axis::Both => DVec2::splat(factor),
			Axis::X => DVec2::new(factor, 1.),
			Axis::Y => DVec2::new(1., factor),
		}
	}

	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			dragged_factor: self.dragged_factor + delta,
			typed_factor: None,
			constraint: self.constraint,
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
struct Rotation {
	pub dragged_angle: f64,
	pub typed_angle: Option<f64>,
}

impl Rotation {
	pub fn to_f64(self, snap: bool) -> f64 {
		if let Some(value) = self.typed_angle {
			value.to_radians()
		} else if snap {
			let snap_resolution = ROTATE_SNAP_ANGLE.to_radians();
			(self.dragged_angle / snap_resolution).round() * snap_resolution
		} else {
			self.dragged_angle
		}
	}

	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			dragged_angle: self.dragged_angle + delta,
			typed_angle: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum Operation {
	None,
	Grabbing(Translation),
	Rotating(Rotation),
	Scaling(Scale),
}

impl Default for Operation {
	fn default() -> Self {
		Operation::None
	}
}

impl Operation {
	pub fn apply_operation(&self, selected: &mut Selected, snapping: bool) {
		if self != &Operation::None {
			let transformation = match self {
				Operation::Grabbing(translation) => DAffine2::from_translation(translation.to_dvec()),
				Operation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64(snapping)),
				Operation::Scaling(scale) => DAffine2::from_scale(scale.to_dvec(snapping)),
				Operation::None => unreachable!(),
			};

			selected.update_transforms(transformation);
		}
	}

	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected, snapping: bool) {
		match self {
			Operation::None => (),
			Operation::Grabbing(translation) => translation.constraint.set_or_toggle(axis),
			Operation::Rotating(_) => (),
			Operation::Scaling(scale) => scale.constraint.set_or_toggle(axis),
		};

		self.apply_operation(selected, snapping);
	}

	pub fn handle_typed(&mut self, typed: Option<f64>, selected: &mut Selected, snapping: bool) {
		match self {
			Operation::None => (),
			Operation::Grabbing(translation) => translation.typed_distance = typed,
			Operation::Rotating(rotation) => rotation.typed_angle = typed,
			Operation::Scaling(scale) => scale.typed_factor = typed,
		};

		self.apply_operation(selected, snapping);
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Typing {
	digits: Vec<u8>,
	contains_decimal: bool,
	negative: bool,
}

const DECIMAL_POINT: u8 = 10;

impl Typing {
	pub fn type_number(&mut self, number: u8) -> Option<f64> {
		self.digits.push(number);

		self.evaluate()
	}

	pub fn type_backspace(&mut self) -> Option<f64> {
		if self.digits.is_empty() {
			return None;
		}

		match self.digits.pop() {
			Some(DECIMAL_POINT) => self.contains_decimal = false,
			Some(_) => (),
			None => self.negative = false,
		}

		self.evaluate()
	}

	pub fn type_decimal_point(&mut self) -> Option<f64> {
		if !self.contains_decimal {
			self.contains_decimal = true;
			self.digits.push(DECIMAL_POINT);
		}

		self.evaluate()
	}

	pub fn type_negate(&mut self) -> Option<f64> {
		self.negative = !self.negative;

		self.evaluate()
	}

	pub fn evaluate(&self) -> Option<f64> {
		if self.digits.is_empty() {
			return None;
		}

		let mut result = 0_f64;
		let mut running_decimal_place = 0_i32;

		for digit in &self.digits {
			if *digit == DECIMAL_POINT {
				if running_decimal_place == 0 {
					running_decimal_place = 1;
				}
			} else if running_decimal_place == 0 {
				result *= 10.;
				result += *digit as f64;
			} else {
				result += *digit as f64 * 0.1_f64.powi(running_decimal_place);
				running_decimal_place += 1;
			}
		}

		if self.negative {
			result = -result;
		}

		Some(result)
	}

	pub fn clear(&mut self) {
		self.digits.clear();
		self.contains_decimal = false;
		self.negative = false;
	}
}

#[impl_message(Message, DocumentMessage, TransformLayers)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TransformLayerMessage {
	BeginGrab,
	BeginScale,
	BeginRotate,

	CancelOperation,
	ApplyOperation,

	TypeNumber(u8),
	TypeBackspace,
	TypeDecimalPoint,
	TypeNegate,

	ConstrainX,
	ConstrainY,

	MouseMove { slow_key: Key, snap_key: Key },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformLayerMessageHandler {
	operation: Operation,

	slow: bool,
	snap: bool,
	typing: Typing,

	mouse_position: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	pivot: DVec2,
}

impl MessageHandler<TransformLayerMessage, (&mut HashMap<Vec<LayerId>, LayerData>, &mut Document, &InputPreprocessor)> for TransformLayerMessageHandler {
	fn process_action(&mut self, message: TransformLayerMessage, data: (&mut HashMap<Vec<LayerId>, LayerData>, &mut Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		use TransformLayerMessage::*;

		let (layer_data, document, ipp) = data;
		let mut selected = Selected::new(&mut self.original_transforms, &mut self.pivot, layer_data, responses, document);

		let mut begin_operation = |operation: Operation, typing: &mut Typing, mouse_position: &mut DVec2, start_mouse: &mut DVec2| {
			if !(operation == Operation::None) {
				selected.revert_operation();
				typing.clear();
			} else {
				*selected.pivot = selected.calculate_pivot();
			}

			*mouse_position = ipp.mouse.position;
			*start_mouse = ipp.mouse.position;
		};

		match message {
			BeginGrab => {
				if let Operation::Grabbing(_) = self.operation {
					return;
				}

				begin_operation(self.operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.operation = Operation::Grabbing(Default::default());

				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			BeginRotate => {
				if let Operation::Rotating(_) = self.operation {
					return;
				}

				begin_operation(self.operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.operation = Operation::Rotating(Default::default());

				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			BeginScale => {
				if let Operation::Scaling(_) = self.operation {
					return;
				}

				begin_operation(self.operation, &mut self.typing, &mut self.mouse_position, &mut self.start_mouse);

				self.operation = Operation::Scaling(Default::default());
				self.operation.apply_operation(&mut selected, self.snap);

				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			CancelOperation => {
				selected.revert_operation();

				selected.original_transforms.clear();
				self.typing.clear();

				self.operation = Operation::None;

				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			ApplyOperation => {
				self.original_transforms.clear();
				self.typing.clear();

				self.operation = Operation::None;

				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			MouseMove { slow_key, snap_key } => {
				self.slow = ipp.keyboard.get(slow_key as usize);

				let new_snap = ipp.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					self.operation.apply_operation(&mut selected, self.snap);
				}

				if self.typing.digits.is_empty() {
					let delta_pos = ipp.mouse.position - self.mouse_position;

					match self.operation {
						Operation::None => unreachable!(),
						Operation::Grabbing(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.operation = Operation::Grabbing(translation.increment_amount(change));
							self.operation.apply_operation(&mut selected, self.snap);
						}
						Operation::Rotating(rotation) => {
							let selected_pivot = selected.calculate_pivot();
							let angle = {
								let start_vec = self.mouse_position - selected_pivot;
								let end_vec = ipp.mouse.position - selected_pivot;

								start_vec.angle_between(end_vec)
							};

							let change = if self.slow { angle / SLOWING_DIVISOR } else { angle };
							self.operation = Operation::Rotating(rotation.increment_amount(change));
							self.operation.apply_operation(&mut selected, self.snap);
						}
						Operation::Scaling(scale) => {
							let change = {
								let previous_frame_dist = (self.mouse_position - *selected.pivot).length();
								let current_frame_dist = (ipp.mouse.position - *selected.pivot).length();
								let start_transform_dist = (self.start_mouse - *selected.pivot).length();

								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};

							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							self.operation = Operation::Scaling(scale.increment_amount(change));
							self.operation.apply_operation(&mut selected, self.snap);
						}
					};
				}
				self.mouse_position = ipp.mouse.position;
			}
			TypeNumber(number) => self.operation.handle_typed(self.typing.type_number(number), &mut selected, self.snap),
			TypeBackspace => self.operation.handle_typed(self.typing.type_backspace(), &mut selected, self.snap),
			TypeDecimalPoint => self.operation.handle_typed(self.typing.type_decimal_point(), &mut selected, self.snap),
			TypeNegate => self.operation.handle_typed(self.typing.type_negate(), &mut selected, self.snap),
			ConstrainX => self.operation.constrain_axis(Axis::X, &mut selected, self.snap),
			ConstrainY => self.operation.constrain_axis(Axis::Y, &mut selected, self.snap),
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(TransformLayerMessageDiscriminant;
			BeginGrab,
			BeginScale,
			BeginRotate,
		);

		if self.operation != Operation::None {
			let active = actions!(TransformLayerMessageDiscriminant;
				MouseMove,
				CancelOperation,
				ApplyOperation,
				TypeNumber,
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
