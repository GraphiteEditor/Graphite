use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_scheme_handler_factory_t, cef_base_ref_counted_t, cef_scheme_options_t};
use cef::{Browser, CefString, Frame, ImplRequest, ImplSchemeHandlerFactory, ImplSchemeRegistrar, Request, ResourceHandler, SchemeRegistrar, WrapSchemeHandlerFactory};

use super::resource_handler::ResourceHandlerImpl;
use crate::cef::CefEventHandler;
use crate::cef::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};

pub(crate) struct SchemeHandlerFactoryImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_scheme_handler_factory_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler> SchemeHandlerFactoryImpl<H> {
	pub(crate) fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}

	pub(crate) fn register_schemes(registrar: Option<&mut SchemeRegistrar>) {
		if let Some(registrar) = registrar {
			let mut scheme_options = 0;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_STANDARD as i32;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_FETCH_ENABLED as i32;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_SECURE as i32;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_CORS_ENABLED as i32;
			registrar.add_custom_scheme(Some(&CefString::from(RESOURCE_SCHEME)), scheme_options);
		}
	}
}

impl<H: CefEventHandler> ImplSchemeHandlerFactory for SchemeHandlerFactoryImpl<H> {
	fn create(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, _scheme_name: Option<&CefString>, request: Option<&mut Request>) -> Option<ResourceHandler> {
		if let Some(request) = request {
			let url = CefString::from(&request.url()).to_string();
			let path = url
				.strip_prefix(&format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/"))
				.expect("CEF should only call this for our custom scheme and domain that we registered this factory for");
			let resource = self.event_handler.load_resource(path.to_string().into());
			return Some(ResourceHandler::new(ResourceHandlerImpl::new(resource)));
		}
		None
	}
	fn get_raw(&self) -> *mut _cef_scheme_handler_factory_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for SchemeHandlerFactoryImpl<H> {
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
impl<H: CefEventHandler> Rc for SchemeHandlerFactoryImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapSchemeHandlerFactory for SchemeHandlerFactoryImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_scheme_handler_factory_t, Self>) {
		self.object = object;
	}
}
