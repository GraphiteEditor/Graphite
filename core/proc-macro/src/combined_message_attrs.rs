use crate::helpers::call_site_ident;
use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::Token;
use syn::{ItemEnum, TypePath};

struct MessageArgs {
	pub top_parent: TypePath,
	pub comma1: Token![,],
	pub parent: TypePath,
	pub comma2: Token![,],
	pub variant: Ident,
}

impl Parse for MessageArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			top_parent: input.parse()?,
			comma1: input.parse()?,
			parent: input.parse()?,
			comma2: input.parse()?,
			variant: input.parse()?,
		})
	}
}

struct TopLevelMessageArgs {
	pub parent: TypePath,
	pub comma2: Token![,],
	pub variant: Ident,
}

impl Parse for TopLevelMessageArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			parent: input.parse()?,
			comma2: input.parse()?,
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

	let parent_discriminant = {
		let mut res = parent.clone();
		let last_segment = &mut res.path.segments.last_mut().unwrap().ident;
		*last_segment = call_site_ident(format!("{}Discriminant", last_segment));
		res
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
		if let Some(attr) = var.attrs.iter_mut().find(|a| a.path.is_ident("child")) {
			let last_segment = attr.path.segments.last_mut().unwrap();
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
		if let Some(attr) = var.attrs.iter_mut().find(|a| a.path.is_ident("child")) {
			let last_segment = attr.path.segments.last_mut().unwrap();
			last_segment.ident = call_site_ident("sub_discriminant");
			var.attrs.push(syn::parse_quote! {
				#[discriminant_attr(child)]
			});
		}
	}

	let input_type = &input.ident;
	let discriminant = call_site_ident(format!("{}Discriminant", input_type));

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
