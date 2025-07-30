use crate::parsing::{NodeFnAttributes, ParsedNodeFn};
use crate::shader_nodes::per_pixel_adjust::PerPixelAdjust;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use strum::VariantNames;
use syn::Error;
use syn::parse::{Parse, ParseStream};

pub mod per_pixel_adjust;

pub const STD_FEATURE_GATE: &str = "std";

pub fn modify_cfg(attributes: &NodeFnAttributes) -> TokenStream {
	match (&attributes.cfg, &attributes.shader_node) {
		(Some(cfg), Some(_)) => quote!(#[cfg(all(#cfg, feature = #STD_FEATURE_GATE))]),
		(Some(cfg), None) => quote!(#[cfg(#cfg)]),
		(None, Some(_)) => quote!(#[cfg(feature = #STD_FEATURE_GATE)]),
		(None, None) => quote!(),
	}
}

#[derive(Debug, VariantNames)]
pub(crate) enum ShaderNodeType {
	PerPixelAdjust(PerPixelAdjust),
}

impl Parse for ShaderNodeType {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ident: Ident = input.parse()?;
		Ok(match ident.to_string().as_str() {
			"PerPixelAdjust" => ShaderNodeType::PerPixelAdjust(PerPixelAdjust::parse(input)?),
			_ => return Err(Error::new_spanned(&ident, format!("attr 'shader_node' must be one of {:?}", Self::VARIANTS))),
		})
	}
}

pub trait CodegenShaderEntryPoint {
	fn codegen_shader_entry_point(&self, parsed: &ParsedNodeFn) -> syn::Result<TokenStream>;
}

impl CodegenShaderEntryPoint for ShaderNodeType {
	fn codegen_shader_entry_point(&self, parsed: &ParsedNodeFn) -> syn::Result<TokenStream> {
		if parsed.is_async {
			return Err(Error::new_spanned(&parsed.fn_name, "Shader nodes must not be async"));
		}

		match self {
			ShaderNodeType::PerPixelAdjust(x) => x.codegen_shader_entry_point(parsed),
		}
	}
}
