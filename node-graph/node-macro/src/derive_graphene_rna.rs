use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Attribute, DeriveInput, LitStr, MetaList, Token};

pub fn derive_graphene_rna_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse2::<DeriveInput>(input_item).unwrap();

	match input.data {
		syn::Data::Enum(en) => derive_enum(input.ident, en),
		_ => Err(syn::Error::new(input.ident.span(), "Only enums are supported at the moment")),
	}
}

#[derive(Default)]
struct BasicRna {
	label: String,
	description: Option<String>,
	icon: Option<String>,
}
impl BasicRna {
	fn from_attribute(attr: &Attribute) -> syn::Result<Option<BasicRna>> {
		if !attr.path().is_ident("rna") {
			return Ok(None);
		}

		attr.parse_args_with(Self::parse_meta)
	}

	fn parse_meta(buf: &syn::parse::ParseBuffer<'_>) -> syn::Result<Option<BasicRna>> {
		let mut res = BasicRna::default();

		if buf.peek(LitStr) {
			let label_tok: LitStr = buf.parse()?;
			res.label = label_tok.value();

			if buf.is_empty() {
				return Ok(Some(res));
			} else {
				let _ = buf.parse::<Token![,]>()?;
			}
		}

		while !buf.is_empty() {
			let item: MetaList = buf.parse()?;
			if item.path.is_ident("doc") {
				let doc_tok: LitStr = item.parse_args()?;
				res.description = Some(doc_tok.value());
			} else if item.path.is_ident("icon") {
				let icon_tok: LitStr = item.parse_args()?;
				res.icon = Some(icon_tok.value());
			} else {
				return Err(syn::Error::new(item.path.span(), "Unexpected meta item"));
			}

			if buf.is_empty() {
				break;
			}

			let _ = buf.parse::<Token![,]>()?;
		}
		Ok(Some(res))
	}

	fn merge(&mut self, rhs: BasicRna) {
		if rhs.label.len() > 0 {
			self.label = rhs.label;
		}
		if let Some(d) = rhs.description {
			self.description = Some(d)
		};
		if let Some(i) = rhs.icon {
			self.icon = Some(i)
		};
	}
}

struct Variant {
	name: Ident,
	basic_rna: BasicRna,
}

fn derive_enum(name: Ident, input: syn::DataEnum) -> syn::Result<TokenStream> {
	let mut variants = vec![Vec::new()];
	for va in &input.variants {
		if va.attrs.iter().any(|a| a.path().is_ident("menu_separator")) {
			variants.push(Vec::new());
		}

		let mut basic_rna = BasicRna::default();
		for attr in &va.attrs {
			if let Some(ra) = BasicRna::from_attribute(attr)? {
				basic_rna.merge(ra);
			}
		}
		if basic_rna.label.len() == 0 {
			basic_rna.label = ident_to_label(&va.ident);
		}

		variants.last_mut().unwrap().push(Variant { name: va.ident.clone(), basic_rna })
	}

	let group: Vec<_> = variants
		.iter()
		.map(|vg| {
			let items = vg
				.iter()
				.map(|v| {
					let vname = &v.name;
					let icon = match &v.basic_rna.icon {
						Some(s) => quote! { Some(#s) },
						None => quote! { None },
					};
					quote! { ( #name::#vname, #icon), }
				})
				.collect::<Vec<_>>();
			quote! { &[ #(#items)* ], }
		})
		.collect();
	let display_arm: Vec<_> = variants
		.iter()
		.map(|vg| vg.iter())
		.flatten()
		.map(|v| {
			let vn = &v.name;
			let vl = &v.basic_rna.label;
			quote! { #name::#vn => write!(f, #vl), }
		})
		.collect();
	Ok(quote! {
		impl crate::vector::misc::AsU32 for #name {
			fn as_u32(&self) -> u32 {
				*self as u32
			}
		}

		impl crate::vector::misc::DropdownableStatic for #name {
			fn list() -> &'static [&'static [(Self, Option<&'static str>)]] {
				&[ #(#group)* ]
			}
		}

		impl core::fmt::Display for #name {
			fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
				match self {
					#( #display_arm )*
				}
			}
		}
	})
}

fn ident_to_label(id: &Ident) -> String {
	use convert_case::{Case, Casing};
	id.to_string().from_case(Case::Pascal).to_case(Case::Title)
}
