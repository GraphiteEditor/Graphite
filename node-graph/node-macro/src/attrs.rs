use crate::crate_ident::CrateIdent;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::ParseStream;
use syn::{Attribute, Expr, Ident, Token, Type, braced, token};

/// Implementation of the `attrs!` macro declaring typed attribute keys.
///
/// Grammar: `Name: Type`, comma-separated; `namespace { ... }` blocks nest and contribute a
/// `namespace:` prefix to the key name, which is otherwise derived mechanically from the key
/// ident (UpperCamel → snake_case). An optional `= value` after the type declares the key's
/// implicit default, overriding the value type's `Default` for items lacking the attribute.
pub fn attrs_impl(input: TokenStream) -> syn::Result<TokenStream> {
	let entries: Entries = syn::parse2(input)?;
	let crate_ident = CrateIdent::default();
	let core = crate_ident.gcore()?;

	let items = entries.0.iter().map(|entry| generate_entry(entry, core, "")).collect::<syn::Result<Vec<_>>>()?;
	let lookup = generate_implicit_default_lookup(&entries.0, core);

	Ok(quote! {
		#(#items)*
		#lookup
	})
}

struct Entries(Vec<Entry>);

enum Entry {
	Key {
		docs: Vec<Attribute>,
		ident: Ident,
		ty: Box<Type>,
		default: Option<Box<Expr>>,
	},
	Namespace {
		docs: Vec<Attribute>,
		ident: Ident,
		entries: Vec<Entry>,
	},
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
			let ty = Box::new(input.parse::<Type>()?);
			let default = if input.peek(Token![=]) {
				input.parse::<Token![=]>()?;
				Some(Box::new(input.parse::<Expr>()?))
			} else {
				None
			};
			entries.push(Entry::Key { docs, ident, ty, default });
		}
		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}
	Ok(entries)
}

fn generate_entry(entry: &Entry, core: &TokenStream, prefix: &str) -> syn::Result<TokenStream> {
	match entry {
		Entry::Key { docs, ident, ty, default } => {
			let name = key_name(ident, prefix);
			let implicit_default = default.as_ref().map(|value| {
				quote! {
					fn implicit_default() -> Self::Value {
						#value
					}
				}
			});
			Ok(quote! {
				#(#docs)*
				pub struct #ident;
				impl #core::attr::Attr for #ident {
					type Value = #ty;
					fn name() -> &'static str {
						#name
					}
					#implicit_default
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

/// Generates a string-keyed lookup of the boxed implicit defaults for erased attribute code paths,
/// or nothing if no key in this invocation declares a `= value` default.
fn generate_implicit_default_lookup(entries: &[Entry], core: &TokenStream) -> Option<TokenStream> {
	let mut defaulted_keys = Vec::new();
	collect_defaulted_key_paths(entries, &TokenStream::new(), &mut defaulted_keys);

	if defaulted_keys.is_empty() {
		return None;
	}

	Some(quote! {
		/// The boxed implicit default for the key named `key`, if that key declares one with `= value` in `attrs!`.
		pub fn implicit_default_value(key: &str) -> ::std::option::Option<::std::boxed::Box<dyn #core::list::AnyAttributeValue>> {
			#(
				if key == <#defaulted_keys as #core::attr::Attr>::name() {
					return ::std::option::Option::Some(::std::boxed::Box::new(<#defaulted_keys as #core::attr::Attr>::implicit_default()));
				}
			)*
			::std::option::Option::None
		}
	})
}

/// Walks the entry tree collecting module-qualified paths (like `namespace::Key`) of keys that declare a default.
fn collect_defaulted_key_paths(entries: &[Entry], module_path: &TokenStream, paths: &mut Vec<TokenStream>) {
	for entry in entries {
		match entry {
			Entry::Key { ident, default: Some(_), .. } => paths.push(quote!(#module_path #ident)),
			Entry::Key { .. } => {}
			Entry::Namespace { ident, entries, .. } => collect_defaulted_key_paths(entries, &quote!(#module_path #ident::), paths),
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
