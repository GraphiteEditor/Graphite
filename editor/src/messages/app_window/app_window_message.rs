use crate::messages::prelude::*;

#[impl_message(Message, AppWindow)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AppWindowMessage {
	PointerLock,
	PointerUnlock,
	// Relative pointer movement in logical viewport units (platforms divide physical deltas by the viewport scale)
	PointerLockMove { x: f64, y: f64 },
	DirectInput { enabled: bool },
	Restart,
	Close,
	Minimize,
	Maximize,
	Fullscreen,
	Drag,
	Focus,
	Hide,
	HideOthers,
	ShowAll,
}
