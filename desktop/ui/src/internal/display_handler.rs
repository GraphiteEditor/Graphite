use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_display_handler_t, cef_base_ref_counted_t, cef_cursor_type_t::*, cef_log_severity_t::*};
use cef::{CefString, ImplDisplayHandler, Point, Size, WrapDisplayHandler};
use winit::cursor::CursorIcon;

use crate::delegate::BrowserDelegate;

pub(crate) struct DisplayHandlerImpl {
	object: *mut RcImpl<_cef_display_handler_t, Self>,
	delegate: BrowserDelegate,
}

impl DisplayHandlerImpl {
	pub fn new(delegate: BrowserDelegate) -> Self {
		Self {
			object: std::ptr::null_mut(),
			delegate,
		}
	}
}

#[cfg(not(target_os = "macos"))]
type CefCursorHandle = cef::CursorHandle;
#[cfg(target_os = "macos")]
type CefCursorHandle = *mut u8;

impl ImplDisplayHandler for DisplayHandlerImpl {
	fn on_cursor_change(&self, _browser: Option<&mut cef::Browser>, _cursor: CefCursorHandle, cursor_type: cef::CursorType, custom_cursor_info: Option<&cef::CursorInfo>) -> std::ffi::c_int {
		if let Some(custom_cursor_info) = custom_cursor_info {
			let Size { width, height } = custom_cursor_info.size;
			let Point { x: hotspot_x, y: hotspot_y } = custom_cursor_info.hotspot;
			let buffer_size = (width * height * 4) as usize;
			let buffer_ptr = custom_cursor_info.buffer as *const u8;

			if !buffer_ptr.is_null() && buffer_ptr.align_offset(std::mem::align_of::<u8>()) == 0 {
				let buffer = unsafe { std::slice::from_raw_parts(buffer_ptr, buffer_size) }.to_vec();
				self.delegate.cursor_change(crate::Cursor::Custom {
					rgba: buffer,
					width: width as u16,
					height: height as u16,
					hotspot_x: hotspot_x as u16,
					hotspot_y: hotspot_y as u16,
				});
				return 1; // We handled the cursor change.
			}
		}

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
			CT_NOTALLOWED => CursorIcon::NotAllowed,
			CT_ZOOMIN => CursorIcon::ZoomIn,
			CT_ZOOMOUT => CursorIcon::ZoomOut,
			CT_GRAB => CursorIcon::Grab,
			CT_GRABBING => CursorIcon::Grabbing,
			CT_MIDDLE_PANNING_VERTICAL => CursorIcon::AllScroll,
			CT_MIDDLE_PANNING_HORIZONTAL => CursorIcon::AllScroll,
			CT_DND_NONE => CursorIcon::Default,
			CT_DND_MOVE => CursorIcon::Move,
			CT_DND_COPY => CursorIcon::Copy,
			CT_DND_LINK => CursorIcon::Alias,
			CT_NUM_VALUES => CursorIcon::Default,
			CT_NONE => {
				self.delegate.cursor_change(crate::Cursor::None);
				return 1; // We handled the cursor change.
			}
			_ => CursorIcon::Default,
		};

		self.delegate.cursor_change(cursor.into());

		1 // We handled the cursor change.
	}

	fn on_console_message(&self, _browser: Option<&mut cef::Browser>, level: cef::LogSeverity, message: Option<&CefString>, source: Option<&CefString>, line: std::ffi::c_int) -> std::ffi::c_int {
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

impl Clone for DisplayHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			delegate: self.delegate.clone(),
		}
	}
}
impl Rc for DisplayHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapDisplayHandler for DisplayHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_display_handler_t, Self>) {
		self.object = object;
	}
}
