use crate::parsing::NodeFnAttributes;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use strum::{EnumString, VariantNames};
use syn::Error;
use syn::parse::{Parse, ParseStream};

pub const STD_FEATURE_GATE: &str = "std";

pub fn modify_cfg(attributes: &NodeFnAttributes) -> TokenStream {
	match (&attributes.cfg, &attributes.shader_node) {
		(Some(cfg), Some(_)) => quote!(#[cfg(all(#cfg, feature = #STD_FEATURE_GATE))]),
		(Some(cfg), None) => quote!(#[cfg(#cfg)]),
		(None, Some(_)) => quote!(#[cfg(feature = #STD_FEATURE_GATE)]),
		(None, None) => quote!(),
	}
}

#[derive(Debug, EnumString, VariantNames)]
pub(crate) enum ShaderNodeType {
	PerPixelAdjust,
}

impl Parse for ShaderNodeType {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ident: Ident = input.parse()?;
		Ok(match ident.to_string().as_str() {
			"PerPixelAdjust" => ShaderNodeType::PerPixelAdjust,
			_ => return Err(Error::new_spanned(&ident, format!("attr 'shader_node' must be one of {:?}", Self::VARIANTS))),
		})
	}
}
