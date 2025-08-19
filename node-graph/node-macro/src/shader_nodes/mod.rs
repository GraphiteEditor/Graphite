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

#[derive(Debug, Clone, VariantNames)]
pub(crate) enum ShaderNodeType {
	/// Marker for this node being a generated gpu node implementation, that should not emit anything to prevent
	/// recursively generating more gpu nodes. But it still counts as a gpu node and will get the
	/// `#[cfg(feature = "std")]` feature gate around it's impl.
	GpuNode,
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

pub trait ShaderCodegen {
	fn codegen(&self, parsed: &ParsedNodeFn, node_cfg: &TokenStream) -> syn::Result<ShaderTokens>;
}

impl ShaderCodegen for ShaderNodeType {
	fn codegen(&self, parsed: &ParsedNodeFn, node_cfg: &TokenStream) -> syn::Result<ShaderTokens> {
		match self {
			ShaderNodeType::GpuNode => (),
			_ => {
				if parsed.is_async {
					return Err(Error::new_spanned(&parsed.fn_name, "Shader nodes must not be async"));
				}
			}
		}

		match self {
			ShaderNodeType::GpuNode => Ok(ShaderTokens::default()),
			ShaderNodeType::PerPixelAdjust(x) => x.codegen(parsed, node_cfg),
		}
	}
}

#[derive(Clone, Default)]
pub struct ShaderTokens {
	pub shader_entry_point: TokenStream,
	pub gpu_node: TokenStream,
}
