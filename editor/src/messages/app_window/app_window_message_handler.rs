use crate::messages::app_window::AppWindowMessage;
use crate::messages::prelude::*;
use graphite_proc_macros::{ExtractField, message_handler_data};

#[derive(Debug, Clone, Default, ExtractField)]
pub struct AppWindowMessageHandler {
	platform: AppWindowPlatform,
	maximized: bool,
	minimized: bool,
}

#[message_handler_data]
impl MessageHandler<AppWindowMessage, ()> for AppWindowMessageHandler {
	fn process_message(&mut self, message: AppWindowMessage, responses: &mut std::collections::VecDeque<Message>, _: ()) {
		match message {
			AppWindowMessage::AppWindowMaximize => {
				self.maximized = !self.maximized;
				responses.add(FrontendMessage::UpdateWindowState {
					maximized: self.maximized,
					minimized: self.minimized,
				});
			}
			AppWindowMessage::AppWindowMinimize => {
				self.minimized = !self.minimized;
				responses.add(FrontendMessage::UpdateWindowState {
					maximized: self.maximized,
					minimized: self.minimized,
				});
			}
			AppWindowMessage::AppWindowUpdatePlatform { platform } => {
				self.platform = platform;
				responses.add(FrontendMessage::UpdatePlatform { platform: self.platform });
			}
			AppWindowMessage::AppWindowClose => {
				responses.add(FrontendMessage::CloseWindow);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(AppWindowMessageDiscriminant;)
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum AppWindowPlatform {
	#[default]
	Web,
	Windows,
	Mac,
	Linux,
}
