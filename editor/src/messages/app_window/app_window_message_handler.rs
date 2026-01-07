use crate::messages::app_window::AppWindowMessage;
use crate::messages::prelude::*;
use graphite_proc_macros::{ExtractField, message_handler_data};

#[derive(Debug, Clone, Default, ExtractField)]
pub struct AppWindowMessageHandler {
	platform: AppWindowPlatform,
}

#[message_handler_data]
impl MessageHandler<AppWindowMessage, ()> for AppWindowMessageHandler {
	fn process_message(&mut self, message: AppWindowMessage, responses: &mut std::collections::VecDeque<Message>, _: ()) {
		match message {
			AppWindowMessage::UpdatePlatform { platform } => {
				self.platform = platform;
				responses.add(FrontendMessage::UpdatePlatform { platform: self.platform });
			}
			AppWindowMessage::Close => {
				responses.add(FrontendMessage::WindowClose);
			}
			AppWindowMessage::Minimize => {
				responses.add(FrontendMessage::WindowMinimize);
			}
			AppWindowMessage::Maximize => {
				responses.add(FrontendMessage::WindowMaximize);
			}
			AppWindowMessage::Drag => {
				responses.add(FrontendMessage::WindowDrag);
			}
			AppWindowMessage::Hide => {
				responses.add(FrontendMessage::WindowHide);
			}
			AppWindowMessage::HideOthers => {
				responses.add(FrontendMessage::WindowHideOthers);
			}
			AppWindowMessage::ShowAll => {
				responses.add(FrontendMessage::WindowShowAll);
			}
		}
	}
	advertise_actions!(AppWindowMessageDiscriminant;
		Close,
		Minimize,
		Maximize,
		Drag,
		Hide,
		HideOthers,
	);
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum AppWindowPlatform {
	#[default]
	Web,
	Windows,
	Mac,
	Linux,
}
