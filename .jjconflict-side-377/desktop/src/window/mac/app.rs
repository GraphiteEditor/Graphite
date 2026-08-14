use std::ffi::CStr;
use std::ffi::OsStr;
use std::ops::Deref;
use std::ops::DerefMut;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Mutex, Once};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSEvent, NSEventType, NSResponder};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSURL};

use crate::event::{AppEvent, AppEventScheduler};

static APP_EVENT_SCHEDULER: Mutex<Option<AppEventScheduler>> = Mutex::new(None);
static PENDING_EVENTS: Mutex<Option<Vec<AppEvent>>> = Mutex::new(Some(Vec::new()));

fn dispatch_event(event: AppEvent) {
	let app_event_scheduler_guard = APP_EVENT_SCHEDULER.lock().unwrap();
	if let Some(app_event_scheduler) = app_event_scheduler_guard.deref() {
		app_event_scheduler.schedule(event);
	} else if let Some(pending_events) = PENDING_EVENTS.lock().unwrap().deref_mut() {
		pending_events.push(event);
	} else {
		tracing::error!("Failed to dispatch event");
	}
}

fn instance() -> objc2::rc::Retained<NSApplication> {
	unsafe { msg_send![GraphiteApplication::class(), sharedApplication] }
}

static INSTALL_DELEGATE: Once = Once::new();

pub(super) fn init() {
	let _ = instance();

	INSTALL_DELEGATE.call_once(|| {
		let mtm = MainThreadMarker::new().expect("should only ever be called from main thread");
		let delegate: Retained<GraphiteApplicationDelegate> = unsafe { msg_send![super(GraphiteApplicationDelegate::alloc(mtm).set_ivars(())), init] };
		instance().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
		std::mem::forget(delegate);
	});
}

pub(super) fn setup(app_event_scheduler: AppEventScheduler) {
	let mut app_event_scheduler_guard = APP_EVENT_SCHEDULER.lock().unwrap();

	if let Some(mut pending_events) = PENDING_EVENTS.lock().unwrap().take() {
		pending_events.drain(..).for_each(|event| {
			app_event_scheduler.schedule(event);
		});
	} else {
		tracing::error!("Failed to take PENDING_EVENTS and schedule them. This a bug.");
	}

	*app_event_scheduler_guard = Some(app_event_scheduler);
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

define_class!(
	#[unsafe(super(NSObject))]
	#[thread_kind = MainThreadOnly]
	#[name = "GraphiteApplicationDelegate"]
	struct GraphiteApplicationDelegate;

	unsafe impl NSObjectProtocol for GraphiteApplicationDelegate {}

	unsafe impl NSApplicationDelegate for GraphiteApplicationDelegate {
		#[unsafe(method(application:openURLs:))]
		fn application_open_urls(&self, _application: &NSApplication, urls: &NSArray<NSURL>) {
			let paths = (0..urls.count())
				.filter_map(|index| {
					let url = urls.objectAtIndex(index);
					if !url.isFileURL() {
						tracing::error!("Ignoring open URL event for non-file URL: {:?}", url);
						return None;
					}
					let cstr = unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) };
					let path = PathBuf::from(OsStr::from_bytes(cstr.to_bytes()));
					Some(path)
				})
				.collect::<Vec<_>>();

			dispatch_event(AppEvent::OpenFiles(paths));
		}
	}
);
