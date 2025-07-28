use crate::messages::prelude::*;

#[impl_message(Message, AppWindow)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AppWindowMessage {
	AppWindowMinimize,
	AppWindowMaximize,
	AppWindowClose,
}
