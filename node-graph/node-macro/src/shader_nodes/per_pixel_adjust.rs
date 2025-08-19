use crate::parsing::{Input, NodeFnAttributes, ParsedField, ParsedFieldType, ParsedNodeFn, RegularParsedField};
use crate::shader_nodes::{CodegenShaderEntryPoint, ShaderNodeType};
use convert_case::{Case, Casing};
use proc_macro_crate::FoundCrate;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::borrow::Cow;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Path, Token, TraitBound, TraitBoundModifier, Type, TypeImplTrait, TypeParamBound};

#[derive(Debug, Clone)]
pub struct PerPixelAdjust {}

impl Parse for PerPixelAdjust {
	fn parse(_input: ParseStream) -> syn::Result<Self> {
		Ok(Self {})
	}
}

impl CodegenShaderEntryPoint for PerPixelAdjust {
	fn codegen_shader_entry_point(&self, parsed: &ParsedNodeFn) -> syn::Result<TokenStream> {
		let fn_name = &parsed.fn_name;
		let gpu_mod = format_ident!("{}_gpu_entry_point", parsed.fn_name);
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

	fn codegen_gpu_node(&self, parsed: &ParsedNodeFn) -> syn::Result<TokenStream> {
		let fn_name = format_ident!("{}_gpu", parsed.fn_name);
		let struct_name = format_ident!("{}", fn_name.to_string().to_case(Case::Pascal));
		let mod_name = fn_name.clone();

		let gcore = match &parsed.crate_name {
			FoundCrate::Itself => format_ident!("crate"),
			FoundCrate::Name(name) => format_ident!("{name}"),
		};
		let raster_gpu = syn::parse2::<Type>(quote!(#gcore::table::Table<#gcore::raster_types::Raster<#gcore::raster_types::GPU>>))?;

		let fields = parsed
			.fields
			.iter()
			.map(|f| match &f.ty {
				ParsedFieldType::Regular(reg @ RegularParsedField { gpu_image: true, .. }) => Ok(ParsedField {
					ty: ParsedFieldType::Regular(RegularParsedField {
						ty: raster_gpu.clone(),
						..reg.clone()
					}),
					..f.clone()
				}),
				ParsedFieldType::Regular(RegularParsedField { gpu_image: false, .. }) => Ok(f.clone()),
				ParsedFieldType::Node { .. } => Err(syn::Error::new_spanned(&f.pat_ident, "PerPixelAdjust shader nodes cannot accept other nodes as generics")),
			})
			.collect::<syn::Result<_>>()?;
		let body = quote! {
			{

			}
		};

		crate::codegen::generate_node_code(&ParsedNodeFn {
			vis: parsed.vis.clone(),
			attributes: NodeFnAttributes {
				shader_node: Some(ShaderNodeType::GpuNode),
				..parsed.attributes.clone()
			},
			fn_name,
			struct_name,
			mod_name,
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: parsed.input.pat_ident.clone(),
				ty: Type::ImplTrait(TypeImplTrait {
					impl_token: Token![impl](Span::call_site()),
					bounds: Punctuated::from_iter([TypeParamBound::Trait(TraitBound {
						paren_token: None,
						modifier: TraitBoundModifier::None,
						lifetimes: None,
						path: Path::from(format_ident!("Ctx")),
					})]),
				}),
				implementations: Default::default(),
			},
			output_type: raster_gpu,
			is_async: true,
			fields,
			body,
			crate_name: parsed.crate_name.clone(),
			description: "".to_string(),
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
