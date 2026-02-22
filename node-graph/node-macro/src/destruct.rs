use convert_case::{Case, Casing};
use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error, LitStr, Meta, Type, spanned::Spanned};

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream2> {
	let struct_name = input.ident;
	let generics = input.generics;
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let Data::Struct(data_struct) = input.data else {
		return Err(Error::new(
			Span::call_site(),
			"Deriving `Destruct` is currently only supported for structs",
		));
	};

	let graphene_core = match proc_macro_crate::crate_name("graphene-core").map_err(|e| {
		Error::new(
			Span::call_site(),
			format!("Failed to find location of graphene_core. Make sure it is imported as a dependency: {e}"),
		)
	})? {
		FoundCrate::Itself => quote!(crate),
		FoundCrate::Name(name) => {
			let ident = syn::Ident::new(&name, Span::call_site());
			quote!(#ident)
		}
	};

	let mut node_implementations = Vec::with_capacity(data_struct.fields.len());
	let mut output_fields = Vec::with_capacity(data_struct.fields.len());

	for field in data_struct.fields {
		let Some(field_name) = field.ident else {
			return Err(Error::new(field.span(), "Destruct cannot be used on tuple structs"));
		};

		let ty = field.ty;
		let output_name = parse_output_name(&field.attrs)?.unwrap_or_else(|| field_name.to_string().to_case(Case::Title));
		let output_name_lit = LitStr::new(&output_name, field_name.span());

		let fn_name = format_ident!("extract_{}_{}", struct_name.to_string().to_case(Case::Snake), field_name);
		let node_struct_name = format_ident!("{}Node", fn_name.to_string().to_case(Case::Pascal));

		node_implementations.push(generate_extractor_node(&graphene_core, &fn_name, &struct_name, &field_name, &ty, &output_name_lit));
		output_fields.push(quote! {
			#graphene_core::registry::StructField {
				name: #output_name_lit,
				node_path: concat!(std::module_path!().rsplit_once("::").unwrap().0, "::", stringify!(#node_struct_name)),
				ty: #graphene_core::concrete!(#ty),
			}
		});
	}

	Ok(quote! {
		#(#node_implementations)*

		impl #impl_generics #graphene_core::registry::Destruct for #struct_name #ty_generics #where_clause {
			fn fields() -> &'static [#graphene_core::registry::StructField] {
				&[
					#(#output_fields,)*
				]
			}
		}
	})
}

fn generate_extractor_node(
	graphene_core: &TokenStream2,
	fn_name: &syn::Ident,
	struct_name: &syn::Ident,
	field_name: &syn::Ident,
	ty: &Type,
	output_name: &LitStr,
) -> TokenStream2 {
	quote! {
		#[node_macro::node(category(""), name(#output_name))]
		fn #fn_name(_: impl #graphene_core::Ctx, data: #struct_name) -> #ty {
			data.#field_name
		}
	}
}

fn parse_output_name(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
	let mut output_name = None;

	for attr in attrs {
		if !attr.path().is_ident("output") {
			continue;
		}

		let mut this_output_name = None;
		match &attr.meta {
			Meta::Path(_) => {
				return Err(Error::new_spanned(attr, "Expected output metadata like #[output(name = \"Result\")]"));
			}
			Meta::NameValue(_) => {
				return Err(Error::new_spanned(attr, "Expected output metadata like #[output(name = \"Result\")]"));
			}
			Meta::List(_) => {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("name") {
						if this_output_name.is_some() {
							return Err(meta.error("Multiple output names provided for one field"));
						}
						let value = meta.value()?;
						let lit: LitStr = value.parse()?;
						this_output_name = Some(lit.value());
						Ok(())
					} else {
						Err(meta.error("Unsupported output metadata. Supported syntax is #[output(name = \"...\")]"))
					}
				})?;
			}
		}

		let this_output_name = this_output_name.ok_or_else(|| Error::new_spanned(attr, "Missing output name. Use #[output(name = \"...\")]"))?;
		if output_name.is_some() {
			return Err(Error::new_spanned(attr, "Multiple #[output(...)] attributes are not allowed on one field"));
		}
		output_name = Some(this_output_name);
	}

	Ok(output_name)
}
