pub use super::layer_panel::*;

use super::LayerData;

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use glam::{DAffine2, DVec2};
use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Copy)]
enum Axis {
	Both(DVec2),
	X(f64),
	Y(f64),
}

impl Default for Axis {
	fn default() -> Self {
		Self::Both(DVec2::ZERO)
	}
}

impl Axis {
	pub fn to_dvec(&self) -> DVec2 {
		match self {
			Axis::Both(vec) => *vec,
			Axis::X(x) => DVec2::new(*x, 0.),
			Axis::Y(y) => DVec2::new(0., *y),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum OperationType {
	None,
	Translating,
	Rotating,
	Scaling,
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum Operation {
	None,
	Translating { change: Axis },
	Rotating { change: f64 },
	Scaling { change: Axis },
}

impl Default for Operation {
	fn default() -> Self {
		Self::None
	}
}

impl Operation {
	fn get_type(&self) -> OperationType {
		match self {
			Operation::None => OperationType::None,
			Operation::Translating { change: _ } => OperationType::Translating,
			Operation::Rotating { change: _ } => OperationType::Rotating,
			Operation::Scaling { change: _ } => OperationType::Scaling,
		}
	}

	fn to_daffine(&self) -> DAffine2 {
		match self {
			Operation::Translating { change } => DAffine2::from_translation(change.to_dvec()),
			Operation::Rotating { change } => DAffine2::from_angle(*change),
			Operation::Scaling { change } => DAffine2::from_translation(change.to_dvec()),
			Operation::None => DAffine2::IDENTITY,
		}
	}

	pub fn change<'a>(&mut self, new: Self, selected: &Vec<&'a Vec<LayerId>>, responses: &mut VecDeque<Message>) {
		let transform = self.to_daffine().inverse() * new.to_daffine();
		for path in selected {
			responses.push_back(
				DocumentOperation::TransformLayer {
					path: (**path).clone(),
					transform: transform.to_cols_array(),
				}
				.into(),
			);
		}
		*self = new.clone();
	}
	pub fn switch<'a>(&mut self, new: OperationType, selected: &Vec<&'a Vec<LayerId>>, responses: &mut VecDeque<Message>) {
		if !(self.get_type() == new) {
			log::info!("Switching to {:?} layer transform from {:?}", new, &self.get_type());
			self.change(
				match new {
					OperationType::None => Self::None,
					OperationType::Translating => Self::Translating { change: Axis::default() },
					OperationType::Rotating => Self::Rotating { change: 0. },
					OperationType::Scaling => Self::Scaling { change: Axis::default() },
				},
				selected,
				responses,
			);
		}
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

impl MessageHandler<TransformLayerMessage, (&mut HashMap<Vec<LayerId>, LayerData>, &Document, &InputPreprocessor)> for TransformLayerMessageHandler {
	fn process_action(&mut self, message: TransformLayerMessage, data: (&mut HashMap<Vec<LayerId>, LayerData>, &Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, _, _) = data;
		let selected: Vec<_> = layerdata.iter().filter_map(|(path, data)| data.selected.then(|| path)).collect();
		use TransformLayerMessage::*;
		match message {
			BeginTranslate => self.operation.switch(OperationType::Translating, &selected, responses),
			BeginRotate => self.operation.switch(OperationType::Rotating, &selected, responses),
			BeginScale => self.operation.switch(OperationType::Scaling, &selected, responses),
			CancelOperation => self.operation.switch(OperationType::None, &selected, responses),
			ApplyOperation => self.operation = Operation::None,
			MouseMove => log::info!("Mouse Moved"),
			TypeNum(k) => log::info!("Num Typed {}", k),
			TypeDelete => log::info!("Delete "),
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
			);
			common.extend(active);
		}
		common
	}
}
