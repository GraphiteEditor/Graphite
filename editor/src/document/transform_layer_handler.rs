pub use super::layer_panel::*;

use super::LayerData;

use crate::input::{mouse::ViewportBounds, mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use glam::DVec2;
use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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
	axis: Axis,

	mouse_pos: ViewportPosition,
	previous_val: DVec2,
	change: DVec2,
}

impl TransformLayerMessageHandler {
	fn create_document_transform_from_layerdata(&self, layerdata: &LayerData, viewport_bounds: &ViewportBounds, responses: &mut VecDeque<Message>) {
		let half_viewport = viewport_bounds.size() / 2.;
		let scaled_half_viewport = half_viewport / layerdata.scale;
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path: vec![],
				transform: layerdata.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
			}
			.into(),
		);
	}
}

impl MessageHandler<TransformLayerMessage, (&mut LayerData, &Document, &InputPreprocessor)> for TransformLayerMessageHandler {
	fn process_action(&mut self, message: TransformLayerMessage, data: (&mut LayerData, &Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, document, ipp) = data;
		use TransformLayerMessage::*;
		match message {
			BeginTranslate => self.operation = Operation::Translating { change: Axis::default() },
			BeginScale => todo!(),
			BeginRotate => todo!(),
			CancelOperation => todo!(),
			ApplyOperation => todo!(),
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
			let snapping = actions!(TransformLayerMessageDiscriminant;
				MouseMove,
				CancelOperation,
				ApplyOperation,
				TypeNum,
				TypeDelete,
			);
			common.extend(snapping);
		}
		common
	}
}
