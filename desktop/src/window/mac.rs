use winit::event_loop::ActiveEventLoop;
use winit::platform::macos::WindowAttributesMacOS;
use winit::window::{Window, WindowAttributes};

use crate::event::AppEventScheduler;
use crate::wrapper::messages::MenuItem;

mod app;
mod menu;

pub(super) struct NativeWindowImpl {
	menu: menu::Menu,
}

impl super::NativeWindow for NativeWindowImpl {
	fn init() {
		app::init();
	}

	fn configure(attributes: WindowAttributes, _event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		let mac_window = WindowAttributesMacOS::default()
			.with_titlebar_transparent(true)
			.with_fullsize_content_view(true)
			.with_title_hidden(true);
		attributes.with_platform_attributes(Box::new(mac_window))
	}

	fn new(_window: &dyn Window, app_event_scheduler: AppEventScheduler) -> Self {
		let menu = menu::Menu::new(app_event_scheduler);

		NativeWindowImpl { menu }
	}

	fn update_menu(&self, entries: Vec<MenuItem>) {
		self.menu.update(entries);
	}

	fn hide(&self) {
		app::hide();
	}

	fn hide_others(&self) {
		app::hide_others();
	}

	fn show_all(&self) {
		app::show_all();
	}
}
