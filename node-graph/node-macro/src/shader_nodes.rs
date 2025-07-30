use crate::parsing::NodeFnAttributes;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, TokenStreamExt, quote};
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
	PerPixelAdjust(PerPixelAdjust),
}

impl Parse for ShaderNodeType {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ident: Ident = input.parse()?;
		Ok(match ident.to_string().as_str() {
			"PerPixelAdjust" => ShaderNodeType::PerPixelAdjust(crate::shader_nodes::PerPixelAdjust::parse(input)?),
			_ => return Err(Error::new_spanned(&ident, format!("attr 'shader_node' must be one of {:?}", Self::VARIANTS))),
		})
	}
}

impl ShaderNodeType {
	pub fn codegen_shader_entry_point(&self) -> TokenStream {
		match self {
			ShaderNodeType::PerPixelAdjust(x) => x.codegen_shader_entry_point(),
		}
	}
}

#[derive(Debug)]
pub struct PerPixelAdjust {}

impl Parse for PerPixelAdjust {
	fn parse(_input: ParseStream) -> syn::Result<Self> {
		Ok(Self {})
	}
}

impl PerPixelAdjust {
	pub fn codegen_shader_entry_point(&self) -> TokenStream {
		quote! {}
	}
}
