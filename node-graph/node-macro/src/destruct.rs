use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Error, Ident, spanned::Spanned};

pub fn derive(struct_name: Ident, data: syn::Data) -> syn::Result<TokenStream2> {
	let syn::Data::Struct(data_struct) = data else {
		return Err(Error::new(proc_macro2::Span::call_site(), String::from("Deriving `Destruct` is currently only supported for structs")));
	};

	let found_crate = proc_macro_crate::crate_name("graphene-core").map_err(|e| {
		Error::new(
			proc_macro2::Span::call_site(),
			format!("Failed to find location of graphene_core. Make sure it is imported as a dependency: {}", e),
		)
	})?;

	let crate_name = match found_crate {
		proc_macro_crate::FoundCrate::Itself => quote!(crate),
		proc_macro_crate::FoundCrate::Name(name) => {
			let ident = format_ident!("{}", name);
			quote!(#ident)
		}
	};

	let path = quote!(std::module_path!().rsplit_once("::").unwrap().0);

	let mut node_implementations = Vec::with_capacity(data_struct.fields.len());
	let mut field_structs = Vec::with_capacity(data_struct.fields.len());

	for field in data_struct.fields {
		let Some(field_name) = field.ident else {
			return Err(Error::new(field.span(), String::from("Destruct cant be used on tuple structs")));
		};
		let ty = field.ty;
		let fn_name = quote::format_ident!("extract_ {field_name}");
		node_implementations.push(quote! {
			#[node_macro(category(""))]
			fn #fn_name(_: impl Ctx, data: #struct_name) -> #ty {
				data.#field_name
			}
		});

		field_structs.push(quote! {
			#crate_name::registry::FieldStruct {
				name: stringify!(#field_name),
				node_path: concat!()

			}
		})
	}

	Ok(quote! {
		impl graphene_core::registry::Destruct for #struct_name {
			fn fields() -> &[graphene_core::registry::FieldStruct] {
				&[

				]
			}
		}

	})
}
