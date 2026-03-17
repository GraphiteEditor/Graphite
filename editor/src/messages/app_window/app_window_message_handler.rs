use crate::application::{Environment, Platform};
use crate::messages::prelude::*;
use crate::{application::Host, messages::app_window::AppWindowMessage};
use graphite_proc_macros::{ExtractField, message_handler_data};

#[derive(Debug, Clone, Default, ExtractField)]
pub struct AppWindowMessageHandler {}

#[message_handler_data]
impl MessageHandler<AppWindowMessage, ()> for AppWindowMessageHandler {
	fn process_message(&mut self, message: AppWindowMessage, responses: &mut std::collections::VecDeque<Message>, _: ()) {
		match message {
			AppWindowMessage::PointerLock => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowPointerLock);
			}
			AppWindowMessage::PointerLockMove { x, y } => {
				responses.add(FrontendMessage::WindowPointerLockMove { position: (x, y) });
			}
			AppWindowMessage::Close => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowClose);
			}
			AppWindowMessage::Minimize => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowMinimize);
			}
			AppWindowMessage::Maximize => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowMaximize);
			}
			AppWindowMessage::Fullscreen => {
				responses.add(FrontendMessage::WindowFullscreen);
			}
			AppWindowMessage::Drag => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowDrag);
			}
			AppWindowMessage::Hide => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowHide);
			}
			AppWindowMessage::HideOthers => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowHideOthers);
			}
			AppWindowMessage::ShowAll => {
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowShowAll);
			}
			AppWindowMessage::Restart => {
				responses.add(PortfolioMessage::AutoSaveAllDocuments);
				#[cfg(not(target_family = "wasm"))]
				responses.add(FrontendMessage::WindowRestart);
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum AppWindowPlatform {
	#[default]
	Web,
	Windows,
	Mac,
	Linux,
}

impl From<&Environment> for AppWindowPlatform {
	fn from(environment: &Environment) -> Self {
		match (environment.platform, environment.host) {
			(Platform::Web, _) => AppWindowPlatform::Web,
			(Platform::Desktop, Host::Linux) => AppWindowPlatform::Linux,
			(Platform::Desktop, Host::Mac) => AppWindowPlatform::Mac,
			(Platform::Desktop, Host::Windows) => AppWindowPlatform::Windows,
		}
	}
}
