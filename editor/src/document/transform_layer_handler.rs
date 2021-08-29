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
	mid: &'a mut DVec2,
}
impl<'a> Selected<'a> {
	pub fn new(
		original_transforms: &'a mut OriginalTransforms,
		mid: &'a mut DVec2,
		layerdata: &'a mut HashMap<Vec<LayerId>, LayerData>,
		responses: &'a mut VecDeque<Message>,
		document: &'a mut Document,
	) -> Self {
		let selected = layerdata.iter().filter_map(|(path, data)| data.selected.then(|| path.to_owned())).collect();
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
			mid,
		}
	}
	pub fn calculate_mid(&mut self) -> DVec2 {
		self.selected
			.iter()
			.map(|path| {
				let multiplied_transform = self.document.multiply_transforms(path).unwrap();
				let bounds = self
					.document
					.layer(path)
					.unwrap()
					.current_bounding_box_with_transform(multiplied_transform)
					.unwrap_or([multiplied_transform.translation; 2]);
				let mid = (bounds[0] + bounds[1]) / 2.;
				mid
			})
			.fold(DVec2::ZERO, |acc, x| acc + x)
			/ self.selected.len() as f64
	}
	pub fn repopulate_transforms(&mut self) {
		self.original_transforms.clear();
		for path in &self.selected {
			self.original_transforms.insert(path.clone(), self.document.layer(path).unwrap().transform);
		}
		*self.mid = self.calculate_mid();
	}
	pub fn update_transforms(&mut self, delta: DAffine2) {
		if self.selected.len() > 0 {
			let mid = DAffine2::from_translation(*self.mid);
			let transformation = mid * delta * mid.inverse();
			for path in &self.selected {
				let to = self.document.generate_transform_across_scope(&path[..path.len() - 1], None).unwrap();
				let new = to.inverse() * transformation * to * *self.original_transforms.get(path).unwrap();
				self.responses.push_back(
					DocumentOperation::SetLayerTransform {
						path: path.to_vec(),
						transform: new.to_cols_array(),
					}
					.into(),
				);
			}

			self.responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
		}
	}
	pub fn reset(&mut self) {
		for path in &self.selected {
			self.responses.push_back(
				DocumentOperation::SetLayerTransform {
					path: path.to_vec(),
					transform: (*self.original_transforms.get(path).unwrap()).to_cols_array(),
				}
				.into(),
			);
		}
		self.original_transforms.clear();
		self.selected.clear();
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
	pub fn invert(&mut self, target: Axis) {
		if *self == target {
			*self = Axis::Both;
		} else {
			*self = target;
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
struct Translation {
	pub amount: DVec2,
	pub constraint: Axis,
	pub typed: Option<f64>,
}

impl Translation {
	pub fn to_dvec(&self) -> DVec2 {
		if let Some(x) = self.typed {
			if self.constraint == Axis::Y {
				return DVec2::new(0., x);
			} else {
				return DVec2::new(x, 0.);
			}
		}
		match self.constraint {
			Axis::Both => self.amount,
			Axis::X => DVec2::new(self.amount.x, 0.),
			Axis::Y => DVec2::new(0., self.amount.y),
		}
	}
	pub fn increment_amount(self, delta: DVec2) -> Self {
		Self {
			amount: self.amount + delta,
			constraint: self.constraint,
			typed: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
struct Scale {
	pub amount: f64,
	pub constraint: Axis,
	pub typed: Option<f64>,
}

impl Default for Scale {
	fn default() -> Self {
		Self {
			amount: 1.,
			constraint: Axis::default(),
			typed: None,
		}
	}
}

impl Scale {
	pub fn to_dvec(&self, snap: bool) -> DVec2 {
		let value = if let Some(x) = self.typed { x } else { self.amount };
		let value = if snap { (value / SCALE_SNAP_INTERVAL).round() * SCALE_SNAP_INTERVAL } else { value };
		match self.constraint {
			Axis::Both => DVec2::splat(value),
			Axis::X => DVec2::new(value, 1.),
			Axis::Y => DVec2::new(1., value),
		}
	}
	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			amount: self.amount + delta,
			constraint: self.constraint,
			typed: None,
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
struct Rotation {
	pub amount: f64,
	pub typed: Option<f64>,
}

impl Rotation {
	pub fn to_f64(&self, snap: bool) -> f64 {
		if let Some(x) = self.typed {
			x.to_radians()
		} else if snap {
			let snap_resolution = ROTATE_SNAP_ANGLE.to_radians();
			(self.amount / snap_resolution).round() * snap_resolution
		} else {
			self.amount
		}
	}
	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			amount: self.amount + delta,
			typed: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum Operation {
	None,
	Translating(Translation),
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
				Operation::Translating(translation) => DAffine2::from_translation(translation.to_dvec()),
				Operation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64(snapping)),
				Operation::Scaling(scale) => DAffine2::from_scale(scale.to_dvec(snapping)),
				Operation::None => unreachable!(),
			};

			selected.update_transforms(transformation);
		}
	}

	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected, snapping: bool) {
		match self {
			Operation::None => {}
			Operation::Translating(t) => t.constraint.invert(axis),
			Operation::Rotating(_) => {}
			Operation::Scaling(s) => s.constraint.invert(axis),
		}
		self.apply_operation(selected, snapping);
	}

	pub fn handle_typed(&mut self, typed: Option<f64>, selected: &mut Selected, snapping: bool) {
		match self {
			Operation::None => {}
			Operation::Translating(t) => t.typed = typed,
			Operation::Rotating(r) => r.typed = typed,
			Operation::Scaling(s) => s.typed = typed,
		}
		self.apply_operation(selected, snapping);
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Typing {
	is_typing: bool,
	digits: Vec<u8>,
	contains_decimal: bool,
	negative: bool,
}
impl Typing {
	pub fn type_num(&mut self, num: u8) -> Option<f64> {
		self.is_typing = true;
		self.digits.push(num);
		self.evaluate()
	}
	pub fn type_delete(&mut self) -> Option<f64> {
		if self.is_typing {
			if self.digits.pop() == Some(200) {
				self.contains_decimal = false;
			}
			if self.digits.len() == 0 {
				self.is_typing = false;
				self.negative = false;
			}
		}
		self.evaluate()
	}
	pub fn type_decimal(&mut self) -> Option<f64> {
		if !self.contains_decimal {
			self.digits.push(200);
			self.contains_decimal = true;
		}
		self.evaluate()
	}
	pub fn type_negative(&mut self) -> Option<f64> {
		self.negative = !self.negative;
		self.evaluate()
	}
	pub fn evaluate(&self) -> Option<f64> {
		if self.digits.len() == 0 {
			return None;
		}
		let mut result = 0_f64;
		let mut decimal = 0_i32;
		for v in &self.digits {
			if v == &200 {
				if decimal == 0 {
					decimal = 1;
				}
			} else {
				if decimal == 0 {
					result *= 10.;
					result += *v as f64;
				} else {
					result += *v as f64 * 0.1_f64.powi(decimal);
					decimal += 1;
				}
			}
		}
		if self.negative {
			result = -result;
		}
		Some(result)
	}
	pub fn reset(&mut self) {
		self.digits.clear();
		self.is_typing = false;
		self.negative = false;
	}
}

#[impl_message(Message, DocumentMessage, TransformLayers)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TransformLayerMessage {
	BeginTranslate,
	BeginScale,
	BeginRotate,

	CancelOperation,
	ApplyOperation,

	TypeNum(u8),
	TypeDelete,
	TypeDecimalPoint,
	TypeNegative,

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

	mouse_pos: ViewportPosition,
	start_mouse: ViewportPosition,

	original_transforms: OriginalTransforms,
	mid: DVec2,
}

impl MessageHandler<TransformLayerMessage, (&mut HashMap<Vec<LayerId>, LayerData>, &mut Document, &InputPreprocessor)> for TransformLayerMessageHandler {
	fn process_action(&mut self, message: TransformLayerMessage, data: (&mut HashMap<Vec<LayerId>, LayerData>, &mut Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, document, ipp) = data;
		let mut selected = Selected::new(&mut self.original_transforms, &mut self.mid, layerdata, responses, document);
		use TransformLayerMessage::*;
		match message {
			BeginTranslate => {
				self.mouse_pos = ipp.mouse.position;
				self.start_mouse = ipp.mouse.position;
				selected.repopulate_transforms();
				self.operation = Operation::Translating(Default::default());
				responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
			}
			BeginRotate => {
				self.mouse_pos = ipp.mouse.position;
				self.start_mouse = ipp.mouse.position;
				selected.repopulate_transforms();
				self.operation = Operation::Rotating(Default::default());
				responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
			}
			BeginScale => {
				self.mouse_pos = ipp.mouse.position;
				self.start_mouse = ipp.mouse.position;
				selected.repopulate_transforms();
				self.operation = Operation::Scaling(Default::default());
				responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
			}
			CancelOperation => {
				selected.reset();
				self.operation = Operation::None;
				self.typing.reset();
				responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
			}
			ApplyOperation => {
				selected.selected.clear();
				self.original_transforms.clear();
				self.typing.reset();
				self.operation = Operation::None;
				responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
			}
			MouseMove { slow_key, snap_key } => {
				self.slow = ipp.keyboard.get(slow_key as usize);

				let new_snap = ipp.keyboard.get(snap_key as usize);
				if new_snap != self.snap {
					self.snap = new_snap;
					self.operation.apply_operation(&mut selected, self.snap);
				}

				if !self.typing.is_typing {
					let delta_pos = ipp.mouse.position - self.mouse_pos;
					match self.operation {
						Operation::None => unreachable!(),
						Operation::Translating(translation) => {
							let change = if self.slow { delta_pos / SLOWING_DIVISOR } else { delta_pos };
							self.operation = Operation::Translating(translation.increment_amount(change));
							self.operation.apply_operation(&mut selected, self.snap);
						}
						Operation::Rotating(r) => {
							let selected_mid = selected.calculate_mid();
							let rotation = {
								let start_vec = self.mouse_pos - selected_mid;
								let end_vec = ipp.mouse.position - selected_mid;
								start_vec.angle_between(end_vec)
							};
							let change = if self.slow { rotation / SLOWING_DIVISOR } else { rotation };
							self.operation = Operation::Rotating(r.increment_amount(change));
							self.operation.apply_operation(&mut selected, self.snap);
						}
						Operation::Scaling(s) => {
							let change = {
								let previous_frame_dist = (self.mouse_pos - *selected.mid).length();
								let current_frame_dist = (ipp.mouse.position - *selected.mid).length();
								let start_transform_dist = (self.start_mouse - *selected.mid).length();
								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};
							let change = if self.slow { change / SLOWING_DIVISOR } else { change };
							self.operation = Operation::Scaling(s.increment_amount(change));
							self.operation.apply_operation(&mut selected, self.snap);
						}
					}
				}
				self.mouse_pos = ipp.mouse.position;
			}
			TypeNum(number) => self.operation.handle_typed(self.typing.type_num(number), &mut selected, self.snap),
			TypeDelete => self.operation.handle_typed(self.typing.type_delete(), &mut selected, self.snap),
			TypeDecimalPoint => self.operation.handle_typed(self.typing.type_decimal(), &mut selected, self.snap),
			TypeNegative => self.operation.handle_typed(self.typing.type_negative(), &mut selected, self.snap),
			ConstrainX => self.operation.constrain_axis(Axis::X, &mut selected, self.snap),
			ConstrainY => self.operation.constrain_axis(Axis::Y, &mut selected, self.snap),
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(TransformLayerMessageDiscriminant;
			BeginTranslate,
			BeginScale,
			BeginRotate,
		);

		if self.operation != Operation::None {
			let active = actions!(TransformLayerMessageDiscriminant;
				MouseMove,
				CancelOperation,
				ApplyOperation,
				TypeNum,
				TypeDelete,
				TypeDecimalPoint,
				TypeNegative,
				ConstrainX,
				ConstrainY,
			);
			common.extend(active);
		}
		common
	}
}
