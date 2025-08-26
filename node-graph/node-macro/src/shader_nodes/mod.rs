use crate::crate_ident::CrateIdent;
use crate::parsing::{NodeFnAttributes, ParsedNodeFn};
use crate::shader_nodes::per_pixel_adjust::PerPixelAdjust;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use strum::VariantNames;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Token};

pub mod per_pixel_adjust;

pub const STD_FEATURE_GATE: &str = "std";
pub const SHADER_NODES_FEATURE_GATE: &str = "shader-nodes";

pub fn modify_cfg(attributes: &NodeFnAttributes) -> TokenStream {
	let feature_gate = match &attributes.shader_node {
		// shader node cfg is done on the mod
		Some(ShaderNodeType::ShaderNode) => quote!(),
		Some(_) => quote!(feature = #STD_FEATURE_GATE),
		None => quote!(),
	};
	let cfgs: Punctuated<_, Token![,]> = match &attributes.cfg {
		None => [&feature_gate].into_iter().collect(),
		Some(cfg) => [cfg, &feature_gate].into_iter().collect(),
	};
	quote!(#[cfg(all(#cfgs))])
}

#[derive(Debug, Clone, VariantNames)]
pub(crate) enum ShaderNodeType {
	/// Marker for this node being in a gpu node crate, but not having a gpu implementation. This is distinct from not
	/// declaring `shader_node` at all, as it will wrap the CPU node with a `#[cfg(feature = "std")]` feature gate.
	None,
	/// Marker for this node being a generated gpu node implementation, that should not emit anything to prevent
	/// recursively generating more gpu nodes. But it still counts as a gpu node and will get the
	/// `#[cfg(feature = "std")]` feature gate around it's impl.
	ShaderNode,
	PerPixelAdjust(PerPixelAdjust),
}

impl Parse for ShaderNodeType {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ident: Ident = input.parse()?;
		Ok(match ident.to_string().as_str() {
			"None" => ShaderNodeType::None,
			"PerPixelAdjust" => ShaderNodeType::PerPixelAdjust(PerPixelAdjust::parse(input)?),
			_ => return Err(Error::new_spanned(&ident, format!("attr 'shader_node' must be one of {:?}", Self::VARIANTS))),
		})
	}
}

pub trait ShaderCodegen {
	fn codegen(&self, crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<ShaderTokens>;
}

impl ShaderCodegen for ShaderNodeType {
	fn codegen(&self, crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<ShaderTokens> {
		match self {
			ShaderNodeType::None | ShaderNodeType::ShaderNode => (),
			_ => {
				if parsed.is_async {
					return Err(Error::new_spanned(&parsed.fn_name, "Shader nodes must not be async"));
				}
			}
		}

		match self {
			ShaderNodeType::None | ShaderNodeType::ShaderNode => Ok(ShaderTokens::default()),
			ShaderNodeType::PerPixelAdjust(x) => x.codegen(crate_ident, parsed),
		}
	}
}

#[derive(Clone, Default)]
pub struct ShaderTokens {
	pub shader_entry_point: TokenStream,
	pub gpu_node: TokenStream,
}
