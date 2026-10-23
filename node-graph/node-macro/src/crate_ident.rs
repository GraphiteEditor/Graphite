use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

pub struct CrateIdent {
	gcore: syn::Result<TokenStream>,
	gcore_shaders: syn::Result<TokenStream>,
	graphene_core: syn::Result<TokenStream>,
}

impl CrateIdent {
	pub fn gcore(&self) -> syn::Result<&TokenStream> {
		self.gcore.as_ref().map_err(Clone::clone)
	}

	pub fn gcore_shaders(&self) -> syn::Result<&TokenStream> {
		self.gcore_shaders.as_ref().map_err(Clone::clone)
	}

	pub fn graphene_core(&self) -> syn::Result<&TokenStream> {
		self.graphene_core.as_ref().map_err(Clone::clone)
	}
}

impl Default for CrateIdent {
	fn default() -> Self {
		let find_crate = |orig_name| match crate_name(orig_name) {
			Ok(FoundCrate::Itself) => Ok(quote!(crate)),
			Ok(FoundCrate::Name(name)) => {
				let name = format_ident!("{}", name);
				Ok(quote!(::#name))
			}
			Err(e) => Err(syn::Error::new(Span::call_site(), format!("Could not find dependency on `{orig_name}`:\n{e}"))),
		};

		let gcore = find_crate("core-types");
		let gcore_shaders = find_crate("no-std-types").or_else(|eshaders| gcore.clone().map_err(|ecore| syn::Error::new(Span::call_site(), format!("{ecore}\n\nFallback: {eshaders}"))));
		let graphene_core = find_crate("graphene-core").or_else(|ecore| {
			find_crate("graphene-std")
				.map(|gstd| quote!(#gstd::graphene_core))
				.map_err(|estd| syn::Error::new(Span::call_site(), format!("{ecore}\n\nFallback: {estd}")))
		});
		Self { gcore, gcore_shaders, graphene_core }
	}
}
