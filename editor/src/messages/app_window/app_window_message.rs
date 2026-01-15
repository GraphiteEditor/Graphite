use crate::messages::prelude::*;

use super::app_window_message_handler::AppWindowPlatform;

#[impl_message(Message, AppWindow)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AppWindowMessage {
	UpdatePlatform { platform: AppWindowPlatform },
	PointerLock,
	PointerLockMove { x: f64, y: f64 },
	Close,
	Minimize,
	Maximize,
	Drag,
	Hide,
	HideOthers,
	ShowAll,
}
