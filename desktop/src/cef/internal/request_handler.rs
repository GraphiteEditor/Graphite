use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_request_handler_t, cef_base_ref_counted_t};
use cef::{AuthCallback, Browser, CefString, Frame, ImplRequest, ImplRequestHandler, Request, ResourceRequestHandler, WrapRequestHandler};
use std::ffi::c_int;

use super::resource_request_handler::ResourceRequestHandlerImpl;
use crate::cef::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};

pub(crate) struct RequestHandlerImpl {
	object: *mut RcImpl<_cef_request_handler_t, Self>,
}

impl RequestHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplRequestHandler for RequestHandlerImpl {
	fn on_before_browse(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, request: Option<&mut Request>, _user_gesture: c_int, _is_redirect: c_int) -> c_int {
		let Some(request) = request else { return 1 };
		let url = CefString::from(&request.url()).to_string();
		if url.starts_with(&format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/")) {
			0
		} else {
			tracing::warn!("Blocked navigation to: {}", url);
			1
		}
	}

	fn resource_request_handler(
		&self,
		_browser: Option<&mut Browser>,
		_frame: Option<&mut Frame>,
		_request: Option<&mut Request>,
		_is_navigation: c_int,
		_is_download: c_int,
		_request_initiator: Option<&CefString>,
		_disable_default_handling: Option<&mut c_int>,
	) -> Option<ResourceRequestHandler> {
		Some(ResourceRequestHandler::new(ResourceRequestHandlerImpl::new()))
	}

	fn auth_credentials(
		&self,
		_browser: Option<&mut Browser>,
		_origin_url: Option<&CefString>,
		_is_proxy: c_int,
		_host: Option<&CefString>,
		_port: c_int,
		_realm: Option<&CefString>,
		_scheme: Option<&CefString>,
		_callback: Option<&mut AuthCallback>,
	) -> c_int {
		0
	}

	fn get_raw(&self) -> *mut _cef_request_handler_t {
		self.object.cast()
	}
}

impl Clone for RequestHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for RequestHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapRequestHandler for RequestHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_request_handler_t, Self>) {
		self.object = object;
	}
}
