use crate::crate_ident::CrateIdent;
use crate::parsing::{Input, NodeFnAttributes, ParsedField, ParsedFieldType, ParsedNodeFn, ParsedValueSource, RegularParsedField};
use crate::shader_nodes::{SHADER_NODES_FEATURE_GATE, ShaderCodegen, ShaderNodeType, ShaderTokens};
use convert_case::{Case, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::borrow::Cow;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{LitStr, PatIdent, Type, parse_quote};

#[derive(Debug, Clone)]
pub struct PerPixelAdjust {}

impl Parse for PerPixelAdjust {
	fn parse(_input: ParseStream) -> syn::Result<Self> {
		Ok(Self {})
	}
}

impl ShaderCodegen for PerPixelAdjust {
	fn codegen(&self, crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<ShaderTokens> {
		let fn_name = &parsed.fn_name;

		let mut params;
		let has_uniform;
		{
			// categorize params
			params = parsed
				.fields
				.iter()
				.map(|f| {
					let ident = &f.pat_ident;
					match &f.ty {
						ParsedFieldType::Node { .. } => Err(syn::Error::new_spanned(ident, "PerPixelAdjust shader nodes cannot accept other nodes as generics")),
						ParsedFieldType::Regular(RegularParsedField { gpu_image: false, ty, .. }) => Ok(Param {
							ident: Cow::Borrowed(&ident.ident),
							ty: ty.to_token_stream(),
							param_type: ParamType::Uniform,
						}),
						ParsedFieldType::Regular(RegularParsedField { gpu_image: true, .. }) => {
							let param = Param {
								ident: Cow::Owned(format_ident!("image_{}", &ident.ident)),
								ty: quote!(Image2d),
								param_type: ParamType::Image { binding: 0 },
							};
							Ok(param)
						}
					}
				})
				.collect::<syn::Result<Vec<_>>>()?;

			has_uniform = params.iter().any(|p| matches!(p.param_type, ParamType::Uniform));

			// assign image bindings
			// if an arg_buffer exists, bindings for images start at 1 to leave 0 for arg buffer
			let mut binding_cnt = if has_uniform { 1 } else { 0 };
			for p in params.iter_mut() {
				match &mut p.param_type {
					ParamType::Image { binding } => {
						*binding = binding_cnt;
						binding_cnt += 1;
					}
					ParamType::Uniform => {}
				}
			}
		}

		let entry_point_mod = format_ident!("{}_gpu_entry_point", fn_name);
		let entry_point_name_ident = format_ident!("ENTRY_POINT_NAME");
		let entry_point_name = quote!(#entry_point_mod::#entry_point_name_ident);
		let uniform_struct_ident = format_ident!("Uniform");
		let uniform_struct = quote!(#entry_point_mod::#uniform_struct_ident);
		let shader_node_mod = format_ident!("{}_shader_node", fn_name);

		let codegen = PerPixelAdjustCodegen {
			crate_ident,
			parsed,
			params,
			has_uniform,
			entry_point_mod,
			entry_point_name_ident,
			entry_point_name,
			uniform_struct_ident,
			uniform_struct,
			shader_node_mod,
		};

		Ok(ShaderTokens {
			shader_entry_point: codegen.codegen_shader_entry_point()?,
			gpu_node: codegen.codegen_gpu_node()?,
		})
	}
}

pub struct PerPixelAdjustCodegen<'a> {
	crate_ident: &'a CrateIdent,
	parsed: &'a ParsedNodeFn,
	params: Vec<Param<'a>>,
	has_uniform: bool,
	entry_point_mod: Ident,
	entry_point_name_ident: Ident,
	entry_point_name: TokenStream,
	uniform_struct_ident: Ident,
	uniform_struct: TokenStream,
	shader_node_mod: Ident,
}

impl PerPixelAdjustCodegen<'_> {
	fn codegen_shader_entry_point(&self) -> syn::Result<TokenStream> {
		let fn_name = &self.parsed.fn_name;
		let gcore_shaders = self.crate_ident.gcore_shaders()?;
		let reexport = quote!(#gcore_shaders::shaders::__private);

		let uniform_members = self
			.params
			.iter()
			.filter_map(|Param { ident, ty, param_type }| match param_type {
				ParamType::Image { .. } => None,
				ParamType::Uniform => Some(quote! {#ident: #ty}),
			})
			.collect::<Vec<_>>();
		let uniform_struct_ident = &self.uniform_struct_ident;
		let uniform_struct = parse_quote! {
			#[repr(C)]
			#[derive(Copy, Clone)]
			pub struct #uniform_struct_ident {
				#(pub #uniform_members),*
			}
		};
		let uniform_struct_shader_struct_derive = crate::buffer_struct::derive_buffer_struct_struct(self.crate_ident, &uniform_struct)?;

		let image_params = self
			.params
			.iter()
			.filter_map(|Param { ident, ty, param_type }| match param_type {
				ParamType::Image { binding } => Some(quote! {#[spirv(descriptor_set = 0, binding = #binding)] #ident: &#ty}),
				ParamType::Uniform => None,
			})
			.collect::<Vec<_>>();
		let call_args = self
			.params
			.iter()
			.map(|Param { ident, param_type, .. }| match param_type {
				ParamType::Image { .. } => quote!(Color::from_vec4(#ident.fetch_with(texel_coord, lod(0)))),
				ParamType::Uniform => quote!(uniform.#ident),
			})
			.collect::<Vec<_>>();
		let context = quote!(());

		let entry_point_mod = &self.entry_point_mod;
		let entry_point_name = &self.entry_point_name_ident;
		Ok(quote! {
			pub mod #entry_point_mod {
				use super::*;
				use #gcore_shaders::color::Color;
				use #reexport::glam::{Vec4, Vec4Swizzles};
				use #reexport::spirv_std::spirv;
				use #reexport::spirv_std::image::{Image2d, ImageWithMethods};
				use #reexport::spirv_std::image::sample_with::lod;

				pub const #entry_point_name: &str = core::concat!(core::module_path!(), "::entry_point");

				#uniform_struct
				#uniform_struct_shader_struct_derive

				#[spirv(fragment)]
				pub fn entry_point(
					#[spirv(frag_coord)] frag_coord: Vec4,
					color_out: &mut Vec4,
					#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] uniform: &UniformBuffer,
					#(#image_params),*
				) {
					let uniform = <Uniform as #gcore_shaders::shaders::buffer_struct::BufferStruct>::read(*uniform);
					let texel_coord = frag_coord.xy().as_uvec2();
					let color: Color = #fn_name(#context, #(#call_args),*);
					*color_out = color.to_vec4();
				}
			}
		})
	}

	fn codegen_gpu_node(&self) -> syn::Result<TokenStream> {
		let gcore = self.crate_ident.gcore()?;
		let wgpu_executor = self.crate_ident.wgpu_executor()?;

		// adapt fields for gpu node
		let raster_gpu: Type = parse_quote!(#gcore::table::Table<#gcore::raster_types::Raster<#gcore::raster_types::GPU>>);
		let mut fields = self
			.parsed
			.fields
			.iter()
			.map(|f| match &f.ty {
				ParsedFieldType::Regular(reg @ RegularParsedField { gpu_image: true, .. }) => Ok(ParsedField {
					pat_ident: PatIdent {
						mutability: None,
						by_ref: None,
						..f.pat_ident.clone()
					},
					ty: ParsedFieldType::Regular(RegularParsedField {
						ty: raster_gpu.clone(),
						implementations: Punctuated::default(),
						..reg.clone()
					}),
					..f.clone()
				}),
				ParsedFieldType::Regular(RegularParsedField { gpu_image: false, .. }) => Ok(ParsedField {
					pat_ident: PatIdent {
						mutability: None,
						by_ref: None,
						..f.pat_ident.clone()
					},
					..f.clone()
				}),
				ParsedFieldType::Node { .. } => Err(syn::Error::new_spanned(&f.pat_ident, "PerPixelAdjust shader nodes cannot accept other nodes as generics")),
			})
			.collect::<syn::Result<Vec<_>>>()?;

		// insert wgpu_executor field
		let executor = format_ident!("__wgpu_executor");
		fields.push(ParsedField {
			pat_ident: PatIdent {
				attrs: vec![],
				by_ref: None,
				mutability: None,
				ident: parse_quote!(#executor),
				subpat: None,
			},
			name: None,
			description: "".to_string(),
			widget_override: Default::default(),
			ty: ParsedFieldType::Regular(RegularParsedField {
				ty: parse_quote!(&'a WgpuExecutor),
				exposed: true,
				value_source: ParsedValueSource::Scope(LitStr::new("wgpu-executor", Span::call_site())),
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

		// find exactly one gpu_image field, runtime doesn't support more than 1 atm
		let gpu_image_field = {
			let mut iter = fields.iter().filter(|f| matches!(f.ty, ParsedFieldType::Regular(RegularParsedField { gpu_image: true, .. })));
			match (iter.next(), iter.next()) {
				(Some(v), None) => Ok(v),
				(Some(_), Some(more)) => Err(syn::Error::new_spanned(&more.pat_ident, "No more than one parameter must be annotated with `#[gpu_image]`")),
				(None, _) => Err(syn::Error::new_spanned(&self.parsed.fn_name, "At least one parameter must be annotated with `#[gpu_image]`")),
			}?
		};
		let gpu_image = &gpu_image_field.pat_ident.ident;

		// uniform buffer struct construction
		let has_uniform = self.has_uniform;
		let uniform_buffer = if has_uniform {
			let uniform_struct = &self.uniform_struct;
			let uniform_members = self
				.params
				.iter()
				.filter_map(|p| match p.param_type {
					ParamType::Image { .. } => None,
					ParamType::Uniform => Some(p.ident.as_ref()),
				})
				.collect::<Vec<_>>();
			quote!(Some(&super::#uniform_struct {
				#(#uniform_members),*
			}))
		} else {
			// explicit generics placed here cause it's easier than explicitly writing `run_per_pixel_adjust::<()>`
			quote!(Option::<&()>::None)
		};

		// node function body
		let entry_point_name = &self.entry_point_name;
		let body = quote! {
			{
				#executor.shader_runtime.run_per_pixel_adjust(&::wgpu_executor::shader_runtime::per_pixel_adjust_runtime::Shaders {
					wgsl_shader: crate::WGSL_SHADER,
					fragment_shader_name: super::#entry_point_name,
					has_uniform: #has_uniform,
				}, #gpu_image, #uniform_buffer).await
			}
		};

		// call node codegen
		let display_name = self.parsed.attributes.display_name.clone();
		let display_name = display_name.unwrap_or_else(|| LitStr::new(&self.shader_node_mod.to_string().strip_suffix("_shader_node").unwrap().to_case(Case::Title), Span::call_site()));
		let display_name = LitStr::new(&format!("{} GPU", display_name.value()), display_name.span());
		let mut parsed_node_fn = ParsedNodeFn {
			vis: self.parsed.vis.clone(),
			attributes: NodeFnAttributes {
				display_name: Some(display_name),
				shader_node: Some(ShaderNodeType::ShaderNode),
				..self.parsed.attributes.clone()
			},
			fn_name: self.shader_node_mod.clone(),
			struct_name: format_ident!("{}", self.shader_node_mod.to_string().to_case(Case::Pascal)),
			mod_name: self.shader_node_mod.clone(),
			fn_generics: vec![parse_quote!('a: 'n)],
			where_clause: None,
			input: Input {
				pat_ident: self.parsed.input.pat_ident.clone(),
				ty: parse_quote!(impl #gcore::context::Ctx),
				implementations: Default::default(),
				context_features: self.parsed.input.context_features.clone(),
			},
			output_type: raster_gpu,
			is_async: true,
			fields,
			body,
			description: self.parsed.description.clone(),
		};
		parsed_node_fn.replace_impl_trait_in_input();
		let gpu_node_impl = crate::codegen::generate_node_code(self.crate_ident, &parsed_node_fn)?;

		// wrap node in `mod #gpu_node_mod`
		let shader_node_mod = &self.shader_node_mod;
		Ok(quote! {
			#[cfg(feature = #SHADER_NODES_FEATURE_GATE)]
			mod #shader_node_mod {
				use super::*;
				use #wgpu_executor::WgpuExecutor;

				#gpu_node_impl
			}
		})
	}
}

struct Param<'a> {
	ident: Cow<'a, Ident>,
	ty: TokenStream,
	param_type: ParamType,
}

enum ParamType {
	Image { binding: u32 },
	Uniform,
}
