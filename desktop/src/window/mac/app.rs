use objc2::{ClassType, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSEvent, NSEventType, NSResponder};
use objc2_foundation::NSObject;

pub(super) fn init() {
	unsafe {
		let _: &NSApplication = msg_send![GraphiteApplication::class(), sharedApplication];
	}
}

define_class!(
	#[unsafe(super(NSApplication, NSResponder, NSObject))]
	#[name = "GraphiteApplication"]
	pub(super) struct GraphiteApplication;

	impl GraphiteApplication {
		#[unsafe(method(sendEvent:))]
		fn send_event(&self, event: &NSEvent) {
			// Route keyDown events straight to the key window to skip native menu shortcut handling.
			if event.r#type() == NSEventType::KeyDown && let Some(key_window) = self.keyWindow() {
				unsafe { msg_send![&key_window, sendEvent: event] }
			} else {
				unsafe { msg_send![super(self), sendEvent: event] }
			}
		}
	}
);
