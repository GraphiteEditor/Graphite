#[cfg(target_os = "linux")]
use std::env;

use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{BrowserProcessHandler, CefString, ImplApp, ImplCommandLine, SchemeRegistrar, WrapApp};

use super::browser_process_handler::BrowserProcessHandlerImpl;
use super::scheme_handler_factory::SchemeHandlerFactoryImpl;
use crate::cef::CefEventHandler;

pub(crate) struct BrowserProcessAppImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_app_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler + Clone> BrowserProcessAppImpl<H> {
	pub(crate) fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}
}

impl<H: CefEventHandler + Clone> ImplApp for BrowserProcessAppImpl<H> {
	fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
		Some(BrowserProcessHandler::new(BrowserProcessHandlerImpl::new(self.event_handler.clone())))
	}

	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		SchemeHandlerFactoryImpl::<H>::register_schemes(registrar);
	}

	fn on_before_command_line_processing(&self, _process_type: Option<&cef::CefString>, command_line: Option<&mut cef::CommandLine>) {
		if let Some(cmd) = command_line {
			#[cfg(not(feature = "accelerated_paint"))]
			{
				// Disable GPU acceleration when accelerated_paint feature is not enabled
				cmd.append_switch(Some(&CefString::from("disable-gpu")));
				cmd.append_switch(Some(&CefString::from("disable-gpu-compositing")));
			}

			#[cfg(feature = "accelerated_paint")]
			{
				// Enable GPU acceleration switches for better performance
				cmd.append_switch(Some(&CefString::from("enable-gpu-rasterization")));
				cmd.append_switch(Some(&CefString::from("enable-accelerated-2d-canvas")));
			}

			#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
			{
				// Use Vulkan for accelerated painting
				cmd.append_switch_with_value(Some(&CefString::from("use-angle")), Some(&CefString::from("vulkan")));
			}

			// Tell CEF to use Wayland if available
			#[cfg(target_os = "linux")]
			{
				let use_wayland = env::var("WAYLAND_DISPLAY")
					.ok()
					.filter(|var| !var.is_empty())
					.or_else(|| env::var("WAYLAND_SOCKET").ok())
					.filter(|var| !var.is_empty())
					.is_some();
				if use_wayland {
					cmd.append_switch_with_value(Some(&CefString::from("ozone-platform")), Some(&CefString::from("wayland")));
				}
			}

			#[cfg(target_os = "macos")]
			{
				// Hide user prompt asking for keychain access
				cmd.append_switch(Some(&CefString::from("use-mock-keychain")));
			}
		}
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler + Clone> Clone for BrowserProcessAppImpl<H> {
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
impl<H: CefEventHandler> Rc for BrowserProcessAppImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler + Clone> WrapApp for BrowserProcessAppImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
