use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_scheme_handler_factory_t, cef_base_ref_counted_t, cef_scheme_options_t};
use cef::{Browser, CefString, Frame, ImplRequest, ImplSchemeHandlerFactory, ImplSchemeRegistrar, Request, ResourceHandler, SchemeRegistrar, WrapSchemeHandlerFactory};

use super::resource_handler::ResourceHandlerImpl;
use crate::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};
use crate::delegate::BrowserDelegate;

pub(crate) struct SchemeHandlerFactoryImpl {
	object: *mut RcImpl<_cef_scheme_handler_factory_t, Self>,
	delegate: BrowserDelegate,
}
impl SchemeHandlerFactoryImpl {
	pub(crate) fn new(delegate: BrowserDelegate) -> Self {
		Self {
			object: std::ptr::null_mut(),
			delegate,
		}
	}
}

pub(crate) fn register_schemes(registrar: Option<&mut SchemeRegistrar>) {
	if let Some(registrar) = registrar {
		let mut scheme_options = 0;
		scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_STANDARD as i32;
		scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_FETCH_ENABLED as i32;
		scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_SECURE as i32;
		scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_CORS_ENABLED as i32;
		registrar.add_custom_scheme(Some(&RESOURCE_SCHEME.into()), scheme_options);
	}
}

impl ImplSchemeHandlerFactory for SchemeHandlerFactoryImpl {
	fn create(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, _scheme_name: Option<&CefString>, request: Option<&mut Request>) -> Option<ResourceHandler> {
		if let Some(request) = request {
			let url = CefString::from(&request.url()).to_string();
			let path = url
				.strip_prefix(&format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/"))
				.expect("CEF should only call this for our custom scheme and domain that we registered this factory for");
			let resource = self.delegate.load_resource(path.to_string().into());
			return Some(ResourceHandler::new(ResourceHandlerImpl::new(resource)));
		}
		None
	}
	fn get_raw(&self) -> *mut _cef_scheme_handler_factory_t {
		self.object.cast()
	}
}

impl Clone for SchemeHandlerFactoryImpl {
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
impl Rc for SchemeHandlerFactoryImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapSchemeHandlerFactory for SchemeHandlerFactoryImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_scheme_handler_factory_t, Self>) {
		self.object = object;
	}
}
