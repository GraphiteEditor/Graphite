use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_display_handler_t, cef_base_ref_counted_t, cef_cursor_type_t::*, cef_log_severity_t::*};
use cef::{CefString, ImplDisplayHandler, WrapDisplayHandler};
use winit::cursor::CursorIcon;

use crate::cef::CefEventHandler;

pub(crate) struct DisplayHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_display_handler_t, Self>,
	event_handler: H,
}

impl<H: CefEventHandler> DisplayHandlerImpl<H> {
	pub fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}
}

#[cfg(not(target_os = "macos"))]
type CefCursorHandle = cef::CursorHandle;
#[cfg(target_os = "macos")]
type CefCursorHandle = *mut u8;

impl<H: CefEventHandler> ImplDisplayHandler for DisplayHandlerImpl<H> {
	fn on_cursor_change(&self, _browser: Option<&mut cef::Browser>, _cursor: CefCursorHandle, cursor_type: cef::CursorType, _custom_cursor_info: Option<&cef::CursorInfo>) -> ::std::os::raw::c_int {
		let cursor = match cursor_type.into() {
			CT_POINTER => CursorIcon::Default,
			CT_CROSS => CursorIcon::Crosshair,
			CT_HAND => CursorIcon::Pointer,
			CT_IBEAM => CursorIcon::Text,
			CT_WAIT => CursorIcon::Wait,
			CT_HELP => CursorIcon::Help,
			CT_EASTRESIZE => CursorIcon::EResize,
			CT_NORTHRESIZE => CursorIcon::NResize,
			CT_NORTHEASTRESIZE => CursorIcon::NeResize,
			CT_NORTHWESTRESIZE => CursorIcon::NwResize,
			CT_SOUTHRESIZE => CursorIcon::SResize,
			CT_SOUTHEASTRESIZE => CursorIcon::SeResize,
			CT_SOUTHWESTRESIZE => CursorIcon::SwResize,
			CT_WESTRESIZE => CursorIcon::WResize,
			CT_NORTHSOUTHRESIZE => CursorIcon::NsResize,
			CT_EASTWESTRESIZE => CursorIcon::EwResize,
			CT_NORTHEASTSOUTHWESTRESIZE => CursorIcon::NeswResize,
			CT_NORTHWESTSOUTHEASTRESIZE => CursorIcon::NwseResize,
			CT_COLUMNRESIZE => CursorIcon::ColResize,
			CT_ROWRESIZE => CursorIcon::RowResize,
			CT_MIDDLEPANNING => CursorIcon::AllScroll,
			CT_EASTPANNING => CursorIcon::AllScroll,
			CT_NORTHPANNING => CursorIcon::AllScroll,
			CT_NORTHEASTPANNING => CursorIcon::AllScroll,
			CT_NORTHWESTPANNING => CursorIcon::AllScroll,
			CT_SOUTHPANNING => CursorIcon::AllScroll,
			CT_SOUTHEASTPANNING => CursorIcon::AllScroll,
			CT_SOUTHWESTPANNING => CursorIcon::AllScroll,
			CT_WESTPANNING => CursorIcon::AllScroll,
			CT_MOVE => CursorIcon::Move,
			CT_VERTICALTEXT => CursorIcon::VerticalText,
			CT_CELL => CursorIcon::Cell,
			CT_CONTEXTMENU => CursorIcon::ContextMenu,
			CT_ALIAS => CursorIcon::Alias,
			CT_PROGRESS => CursorIcon::Progress,
			CT_NODROP => CursorIcon::NoDrop,
			CT_COPY => CursorIcon::Copy,
			CT_NONE => CursorIcon::Default,
			CT_NOTALLOWED => CursorIcon::NotAllowed,
			CT_ZOOMIN => CursorIcon::ZoomIn,
			CT_ZOOMOUT => CursorIcon::ZoomOut,
			CT_GRAB => CursorIcon::Grab,
			CT_GRABBING => CursorIcon::Grabbing,
			CT_MIDDLE_PANNING_VERTICAL => CursorIcon::AllScroll,
			CT_MIDDLE_PANNING_HORIZONTAL => CursorIcon::AllScroll,
			CT_CUSTOM => CursorIcon::Default,
			CT_DND_NONE => CursorIcon::Default,
			CT_DND_MOVE => CursorIcon::Move,
			CT_DND_COPY => CursorIcon::Copy,
			CT_DND_LINK => CursorIcon::Alias,
			CT_NUM_VALUES => CursorIcon::Default,
			_ => CursorIcon::Default,
		};

		self.event_handler.cursor_change(cursor.into());

		1 // We handled the cursor change.
	}

	fn on_console_message(
		&self,
		_browser: Option<&mut cef::Browser>,
		level: cef::LogSeverity,
		message: Option<&CefString>,
		source: Option<&CefString>,
		line: ::std::os::raw::c_int,
	) -> ::std::os::raw::c_int {
		let message = message.map(|m| m.to_string()).unwrap_or_default();
		let source = source.map(|s| s.to_string()).unwrap_or_default();
		let line = line as i64;
		let browser_source = format!("{source}:{line}");
		static BROWSER: &str = "browser";
		match level.as_ref() {
			LOGSEVERITY_FATAL | LOGSEVERITY_ERROR => tracing::error!(target: BROWSER, "{browser_source} {message}"),
			LOGSEVERITY_WARNING => tracing::warn!(target: BROWSER, "{browser_source} {message}"),
			LOGSEVERITY_INFO => tracing::info!(target: BROWSER, "{browser_source} {message}"),
			LOGSEVERITY_DEFAULT | LOGSEVERITY_VERBOSE => tracing::debug!(target: BROWSER, "{browser_source} {message}"),
			_ => tracing::trace!(target: BROWSER, "{browser_source} {message}"),
		}
		0
	}

	fn get_raw(&self) -> *mut _cef_display_handler_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for DisplayHandlerImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			event_handler: self.event_handler.clone(),
		}
	}
}
impl<H: CefEventHandler> Rc for DisplayHandlerImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapDisplayHandler for DisplayHandlerImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_display_handler_t, Self>) {
		self.object = object;
	}
}
