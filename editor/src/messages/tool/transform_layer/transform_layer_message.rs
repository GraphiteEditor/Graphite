use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::prelude::*;
use glam::DAffine2;

#[impl_message(Message, ToolMessage, TransformLayer)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformLayerMessage {
	Overlays(OverlayContext, DAffine2),
	// Messages
	ApplyTransformOperation,
	BeginGrab,
	BeginRotate,
	BeginScale,
	CancelTransformOperation,
	ConstrainX,
	ConstrainY,
	PointerMove { slow_key: Key, snap_key: Key },
	SelectionChanged,
	TypeBackspace,
	TypeDecimalPoint,
	TypeDigit { digit: u8 },
	TypeNegate,
}
