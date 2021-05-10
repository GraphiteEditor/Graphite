use document_core::{color::Color, LayerId};

use crate::tools::ToolType;

#[derive(Debug, Clone)]
pub enum Action {
	SelectTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	SelectLayer(Vec<LayerId>),
	SelectDocument(usize),
	ToggleLayerVisibility(Vec<LayerId>),
	ToggleLayerExpansion(Vec<LayerId>),
	DeleteLayer(Vec<LayerId>),
	AddLayer(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	SwapColors,
	ResetColors,
	Undo,
	Redo,
	Center,
	UnCenter,
	Confirm,
	SnapAngle,
	UnSnapAngle,
	LockAspectRatio,
	UnlockAspectRatio,
	Abort,
	IncreaseSize,
	DecreaseSize,
	Save,
	// …
	LmbDown,
	RmbDown,
	MmbDown,
	LmbUp,
	RmbUp,
	MmbUp,
	MouseMove,
	TextKeyPress(char),
}
