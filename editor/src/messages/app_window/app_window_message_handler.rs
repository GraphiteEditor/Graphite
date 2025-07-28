use crate::messages::app_window::AppWindowMessage;
use crate::messages::prelude::*;
use graphite_proc_macros::{ExtractField, message_handler_data};

#[derive(Debug, Clone, Default, ExtractField)]
pub struct AppWindowMessageHandler {
	platform: AppWindowPlatform,
	maximized: bool,
	viewport_hole_punch_active: bool,
}

#[message_handler_data]
impl MessageHandler<AppWindowMessage, ()> for AppWindowMessageHandler {
	fn process_message(&mut self, message: AppWindowMessage, responses: &mut std::collections::VecDeque<Message>, _: ()) {
		match message {
			AppWindowMessage::AppWindowMinimize => {
				self.platform = if self.platform == AppWindowPlatform::Mac {
					AppWindowPlatform::Windows
				} else {
					AppWindowPlatform::Mac
				};
				responses.add(FrontendMessage::UpdatePlatform { platform: self.platform });
			}
			AppWindowMessage::AppWindowMaximize => {
				self.maximized = !self.maximized;
				responses.add(FrontendMessage::UpdateMaximized { maximized: self.maximized });

				self.viewport_hole_punch_active = !self.viewport_hole_punch_active;
				responses.add(FrontendMessage::UpdateViewportHolePunch {
					active: self.viewport_hole_punch_active,
				});
			}
			AppWindowMessage::AppWindowClose => {
				self.platform = AppWindowPlatform::Web;
				responses.add(FrontendMessage::UpdatePlatform { platform: self.platform });
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
