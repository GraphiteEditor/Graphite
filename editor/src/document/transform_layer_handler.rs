pub use super::layer_panel::*;

use super::LayerData;

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use glam::{DAffine2, DVec2};
use graphene::document::Document;
use graphene::Operation as DocumentOperation;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;

struct Selected<'a> {
	pub selected: Vec<Vec<LayerId>>,
	responses: &'a mut VecDeque<Message>,
	document: &'a mut Document,
}
impl<'a> Selected<'a> {
	pub fn new(layerdata: &'a mut HashMap<Vec<LayerId>, LayerData>, responses: &'a mut VecDeque<Message>, document: &'a mut Document) -> Self {
		Self {
			selected: layerdata.iter().filter_map(|(path, data)| data.selected.then(|| path.to_owned())).collect(),
			responses,
			document,
		}
	}
	pub fn calculate_mid(&self) -> DVec2 {
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
	pub fn to_dvec(&self) -> DVec2 {
		let value = if let Some(x) = self.typed { x } else { self.amount };
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
	pub fn to_f64(&self) -> f64 {
		if let Some(x) = self.typed {
			x.to_radians()
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
	pub fn apply_operation(&self, selected: &mut Selected, invert: bool) {
		if self != &Operation::None {
			let mid = selected.calculate_mid();
			let mut daffine = match self {
				Operation::Translating(translation) => DAffine2::from_translation(translation.to_dvec()),
				Operation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64()),
				Operation::Scaling(scale) => DAffine2::from_scale(scale.to_dvec()),
				Operation::None => unreachable!(),
			};
			daffine = DAffine2::from_translation(mid) * daffine * DAffine2::from_translation(-mid);
			if invert {
				daffine = daffine.inverse();
			}
			for path in &selected.selected {
				selected.responses.push_back(
					DocumentOperation::TransformLayerInViewport {
						path: path.to_vec(),
						transform: daffine.to_cols_array(),
					}
					.into(),
				);
			}
			if selected.selected.len() > 0 {
				selected.responses.push_back(SelectMessage::UpdateSelectionBoundingBox.into());
			}
		}
	}

	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected) {
		self.apply_operation(selected, true);
		match self {
			Operation::None => {}
			Operation::Translating(t) => t.constraint.invert(axis),
			Operation::Rotating(_) => {}
			Operation::Scaling(s) => s.constraint.invert(axis),
		}
		self.apply_operation(selected, false);
	}

	pub fn handle_typed(&mut self, typed: Option<f64>, selected: &mut Selected) {
		self.apply_operation(selected, true);
		match self {
			Operation::None => {}
			Operation::Translating(t) => t.typed = typed,
			Operation::Rotating(r) => r.typed = typed,
			Operation::Scaling(s) => s.typed = typed,
		}
		self.apply_operation(selected, false);
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Typing {
	is_typing: bool,
	digits: Vec<u8>,
}
impl Typing {
	pub fn type_num(&mut self, num: u8) -> Option<f64> {
		self.is_typing = true;
		self.digits.push(num);
		self.evaluate()
	}
	pub fn type_delete(&mut self) -> Option<f64> {
		if self.is_typing {
			self.digits.pop();
			if self.digits.len() == 0 {
				self.is_typing = false;
			}
		}
		self.evaluate()
	}
	pub fn type_decimal(&mut self) -> Option<f64> {
		self.digits.push(200);
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
		Some(result)
	}
	pub fn reset(&mut self) {
		self.digits.clear();
		self.is_typing = false;
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

	ConstrainX,
	ConstrainY,

	MouseMove,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformLayerMessageHandler {
	operation: Operation,

	shift_down: bool,
	ctrl_down: bool,
	typing: Typing,

	mouse_pos: ViewportPosition,
	start_mouse: ViewportPosition,
}

impl MessageHandler<TransformLayerMessage, (&mut HashMap<Vec<LayerId>, LayerData>, &mut Document, &InputPreprocessor)> for TransformLayerMessageHandler {
	fn process_action(&mut self, message: TransformLayerMessage, data: (&mut HashMap<Vec<LayerId>, LayerData>, &mut Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, document, ipp) = data;
		let mut selected = Selected::new(layerdata, responses, document);
		use TransformLayerMessage::*;
		match message {
			BeginTranslate => {
				self.mouse_pos = ipp.mouse.position;
				self.start_mouse = ipp.mouse.position;
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::Translating(Default::default());
			}
			BeginRotate => {
				self.mouse_pos = ipp.mouse.position;
				self.start_mouse = ipp.mouse.position;
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::Rotating(Default::default());
			}
			BeginScale => {
				self.mouse_pos = ipp.mouse.position;
				self.start_mouse = ipp.mouse.position;
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::Scaling(Default::default());
			}
			CancelOperation => {
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::None;
				self.typing.reset();
			}
			ApplyOperation => {
				self.typing.reset();
				self.operation = Operation::None;
			}
			MouseMove => {
				if !self.typing.is_typing {
					let delta_pos = ipp.mouse.position - self.mouse_pos;
					match self.operation {
						Operation::None => unreachable!(),
						Operation::Translating(translation) => {
							self.operation.apply_operation(&mut selected, true);
							self.operation = Operation::Translating(translation.increment_amount(delta_pos));
							self.operation.apply_operation(&mut selected, false);
						}
						Operation::Rotating(r) => {
							self.operation.apply_operation(&mut selected, true);
							let selected_mid = selected.calculate_mid();
							let rotation = {
								let start_vec = self.mouse_pos - selected_mid;
								let end_vec = ipp.mouse.position - selected_mid;
								start_vec.angle_between(end_vec)
							};
							self.operation = Operation::Rotating(r.increment_amount(rotation));
							self.operation.apply_operation(&mut selected, false);
						}
						Operation::Scaling(s) => {
							self.operation.apply_operation(&mut selected, true);
							let selected_mid = selected.calculate_mid();
							let change = {
								let previous_frame_dist = (self.mouse_pos - selected_mid).length();
								let current_frame_dist = (ipp.mouse.position - selected_mid).length();
								let start_transform_dist = (self.start_mouse - selected_mid).length();
								(current_frame_dist - previous_frame_dist) / start_transform_dist
							};
							self.operation = Operation::Scaling(s.increment_amount(change));
							self.operation.apply_operation(&mut selected, false);
						}
					}
				}
				self.mouse_pos = ipp.mouse.position;
			}
			TypeNum(k) => self.operation.handle_typed(self.typing.type_num(k), &mut selected),
			TypeDelete => self.operation.handle_typed(self.typing.type_delete(), &mut selected),
			TypeDecimalPoint => self.operation.handle_typed(self.typing.type_decimal(), &mut selected),
			ConstrainX => self.operation.constrain_axis(Axis::X, &mut selected),
			ConstrainY => self.operation.constrain_axis(Axis::Y, &mut selected),
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
				ConstrainX,
				ConstrainY,
			);
			common.extend(active);
		}
		common
	}
}
