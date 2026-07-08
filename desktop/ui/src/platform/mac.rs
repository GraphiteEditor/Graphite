use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2::{ClassType, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEvent, NSResponder};
use objc2_foundation::NSObject;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};

static HANDLING_SEND_EVENT: AtomicBool = AtomicBool::new(false);

define_class!(
	#[unsafe(super(NSApplication, NSResponder, NSObject))]
	#[name = "GraphiteCefHostApplication"]
	struct CefHostApplication;

	unsafe impl CrAppProtocol for CefHostApplication {
		#[unsafe(method(isHandlingSendEvent))]
		fn is_handling_send_event(&self) -> Bool {
			Bool::new(HANDLING_SEND_EVENT.load(Ordering::Relaxed))
		}
	}

	unsafe impl CrAppControlProtocol for CefHostApplication {
		#[unsafe(method(setHandlingSendEvent:))]
		fn set_handling_send_event(&self, handling: Bool) {
			HANDLING_SEND_EVENT.store(handling.as_bool(), Ordering::Relaxed);
		}
	}

	unsafe impl CefAppProtocol for CefHostApplication {}

	impl CefHostApplication {
		#[unsafe(method(sendEvent:))]
		fn send_event(&self, event: &NSEvent) {
			let was_handling = HANDLING_SEND_EVENT.swap(true, Ordering::Relaxed);
			let _: () = unsafe { msg_send![super(self), sendEvent: event] };
			HANDLING_SEND_EVENT.store(was_handling, Ordering::Relaxed);
		}
	}
);

pub(crate) fn install_application() {
	let app: Retained<NSApplication> = unsafe { msg_send![CefHostApplication::class(), sharedApplication] };
	app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
}

pub(crate) fn spawn_parent_watchdog(main_pid: u32) {
	let result = std::thread::Builder::new().name("parent-watchdog".to_string()).spawn(move || {
		loop {
			// SAFETY: getppid is always safe to call.
			if unsafe { libc::getppid() } as u32 != main_pid {
				tracing::warn!("Main process is gone, exiting CEF host");
				std::process::exit(0);
			}
			std::thread::sleep(Duration::from_millis(500));
		}
	});
	if let Err(e) = result {
		tracing::error!("Failed to spawn the parent watchdog: {e}");
	}
}
