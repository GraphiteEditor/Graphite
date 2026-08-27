use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields};

pub fn tag_derive(input: TokenStream) -> TokenStream {
	let ast: DeriveInput = syn::parse(input).unwrap();

	let name = &ast.ident;

	let data_struct = if let Data::Struct(data_struct) = ast.data {
		data_struct
	} else {
		panic!("Tag trait can only be derived for structs")
	};

	let named_fields = if let Fields::Named(named_fields) = data_struct.fields {
		named_fields
	} else {
		panic!("Tag trait can only be derived for structs with named_fields")
	};

	let struct_idents: Vec<_> = named_fields.named.iter().map(|field| field.ident.clone().unwrap()).collect();
	let struct_types: Vec<_> = named_fields.named.iter().map(|field| field.ty.clone()).collect();

	let new_name = format_ident!("_{}", name);

	let r#gen = quote! {
		struct #new_name {
			#( #struct_idents: <#struct_types as Tag>::Output ),*
		}

		impl Tag for #name {
			type Output = #new_name;

			fn get<R: Read + Seek>(ifd: &Ifd, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
				#( let #struct_idents = <#struct_types as Tag>::get(ifd, file)?; )*
				Ok(#new_name { #( #struct_idents ),* })
			}
		}
	};

	r#gen.into()
}
