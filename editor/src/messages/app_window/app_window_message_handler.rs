use crate::application::{Editor, Environment};
use crate::messages::prelude::*;
use crate::{application::Host, messages::app_window::AppWindowMessage};
use graphite_proc_macros::{ExtractField, message_handler_data};

#[derive(Debug, Clone, Default, ExtractField)]
pub struct AppWindowMessageHandler {}

#[message_handler_data]
impl MessageHandler<AppWindowMessage, ()> for AppWindowMessageHandler {
	fn process_message(&mut self, message: AppWindowMessage, responses: &mut std::collections::VecDeque<Message>, _: ()) {
		match message {
			AppWindowMessage::Init => {
				responses.add(FrontendMessage::UpdatePlatform {
					platform: Editor::environment().into(),
				});
			}
			AppWindowMessage::PointerLock => {
				responses.add(FrontendMessage::WindowPointerLock);
			}
			AppWindowMessage::PointerLockMove { x, y } => {
				responses.add(FrontendMessage::WindowPointerLockMove { x, y });
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
			AppWindowMessage::Fullscreen => {
				responses.add(FrontendMessage::WindowFullscreen);
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
		Fullscreen,
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

impl From<&Environment> for AppWindowPlatform {
	fn from(environment: &Environment) -> Self {
		if environment.is_web() {
			return AppWindowPlatform::Web;
		}
		match environment.host {
			Host::Linux => AppWindowPlatform::Linux,
			Host::Mac => AppWindowPlatform::Mac,
			Host::Windows | Host::Unknown => AppWindowPlatform::Windows,
		}
	}
}
