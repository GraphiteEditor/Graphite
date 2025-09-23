use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_display_handler_t, cef_base_ref_counted_t, cef_log_severity_t::*};
use cef::{CefString, ImplDisplayHandler, WrapDisplayHandler};

pub(crate) struct DisplayHandlerImpl {
	object: *mut RcImpl<_cef_display_handler_t, Self>,
}

impl DisplayHandlerImpl {
	pub fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplDisplayHandler for DisplayHandlerImpl {
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
		// TODO: hide this files location in the log
		match level.as_ref() {
			LOGSEVERITY_FATAL => tracing::error!("Console [{}:{}]: {}", source, line, message),
			LOGSEVERITY_ERROR => tracing::error!("Console [{}:{}]: {}", source, line, message),
			LOGSEVERITY_WARNING => tracing::warn!("Console [{}:{}]: {}", source, line, message),
			LOGSEVERITY_INFO => tracing::info!("Console [{}:{}]: {}", source, line, message),
			LOGSEVERITY_DEFAULT => tracing::debug!("Console [{}:{}]: {}", source, line, message),
			LOGSEVERITY_VERBOSE => tracing::trace!("Console [{}:{}]: {}", source, line, message),
			LOGSEVERITY_DISABLE => tracing::trace!("Console [{}:{}]: {}", source, line, message),
			_ => tracing::trace!("Console [{}:{}]: {}", source, line, message),
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
		Self { object: self.object }
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
