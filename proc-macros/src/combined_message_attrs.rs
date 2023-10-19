use crate::helpers::call_site_ident;
use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::Token;
use syn::{ItemEnum, TypePath};

struct MessageArgs {
	pub _top_parent: TypePath,
	pub _comma1: Token![,],
	pub parent: TypePath,
	pub _comma2: Token![,],
	pub variant: Ident,
}

impl Parse for MessageArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			_top_parent: input.parse()?,
			_comma1: input.parse()?,
			parent: input.parse()?,
			_comma2: input.parse()?,
			variant: input.parse()?,
		})
	}
}

struct TopLevelMessageArgs {
	pub parent: TypePath,
	pub _comma2: Token![,],
	pub variant: Ident,
}

impl Parse for TopLevelMessageArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			parent: input.parse()?,
			_comma2: input.parse()?,
			variant: input.parse()?,
		})
	}
}

pub fn combined_message_attrs_impl(attr: TokenStream, input_item: TokenStream) -> syn::Result<TokenStream> {
	if attr.is_empty() {
		return top_level_impl(input_item);
	}

	let mut input = syn::parse2::<ItemEnum>(input_item)?;

	let (parent_is_top, parent, variant) = match syn::parse2::<MessageArgs>(attr.clone()) {
		Ok(x) => (false, x.parent, x.variant),
		Err(_) => {
			let x = syn::parse2::<TopLevelMessageArgs>(attr)?;
			(true, x.parent, x.variant)
		}
	};

	let parent_discriminant = quote::quote! {
		<#parent as ToDiscriminant>::Discriminant
	};

	input.attrs.push(syn::parse_quote! { #[derive(ToDiscriminant, TransitiveChild)] });
	input.attrs.push(syn::parse_quote! { #[parent(#parent, #parent::#variant)] });
	if parent_is_top {
		input.attrs.push(syn::parse_quote! { #[parent_is_top] });
	}
	input
		.attrs
		.push(syn::parse_quote! { #[discriminant_attr(derive(Debug, Copy, Clone, PartialEq, Eq, Hash, AsMessage, TransitiveChild))] });
	input
		.attrs
		.push(syn::parse_quote! { #[discriminant_attr(parent(#parent_discriminant, #parent_discriminant::#variant))] });
	if parent_is_top {
		input.attrs.push(syn::parse_quote! { #[discriminant_attr(parent_is_top)] });
	}

	for var in &mut input.variants {
		if let Some(attr) = var.attrs.iter_mut().find(|a| a.path().is_ident("child")) {
			let path = match &mut attr.meta {
				syn::Meta::Path(path) => path,
				syn::Meta::List(list) => &mut list.path,
				syn::Meta::NameValue(named_value) => &mut named_value.path,
			};
			let last_segment = path.segments.last_mut().unwrap();
			last_segment.ident = call_site_ident("sub_discriminant");
			var.attrs.push(syn::parse_quote! {
				#[discriminant_attr(child)]
			});
		}
	}

	Ok(input.into_token_stream())
}

fn top_level_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let mut input = syn::parse2::<ItemEnum>(input_item)?;

	input.attrs.push(syn::parse_quote! { #[derive(ToDiscriminant)] });
	input.attrs.push(syn::parse_quote! { #[discriminant_attr(derive(Debug, Copy, Clone, PartialEq, Eq, Hash, AsMessage))] });

	for var in &mut input.variants {
		if let Some(attr) = var.attrs.iter_mut().find(|a| a.path().is_ident("child")) {
			let path = match &mut attr.meta {
				syn::Meta::Path(path) => path,
				syn::Meta::List(list) => &mut list.path,
				syn::Meta::NameValue(named_value) => &mut named_value.path,
			};
			let last_segment = path.segments.last_mut().unwrap();
			last_segment.ident = call_site_ident("sub_discriminant");
			var.attrs.push(syn::parse_quote! {
				#[discriminant_attr(child)]
			});
		}
	}

	let input_type = &input.ident;
	let discriminant = call_site_ident(format!("{input_type}Discriminant"));

	Ok(quote::quote! {
		#input

		impl TransitiveChild for #input_type {
			type TopParent = Self;
			type Parent = Self;
		}

		impl TransitiveChild for #discriminant {
			type TopParent = Self;
			type Parent = Self;
		}
	})
}
