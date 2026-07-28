use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_resource_request_handler_t, cef_base_ref_counted_t};
use cef::{Browser, Callback, CefString, Frame, ImplRequest, ImplResourceRequestHandler, Request, ReturnValue, WrapResourceRequestHandler};

use crate::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};

// TODO: Deny all external requests once we stop relying on google fonts for font preview
fn is_allowed_url(url: &str) -> bool {
	url.starts_with(&format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/")) || url.starts_with("https://fonts.googleapis.com/css2") || url.starts_with("https://fonts.gstatic.com/")
}

pub(crate) struct ResourceRequestHandlerImpl {
	object: *mut RcImpl<_cef_resource_request_handler_t, Self>,
}

impl ResourceRequestHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplResourceRequestHandler for ResourceRequestHandlerImpl {
	fn on_before_resource_load(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, request: Option<&mut Request>, _callback: Option<&mut Callback>) -> ReturnValue {
		let Some(request) = request else { return ReturnValue::CANCEL };
		let url = CefString::from(&request.url()).to_string();
		if is_allowed_url(&url) {
			ReturnValue::CONTINUE
		} else {
			tracing::error!("Blocked resource load: {}", url);
			ReturnValue::CANCEL
		}
	}

	fn get_raw(&self) -> *mut _cef_resource_request_handler_t {
		self.object.cast()
	}
}

impl Clone for ResourceRequestHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for ResourceRequestHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapResourceRequestHandler for ResourceRequestHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_resource_request_handler_t, Self>) {
		self.object = object;
	}
}
