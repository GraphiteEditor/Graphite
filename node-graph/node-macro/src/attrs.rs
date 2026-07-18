use crate::crate_ident::CrateIdent;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::ParseStream;
use syn::{Attribute, Ident, Token, Type, braced, token};

/// Implementation of the `attrs!` macro declaring typed attribute keys.
///
/// Grammar: `Name: Type`, comma-separated; `namespace { ... }` blocks nest and contribute a
/// `namespace:` prefix to the key name, which is otherwise derived mechanically from the key
/// ident (UpperCamel → snake_case).
pub fn attrs_impl(input: TokenStream) -> syn::Result<TokenStream> {
	let entries: Entries = syn::parse2(input)?;
	let crate_ident = CrateIdent::default();
	let core = crate_ident.gcore()?;

	let items = entries.0.iter().map(|entry| generate_entry(entry, core, "")).collect::<syn::Result<Vec<_>>>()?;

	Ok(quote! {
		#(#items)*
	})
}

struct Entries(Vec<Entry>);

enum Entry {
	Key { docs: Vec<Attribute>, ident: Ident, ty: Type },
	Namespace { docs: Vec<Attribute>, ident: Ident, entries: Vec<Entry> },
}

impl syn::parse::Parse for Entries {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self(parse_entries(input)?))
	}
}

fn parse_entries(input: ParseStream) -> syn::Result<Vec<Entry>> {
	let mut entries = Vec::new();
	while !input.is_empty() {
		let docs = input.call(Attribute::parse_outer)?;
		let ident: Ident = input.parse()?;
		if input.peek(token::Brace) {
			let content;
			braced!(content in input);
			entries.push(Entry::Namespace {
				docs,
				ident,
				entries: parse_entries(&content)?,
			});
		} else {
			input.parse::<Token![:]>()?;
			let ty: Type = input.parse()?;
			entries.push(Entry::Key { docs, ident, ty });
		}
		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}
	Ok(entries)
}

fn generate_entry(entry: &Entry, core: &TokenStream, prefix: &str) -> syn::Result<TokenStream> {
	match entry {
		Entry::Key { docs, ident, ty } => {
			let name = key_name(ident, prefix);
			Ok(quote! {
				#(#docs)*
				pub struct #ident;
				impl #core::attr::Attr for #ident {
					type Value = #ty;
					fn name() -> &'static str {
						#name
					}
				}
			})
		}
		Entry::Namespace { docs, ident, entries } => {
			let child_prefix = child_prefix(ident, prefix);
			let items = entries.iter().map(|entry| generate_entry(entry, core, &child_prefix)).collect::<syn::Result<Vec<_>>>()?;
			Ok(quote! {
				#(#docs)*
				pub mod #ident {
					use super::*;
					#(#items)*
				}
			})
		}
	}
}

fn key_name(ident: &Ident, prefix: &str) -> String {
	let snake = snake_case(&ident.to_string());
	if prefix.is_empty() { snake } else { format!("{prefix}:{snake}") }
}

fn child_prefix(ident: &Ident, prefix: &str) -> String {
	if prefix.is_empty() { ident.to_string() } else { format!("{prefix}:{ident}") }
}

fn snake_case(name: &str) -> String {
	let mut result = String::with_capacity(name.len() + 4);
	for (i, c) in name.chars().enumerate() {
		if c.is_uppercase() {
			if i > 0 {
				result.push('_');
			}
			result.extend(c.to_lowercase());
		} else {
			result.push(c);
		}
	}
	result
}
