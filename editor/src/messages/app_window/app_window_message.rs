use crate::messages::prelude::*;

#[impl_message(Message, AppWindow)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AppWindowMessage {
	PointerLock,
	PointerLockMove { x: f64, y: f64 },
	Close,
	Minimize,
	Maximize,
	Fullscreen,
	Drag,
	Hide,
	HideOthers,
	ShowAll,
}
