use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::consts::APP_NAME;
use crate::event::AppEventScheduler;
use crate::wrapper::messages::MenuItem;

pub(super) struct NativeWindowImpl {
	menu: menu::Menu,
}

impl super::NativeWindow for NativeWindowImpl {
	fn configure(attributes: WindowAttributes, _event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		let mac_window = winit::platform::macos::WindowAttributesMacOS::default()
			.with_titlebar_transparent(true)
			.with_fullsize_content_view(true)
			.with_title_hidden(true);
		attributes.with_platform_attributes(Box::new(mac_window))
	}

	fn new(_window: &dyn Window, app_event_scheduler: AppEventScheduler) -> Self {
		let menu = menu::Menu::new(app_event_scheduler, APP_NAME);

		NativeWindowImpl { menu }
	}

	fn update_menu(&self, entries: Vec<MenuItem>) {
		self.menu.update(entries);
	}
}

mod menu;
