use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::transformation::TransformType;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::pivot::PivotGizmo;
use glam::DVec2;

#[impl_message(Message, ToolMessage, TransformLayer)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformLayerMessage {
	// Overlays
	Overlays(OverlayContext),

	// Messages
	ApplyTransformOperation { final_transform: bool },
	BeginTransformOperation { operation: TransformType },
	BeginGrab,
	BeginRotate,
	BeginScale,
	BeginGRS { operation: TransformType },
	BeginGrabPen { last_point: DVec2, handle: DVec2 },
	BeginRotatePen { last_point: DVec2, handle: DVec2 },
	BeginScalePen { last_point: DVec2, handle: DVec2 },
	CancelTransformOperation,
	ConstrainX,
	ConstrainY,
	PointerMove { slow_key: Key, increments_key: Key },
	SelectionChanged,
	TypeBackspace,
	TypeDecimalPoint,
	TypeDigit { digit: u8 },
	TypeNegate,
	SetPivotGizmo { pivot_gizmo: PivotGizmo },
}
