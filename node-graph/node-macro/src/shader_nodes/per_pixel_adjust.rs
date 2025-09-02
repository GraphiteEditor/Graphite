use crate::parsing::{ParsedFieldType, ParsedNodeFn, RegularParsedField};
use crate::shader_nodes::CodegenShaderEntryPoint;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::borrow::Cow;
use syn::parse::{Parse, ParseStream};

#[derive(Debug)]
pub struct PerPixelAdjust {}

impl Parse for PerPixelAdjust {
	fn parse(_input: ParseStream) -> syn::Result<Self> {
		Ok(Self {})
	}
}

impl CodegenShaderEntryPoint for PerPixelAdjust {
	fn codegen_shader_entry_point(&self, parsed: &ParsedNodeFn) -> syn::Result<TokenStream> {
		let fn_name = &parsed.fn_name;
		let gpu_mod = format_ident!("{}_gpu", parsed.fn_name);
		let spirv_image_ty = quote!(Image2d);

		// bindings for images start at 1
		let mut binding_cnt = 0;
		let params = parsed
			.fields
			.iter()
			.map(|f| {
				let ident = &f.pat_ident;
				match &f.ty {
					ParsedFieldType::Node { .. } => Err(syn::Error::new_spanned(ident, "PerPixelAdjust shader nodes cannot accept other nodes as generics")),
					ParsedFieldType::Regular(RegularParsedField { gpu_image: false, ty, .. }) => Ok(Param {
						ident: Cow::Borrowed(&ident.ident),
						ty: Cow::Owned(ty.to_token_stream()),
						param_type: ParamType::Uniform,
					}),
					ParsedFieldType::Regular(RegularParsedField { gpu_image: true, .. }) => {
						binding_cnt += 1;
						Ok(Param {
							ident: Cow::Owned(format_ident!("image_{}", &ident.ident)),
							ty: Cow::Borrowed(&spirv_image_ty),
							param_type: ParamType::Image { binding: binding_cnt },
						})
					}
				}
			})
			.collect::<syn::Result<Vec<_>>>()?;

		let uniform_members = params
			.iter()
			.filter_map(|Param { ident, ty, param_type }| match param_type {
				ParamType::Image { .. } => None,
				ParamType::Uniform => Some(quote! {#ident: #ty}),
			})
			.collect::<Vec<_>>();
		let image_params = params
			.iter()
			.filter_map(|Param { ident, ty, param_type }| match param_type {
				ParamType::Image { binding } => Some(quote! {#[spirv(descriptor_set = 0, binding = #binding)] #ident: &#ty}),
				ParamType::Uniform => None,
			})
			.collect::<Vec<_>>();
		let call_args = params
			.iter()
			.map(|Param { ident, param_type, .. }| match param_type {
				ParamType::Image { .. } => quote!(Color::from_vec4(#ident.fetch_with(texel_coord, lod(0)))),
				ParamType::Uniform => quote!(uniform.#ident),
			})
			.collect::<Vec<_>>();
		let context = quote!(());

		Ok(quote! {
			pub mod #gpu_mod {
				use super::*;
				use graphene_core_shaders::color::Color;
				use spirv_std::spirv;
				use spirv_std::glam::{Vec4, Vec4Swizzles};
				use spirv_std::image::{Image2d, ImageWithMethods};
				use spirv_std::image::sample_with::lod;

				pub struct Uniform {
					#(#uniform_members),*
				}

				#[spirv(fragment)]
				pub fn entry_point(
					#[spirv(frag_coord)] frag_coord: Vec4,
					color_out: &mut Vec4,
					#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] uniform: &Uniform,
					#(#image_params),*
				) {
					let texel_coord = frag_coord.xy().as_uvec2();
					let color: Color = #fn_name(#context, #(#call_args),*);
					*color_out = color.to_vec4();
				}
			}
		})
	}
}

struct Param<'a> {
	ident: Cow<'a, Ident>,
	ty: Cow<'a, TokenStream>,
	param_type: ParamType,
}

enum ParamType {
	Image { binding: u32 },
	Uniform,
}
