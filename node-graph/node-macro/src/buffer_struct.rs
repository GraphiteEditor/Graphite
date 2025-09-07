use crate::crate_ident::CrateIdent;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{Fields, GenericParam, Generics, Item, ItemEnum, ItemStruct, Meta, MetaList, Path, PathSegment, Result, Token, TypeParam, TypeParamBound, visit_mut};

pub fn derive_buffer_struct(crate_ident: &CrateIdent, content: proc_macro::TokenStream) -> Result<TokenStream> {
	let item = syn::parse::<Item>(content)?;
	match &item {
		Item::Enum(item) => derive_buffer_struct_enum(crate_ident, item),
		Item::Struct(item) => derive_buffer_struct_struct(crate_ident, item),
		_ => Err(syn::Error::new_spanned(&item, "Expected a struct or an enum")),
	}
}

pub fn derive_buffer_struct_enum(crate_ident: &CrateIdent, item: &ItemEnum) -> Result<TokenStream> {
	let gcore_shaders = crate_ident.gcore_shaders()?;
	let mod_buffer_struct = quote!(#gcore_shaders::shaders::buffer_struct);
	let reexport = quote!(#gcore_shaders::shaders::__private);

	if !item.generics.params.is_empty() {
		return Err(syn::Error::new_spanned(&item.generics, "enum must not have any generics"));
	}

	let enum_requirements_error = || {
		syn::Error::new(
			Span::call_site(),
			"deriving `BufferStruct` on an enum requires `#[repr(u32)]` and `#[derive(num_enum::FromPrimitive, num_enum::IntoPrimitive)]`",
		)
	};
	let repr_path = Path::from(format_ident!("repr"));
	let repr = item
		.attrs
		.iter()
		.filter_map(|a| match &a.meta {
			Meta::List(MetaList { path, tokens, .. }) if *path == repr_path => Some(tokens),
			_ => None,
		})
		.next()
		.ok_or_else(enum_requirements_error)?;

	let ident = &item.ident;
	Ok(quote! {
		unsafe impl #mod_buffer_struct::BufferStruct for #ident
		{
			type Buffer = #repr;

			fn write(from: Self) -> Self::Buffer {
				<#repr as From<Self>>::from(from)
			}

			fn read(from: Self::Buffer) -> Self {
				<Self as #reexport::num_enum::FromPrimitive>::from_primitive(from)
			}
		}
	})
}

/// see [`BufferStruct`] docs
///
/// This is also largely copied from my (@firestar99) project and adjusted
///
/// [`BufferStruct`]: `graphene_core_shaders::shaders::buffer_struct::BufferStruct`
pub fn derive_buffer_struct_struct(crate_ident: &CrateIdent, item: &ItemStruct) -> Result<TokenStream> {
	let gcore_shaders = crate_ident.gcore_shaders()?;
	let mod_buffer_struct = quote!(#gcore_shaders::shaders::buffer_struct);
	let reexport = quote!(#gcore_shaders::shaders::__private);

	let generics = item
		.generics
		.params
		.iter()
		.filter_map(|g| match g {
			GenericParam::Lifetime(_) => None,
			GenericParam::Type(t) => Some(t.ident.clone()),
			GenericParam::Const(c) => Some(c.ident.clone()),
		})
		.collect();

	let mut members_buffer = Punctuated::<TokenStream, Token![,]>::new();
	let mut write = Punctuated::<TokenStream, Token![,]>::new();
	let mut read = Punctuated::<TokenStream, Token![,]>::new();
	let mut gen_name_gen = GenericNameGen::new();
	let mut gen_ref_tys = Vec::new();
	let (members_buffer, write, read) = match &item.fields {
		Fields::Named(named) => {
			for f in &named.named {
				let name = f.ident.as_ref().unwrap();
				let mut ty = f.ty.clone();
				let mut visitor = GenericsVisitor::new(&item.ident, &generics);
				visit_mut::visit_type_mut(&mut visitor, &mut ty);
				if visitor.found_generics {
					gen_ref_tys.push(f.ty.clone());
					let gen_ident = gen_name_gen.next();
					members_buffer.push(quote!(#name: #gen_ident));
				} else {
					members_buffer.push(quote! {
						#name: <#ty as #mod_buffer_struct::BufferStruct>::Buffer
					});
				}

				write.push(quote! {
					#name: <#ty as #mod_buffer_struct::BufferStruct>::write(from.#name)
				});
				read.push(quote! {
					#name: <#ty as #mod_buffer_struct::BufferStruct>::read(from.#name)
				});
			}
			(quote!({#members_buffer}), quote!(Self::Buffer {#write}), quote!(Self {#read}))
		}
		Fields::Unnamed(unnamed) => {
			for (i, f) in unnamed.unnamed.iter().enumerate() {
				let mut ty = f.ty.clone();
				let mut visitor = GenericsVisitor::new(&item.ident, &generics);
				visit_mut::visit_type_mut(&mut visitor, &mut ty);
				if visitor.found_generics {
					gen_ref_tys.push(f.ty.clone());
					members_buffer.push(gen_name_gen.next().into_token_stream());
				} else {
					members_buffer.push(quote! {
						<#ty as #mod_buffer_struct::BufferStruct>::Buffer
					});
				}

				let index = syn::Index::from(i);
				write.push(quote! {
					<#ty as #mod_buffer_struct::BufferStruct>::write(from.#index)
				});
				read.push(quote! {
					<#ty as #mod_buffer_struct::BufferStruct>::read(from.#index)
				});
			}
			(quote!((#members_buffer);), quote!(Self::Buffer(#write)), quote!(Self(#read)))
		}
		Fields::Unit => (quote!(;), quote!(let _ = from; Self::Buffer {}), quote!(let _ = from; Self::Shader {})),
	};

	let generics_decl = &item.generics;
	let generics_ref = decl_to_ref(item.generics.params.iter());
	let generics_where = gen_ref_tys
		.iter()
		.map(|ty| quote!(#ty: #mod_buffer_struct::BufferStruct))
		.collect::<Punctuated<TokenStream, Token![,]>>()
		.into_token_stream();

	let generics_decl_any = gen_name_gen.decl(quote! {
		#reexport::bytemuck::Pod + Send + Sync
	});
	let generics_ref_buffer = gen_ref_tys
		.iter()
		.map(|ty| quote!(<#ty as #mod_buffer_struct::BufferStruct>::Buffer))
		.collect::<Punctuated<TokenStream, Token![,]>>()
		.into_token_stream();

	let vis = &item.vis;
	let ident = &item.ident;
	let buffer_ident = format_ident!("{}Buffer", ident);
	Ok(quote! {
		#[repr(C)]
		#[derive(Copy, Clone, #reexport::bytemuck::Zeroable, #reexport::bytemuck::Pod)]
		#vis struct #buffer_ident #generics_decl_any #members_buffer

		unsafe impl #generics_decl #mod_buffer_struct::BufferStruct for #ident #generics_ref
		where
			#ident #generics_ref: Copy,
			#generics_where
		{
			type Buffer = #buffer_ident <#generics_ref_buffer>;

			fn write(from: Self) -> Self::Buffer {
				#write
			}

			fn read(from: Self::Buffer) -> Self {
				#read
			}
		}
	})
}

struct GenericsVisitor<'a> {
	self_ident: &'a Ident,
	generics: &'a HashSet<Ident>,
	found_generics: bool,
}

impl<'a> GenericsVisitor<'a> {
	pub fn new(self_ident: &'a Ident, generics: &'a HashSet<Ident>) -> Self {
		Self {
			self_ident,
			generics,
			found_generics: false,
		}
	}
}

impl VisitMut for GenericsVisitor<'_> {
	fn visit_ident_mut(&mut self, i: &mut Ident) {
		if self.generics.contains(i) {
			self.found_generics = true;
		}
		visit_mut::visit_ident_mut(self, i);
	}

	fn visit_path_segment_mut(&mut self, i: &mut PathSegment) {
		if i.ident.to_string() == "Self" {
			i.ident = self.self_ident.clone();
		}
		visit_mut::visit_path_segment_mut(self, i);
	}
}

struct GenericNameGen(u32);

impl GenericNameGen {
	pub fn new() -> Self {
		Self(0)
	}

	pub fn next(&mut self) -> Ident {
		let i = self.0;
		self.0 += 1;
		format_ident!("T{}", i)
	}

	pub fn decl(self, ty: TokenStream) -> Generics {
		let params: Punctuated<GenericParam, Token![,]> = (0..self.0)
			.map(|i| {
				GenericParam::Type(TypeParam {
					attrs: Vec::new(),
					ident: format_ident!("T{}", i),
					colon_token: Some(Default::default()),
					bounds: Punctuated::from_iter([TypeParamBound::Verbatim(ty.clone())]),
					eq_token: None,
					default: None,
				})
			})
			.collect();
		if !params.is_empty() {
			Generics {
				lt_token: Some(Default::default()),
				params,
				gt_token: Some(Default::default()),
				where_clause: None,
			}
		} else {
			Generics::default()
		}
	}
}

fn decl_to_ref<'a>(generics: impl Iterator<Item = &'a GenericParam>) -> TokenStream {
	let out = generics
		.map(|generic| match generic {
			GenericParam::Lifetime(l) => l.lifetime.to_token_stream(),
			GenericParam::Type(t) => t.ident.to_token_stream(),
			GenericParam::Const(c) => c.ident.to_token_stream(),
		})
		.collect::<Punctuated<TokenStream, Token![,]>>();
	if out.is_empty() { TokenStream::new() } else { quote!(<#out>) }
}
