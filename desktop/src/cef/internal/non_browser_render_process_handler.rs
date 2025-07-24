use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_browser_process_handler_t, _cef_render_process_handler_t, cef_base_ref_counted_t, cef_browser_process_handler_t, cef_v8_handler_t, cef_v8_propertyattribute_t};
use cef::{
	CefString, ImplBrowserProcessHandler, ImplRenderProcessHandler, ImplV8Context, ImplV8Value, SchemeHandlerFactory, V8Handler, V8Propertyattribute, V8Value, WrapBrowserProcessHandler,
	WrapRenderProcessHandler, v8_value_create_function,
};

use crate::cef::internal::non_browser_v8_handler::NonBrowserV8HandlerImpl;

pub(crate) struct NonBrowserRenderProcessHandlerImpl {
	object: *mut RcImpl<_cef_render_process_handler_t, Self>,
}
impl NonBrowserRenderProcessHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplRenderProcessHandler for NonBrowserRenderProcessHandlerImpl {
	fn on_context_created(&self, browser: Option<&mut cef::Browser>, frame: Option<&mut cef::Frame>, context: Option<&mut cef::V8Context>) {
		let Some(context) = context else {
			tracing::event!(tracing::Level::ERROR, "No browser in RenderProcessHandlerImpl::on_context_created");
			return;
		};
		let mut v8_handler = V8Handler::new(NonBrowserV8HandlerImpl::new());
		let Some(mut function) = v8_value_create_function(Some(&CefString::from("sendMessageToCef")), Some(&mut v8_handler)) else {
			tracing::event!(tracing::Level::ERROR, "Failed to create V8 function");
			return;
		};
		let Some(global) = context.global() else {
			tracing::event!(tracing::Level::ERROR, "No global object in RenderProcessHandlerImpl::on_context_created");
			return;
		};

		global.set_value_bykey(Some(&CefString::from("sendMessageToCef")), Some(&mut function), V8Propertyattribute::default());
	}

	fn get_raw(&self) -> *mut _cef_render_process_handler_t {
		self.object.cast()
	}
}

impl Clone for NonBrowserRenderProcessHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for NonBrowserRenderProcessHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapRenderProcessHandler for NonBrowserRenderProcessHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_render_process_handler_t, Self>) {
		self.object = object;
	}
}
