use std::ffi::CStr;
use std::ffi::OsStr;
use std::ops::Deref;
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
static INSTALL_DELEGATE: Once = Once::new();

static LAUNCH_DOCUMENTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

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
			let Some(app_event_scheduler) = APP_EVENT_SCHEDULER.lock().ok() else {
				tracing::error!("Received macOS open URL event before the app event scheduler was initialized");
				return;
			};

			let mut pending_paths_to_open = LAUNCH_DOCUMENTS.lock().unwrap();

			for index in 0..urls.count() {
				let url = urls.objectAtIndex(index);
				if !url.isFileURL() {
					tracing::error!("Ignoring macOS open URL event for non-file URL: {:?}", url);
					continue;
				}

				let path = unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) };
				let path = PathBuf::from(OsStr::from_bytes(path.to_bytes()));

				pending_paths_to_open.push(path);
			}

			if let Some(app_event_scheduler) = app_event_scheduler.deref() {
				app_event_scheduler.schedule(AppEvent::AddLaunchDocuments(std::mem::take(&mut pending_paths_to_open)));
			}
		}
	}
);

fn instance() -> objc2::rc::Retained<NSApplication> {
	unsafe { msg_send![GraphiteApplication::class(), sharedApplication] }
}

pub(super) fn init() {
	let _ = instance();

	INSTALL_DELEGATE.call_once(|| {
		let mtm = MainThreadMarker::new().expect("only ever called from main thread");
		let delegate: Retained<GraphiteApplicationDelegate> = unsafe { msg_send![super(GraphiteApplicationDelegate::alloc(mtm).set_ivars(())), init] };
		instance().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
		std::mem::forget(delegate);
	});
}

pub(super) fn setup(app_event_scheduler: AppEventScheduler) {
	let mut app_event_scheduler_guard = APP_EVENT_SCHEDULER.lock().unwrap();

	let mut pending_paths_to_open = LAUNCH_DOCUMENTS.lock().unwrap();
	app_event_scheduler.schedule(AppEvent::AddLaunchDocuments(std::mem::take(&mut pending_paths_to_open)));

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
