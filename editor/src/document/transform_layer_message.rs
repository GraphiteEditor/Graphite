use crate::input::keyboard::Key;
use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, TransformLayers)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TransformLayerMessage {
	ApplyTransformOperation,
	BeginGrab,
	BeginRotate,
	BeginScale,
	CancelTransformOperation,
	ConstrainX,
	ConstrainY,
	MouseMove { slow_key: Key, snap_key: Key },
	TypeBackspace,
	TypeDecimalPoint,
	TypeDigit { digit: u8 },
	TypeNegate,
}
