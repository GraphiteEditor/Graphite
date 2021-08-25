pub use super::layer_panel::*;

use super::LayerData;

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use glam::{DAffine2, DVec2};
use graphene::Operation as DocumentOperation;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

struct Selected<'a> {
	pub selected: Vec<Vec<LayerId>>,
	responses: &'a mut VecDeque<Message>,
}
impl<'a> Selected<'a> {
	pub fn new(layerdata: &'a mut HashMap<Vec<LayerId>, LayerData>, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			selected: layerdata.iter().filter_map(|(path, data)| data.selected.then(|| path.to_owned())).collect(),
			responses,
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
}

impl Translation {
	pub fn to_dvec(&self) -> DVec2 {
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
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
struct Scale {
	pub amount: f64,
	pub constraint: Axis,
}

impl Default for Scale {
	fn default() -> Self {
		Self {
			amount: 1.,
			constraint: Axis::default(),
		}
	}
}

impl Scale {
	pub fn to_dvec(&self) -> DVec2 {
		match self.constraint {
			Axis::Both => DVec2::splat(self.amount),
			Axis::X => DVec2::new(self.amount, 0.),
			Axis::Y => DVec2::new(0., self.amount),
		}
	}
	pub fn _increment_amount(&mut self, delta: f64) -> &mut Self {
		self.amount += delta;
		self
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum Operation {
	None,
	Translating(Translation),
	Rotating(f64),
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
			let mut daffine = match self {
				Operation::Translating(translation) => DAffine2::from_translation(translation.to_dvec()),
				Operation::Rotating(radians) => DAffine2::from_angle(*radians * if invert { -1. } else { 1. }),
				Operation::Scaling(scale) => DAffine2::from_scale(scale.to_dvec()),
				Operation::None => unreachable!(),
			};
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

	ConstrainX,
	ConstrainY,

	MouseMove,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformLayerMessageHandler {
	operation: Operation,

	shift_down: bool,
	ctrl_down: bool,
	typing: bool,

	mouse_pos: ViewportPosition,
	previous_val: DVec2,
	change: DVec2,
}

impl MessageHandler<TransformLayerMessage, (&mut HashMap<Vec<LayerId>, LayerData>, &InputPreprocessor)> for TransformLayerMessageHandler {
	fn process_action(&mut self, message: TransformLayerMessage, data: (&mut HashMap<Vec<LayerId>, LayerData>, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, ipp) = data;
		let mut selected = Selected::new(layerdata, responses);
		use TransformLayerMessage::*;
		match message {
			BeginTranslate => {
				self.mouse_pos = ipp.mouse.position;
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::Translating(Default::default());
			}
			BeginRotate => {
				self.mouse_pos = ipp.mouse.position;
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::Rotating(Default::default());
			}
			BeginScale => {
				self.mouse_pos = ipp.mouse.position;
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::Scaling(Default::default());
			}
			CancelOperation => {
				self.operation.apply_operation(&mut selected, true);
				self.operation = Operation::None;
			}
			ApplyOperation => self.operation = Operation::None,
			MouseMove => {
				let delta_pos = ipp.mouse.position - self.mouse_pos;
				match self.operation {
					Operation::None => unreachable!(),
					Operation::Translating(translation) => {
						self.operation.apply_operation(&mut selected, true);
						self.operation = Operation::Translating(translation.increment_amount(delta_pos));
						self.operation.apply_operation(&mut selected, false);
					}
					Operation::Rotating(_) => todo!(),
					Operation::Scaling(_) => todo!(),
				}
				self.mouse_pos = ipp.mouse.position;
			}
			TypeNum(k) => log::info!("Num Typed {}", k),
			TypeDelete => log::info!("Delete "),
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
				ConstrainX,
				ConstrainY,
			);
			common.extend(active);
		}
		common
	}
}
