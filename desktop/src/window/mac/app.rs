use objc2::{ClassType, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSEvent, NSEventType, NSResponder};
use objc2_foundation::NSObject;

thread_local! {
	pub(crate) static IS_CEF_WORK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

define_class!(
	#[unsafe(super(NSApplication, NSResponder, NSObject))]
	#[name = "GraphiteApplication"]
	pub(super) struct GraphiteApplication;

	impl GraphiteApplication {
		#[unsafe(method(sendEvent:))]
		fn send_event(&self, event: &NSEvent) {
			if IS_CEF_WORK.with(|c| c.get()) {
				// CEF synthesized an NSEvent for an unhandled key press during message loop work.
				// Drop it to avoid duplicate menu triggers and infinite keyboard event loops with winit.
				return;
			}

			// Route keyDown events straight to the key window to skip native menu shortcut handling.
			if event.r#type() == NSEventType::KeyDown && let Some(key_window) = self.keyWindow() {
				unsafe { msg_send![&key_window, sendEvent: event] }
			} else {
				unsafe { msg_send![super(self), sendEvent: event] }
			}
		}
	}
);

fn instance() -> objc2::rc::Retained<NSApplication> {
	unsafe { msg_send![GraphiteApplication::class(), sharedApplication] }
}

pub(super) fn init() {
	let _ = instance();
}

pub(super) fn hide() {
	instance().hide(None);
}

pub(super) fn hide_others() {
	instance().hideOtherApplications(None);
}

pub(super) fn show_all() {
	instance().unhideAllApplications(None);
}
