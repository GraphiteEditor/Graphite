mod browser_process_app;
mod browser_process_client;
mod browser_process_handler;
mod browser_process_life_span_handler;
mod browser_process_load_handler;

mod render_process_app;
mod render_process_handler;
mod render_process_v8_handler;

mod resource_handler;
mod scheme_handler_factory;

mod display_handler;

pub(super) mod render_handler;

#[cfg(not(target_os = "macos"))]
pub(super) mod task;

pub(super) use browser_process_app::BrowserProcessAppImpl;
pub(super) use browser_process_client::BrowserProcessClientImpl;
pub(super) use render_process_app::RenderProcessAppImpl;
pub(super) use scheme_handler_factory::SchemeHandlerFactoryImpl;

use crate::cef::ViewInfo;

pub(super) trait NotifyViewInfoChanged {
	fn notify_view_info_changed(&self, view_info: &ViewInfo);
}
impl<T: cef::ImplBrowserHost> NotifyViewInfoChanged for T {
	fn notify_view_info_changed(&self, view_info: &ViewInfo) {
		self.set_zoom_level(view_info.zoom());
		self.notify_screen_info_changed();
		self.was_resized();
	}
}
