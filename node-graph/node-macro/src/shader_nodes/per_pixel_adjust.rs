use crate::parsing::{Input, NodeFnAttributes, ParsedField, ParsedFieldType, ParsedNodeFn, RegularParsedField};
use crate::shader_nodes::{ShaderCodegen, ShaderNodeType, ShaderTokens};
use convert_case::{Case, Casing};
use proc_macro_crate::FoundCrate;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::borrow::Cow;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{PatIdent, Type, parse_quote};

#[derive(Debug, Clone)]
pub struct PerPixelAdjust {}

impl Parse for PerPixelAdjust {
	fn parse(_input: ParseStream) -> syn::Result<Self> {
		Ok(Self {})
	}
}

impl ShaderCodegen for PerPixelAdjust {
	fn codegen(&self, parsed: &ParsedNodeFn, node_cfg: &TokenStream) -> syn::Result<ShaderTokens> {
		let (shader_entry_point, entry_point_name) = self.codegen_shader_entry_point(parsed)?;
		let gpu_node = self.codegen_gpu_node(parsed, node_cfg, &entry_point_name)?;
		Ok(ShaderTokens { shader_entry_point, gpu_node })
	}
}

impl PerPixelAdjust {
	fn codegen_shader_entry_point(&self, parsed: &ParsedNodeFn) -> syn::Result<(TokenStream, TokenStream)> {
		let fn_name = &parsed.fn_name;
		let gpu_mod = format_ident!("{}_gpu_entry_point", fn_name);
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

		let entry_point_name = format_ident!("ENTRY_POINT_NAME");
		let entry_point_sym = quote!(#gpu_mod::#entry_point_name);

		let shader_entry_point = quote! {
			pub mod #gpu_mod {
				use super::*;
				use graphene_core_shaders::color::Color;
				use spirv_std::spirv;
				use spirv_std::glam::{Vec4, Vec4Swizzles};
				use spirv_std::image::{Image2d, ImageWithMethods};
				use spirv_std::image::sample_with::lod;

				pub const #entry_point_name: &str = core::concat!(core::module_path!(), "::entry_point");

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
		};
		Ok((shader_entry_point, entry_point_sym))
	}

	fn codegen_gpu_node(&self, parsed: &ParsedNodeFn, node_cfg: &TokenStream, entry_point_name: &TokenStream) -> syn::Result<TokenStream> {
		let fn_name = format_ident!("{}_gpu", parsed.fn_name);
		let struct_name = format_ident!("{}", fn_name.to_string().to_case(Case::Pascal));
		let mod_name = fn_name.clone();

		let gcore = match &parsed.crate_name {
			FoundCrate::Itself => format_ident!("crate"),
			FoundCrate::Name(name) => format_ident!("{name}"),
		};
		let raster_gpu: Type = parse_quote!(#gcore::table::Table<#gcore::raster_types::Raster<#gcore::raster_types::GPU>>);

		// adapt fields for gpu node
		let mut fields = parsed
			.fields
			.iter()
			.map(|f| match &f.ty {
				ParsedFieldType::Regular(reg @ RegularParsedField { gpu_image: true, .. }) => Ok(ParsedField {
					ty: ParsedFieldType::Regular(RegularParsedField {
						ty: raster_gpu.clone(),
						implementations: Punctuated::default(),
						..reg.clone()
					}),
					..f.clone()
				}),
				ParsedFieldType::Regular(RegularParsedField { gpu_image: false, .. }) => Ok(f.clone()),
				ParsedFieldType::Node { .. } => Err(syn::Error::new_spanned(&f.pat_ident, "PerPixelAdjust shader nodes cannot accept other nodes as generics")),
			})
			.collect::<syn::Result<Vec<_>>>()?;

		// wgpu_executor field
		let wgpu_executor = format_ident!("__wgpu_executor");
		fields.push(ParsedField {
			pat_ident: PatIdent {
				attrs: vec![],
				by_ref: None,
				mutability: None,
				ident: parse_quote!(#wgpu_executor),
				subpat: None,
			},
			name: None,
			description: "".to_string(),
			widget_override: Default::default(),
			ty: ParsedFieldType::Regular(RegularParsedField {
				ty: parse_quote!(&'a WgpuExecutor),
				exposed: false,
				value_source: Default::default(),
				number_soft_min: None,
				number_soft_max: None,
				number_hard_min: None,
				number_hard_max: None,
				number_mode_range: None,
				implementations: Default::default(),
				gpu_image: false,
			}),
			number_display_decimal_places: None,
			number_step: None,
			unit: None,
		});

		// exactly one gpu_image field, may be expanded later
		let gpu_image_field = {
			let mut iter = fields.iter().filter(|f| matches!(f.ty, ParsedFieldType::Regular(RegularParsedField { gpu_image: true, .. })));
			match (iter.next(), iter.next()) {
				(Some(v), None) => Ok(v),
				(Some(_), Some(more)) => Err(syn::Error::new_spanned(&more.pat_ident, "No more than one parameter must be annotated with `#[gpu_image]`")),
				(None, _) => Err(syn::Error::new_spanned(&parsed.fn_name, "At least one parameter must be annotated with `#[gpu_image]`")),
			}?
		};
		let gpu_image = &gpu_image_field.pat_ident.ident;

		let body = quote! {
			{
				#wgpu_executor.shader_runtime.run_per_pixel_adjust(&::wgpu_executor::shader_runtime::Shaders {
					wgsl_shader: crate::WGSL_SHADER,
					fragment_shader_name: super::#entry_point_name,
				}, #gpu_image, &1u32).await
			}
		};

		let mut parsed_node_fn = ParsedNodeFn {
			vis: parsed.vis.clone(),
			attributes: NodeFnAttributes {
				shader_node: Some(ShaderNodeType::GpuNode),
				..parsed.attributes.clone()
			},
			fn_name,
			struct_name,
			mod_name: mod_name.clone(),
			fn_generics: vec![parse_quote!('a: 'n)],
			where_clause: None,
			input: Input {
				pat_ident: parsed.input.pat_ident.clone(),
				ty: parse_quote!(impl #gcore::context::Ctx),
				implementations: Default::default(),
			},
			output_type: raster_gpu,
			is_async: true,
			fields,
			body,
			crate_name: parsed.crate_name.clone(),
			description: "".to_string(),
		};
		parsed_node_fn.replace_impl_trait_in_input();
		let gpu_node = crate::codegen::generate_node_code(&parsed_node_fn)?;

		Ok(quote! {
			#node_cfg
			mod #mod_name {
				use super::*;
				use wgpu_executor::WgpuExecutor;

				#gpu_node
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
