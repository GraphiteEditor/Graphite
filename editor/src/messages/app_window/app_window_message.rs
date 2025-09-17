use crate::messages::prelude::*;

use super::app_window_message_handler::AppWindowPlatform;

#[impl_message(Message, AppWindow)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AppWindowMessage {
	AppWindowMinimize,
	AppWindowMaximize,
	AppWindowUpdatePlatform { platform: AppWindowPlatform },
	AppWindowDrag,
	AppWindowClose,
}
