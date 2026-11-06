use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Error, Ident, LitInt, Path, Token, Type};

/// The stored roots a format step can convert, by the `FormatMigration` method converting each.
const ROOTS: &[(&str, &str)] = &[
	("registry", "migrate_registry"),
	("delta", "migrate_delta"),
	("hot_op", "migrate_hot_op"),
	("session", "migrate_session"),
];

/// `1 => 2, registry: v1::Registry, delta: v1::Delta`: the version step and the frozen type of each root it converts. An entry
/// `document: function` names a `fn(&mut DocumentPayloads) -> Result<(), MigrationError>` run on the converted document.
pub(crate) struct FormatStep {
	pub from: u32,
	pub to: u32,
	roots: Vec<(Ident, Type)>,
	document: Option<Path>,
}

impl Parse for FormatStep {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let from_literal: LitInt = input.parse()?;
		input.parse::<Token![=>]>()?;
		let to_literal: LitInt = input.parse()?;

		let from: u32 = from_literal.base10_parse()?;
		let to: u32 = to_literal.base10_parse()?;
		if from.checked_add(1) != Some(to) {
			return Err(Error::new(
				to_literal.span(),
				format!("a format step goes from one version to the next, so this must be {}", from.saturating_add(1)),
			));
		}

		let mut roots: Vec<(Ident, Type)> = Vec::new();
		let mut document = None;
		while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
			let root: Ident = input.parse()?;
			if root == "document" {
				if document.is_some() {
					return Err(Error::new(root.span(), "the document function is already named"));
				}
				input.parse::<Token![:]>()?;
				document = Some(input.parse()?);
				continue;
			}

			if !ROOTS.iter().any(|(name, _)| root == name) {
				let names: Vec<&str> = ROOTS.iter().map(|(name, _)| *name).collect();
				return Err(Error::new(root.span(), format!("the stored roots are {}", names.join(", "))));
			}
			if roots.iter().any(|(existing, _)| *existing == root) {
				return Err(Error::new(root.span(), "this root is already listed"));
			}

			input.parse::<Token![:]>()?;
			roots.push((root, input.parse()?));
		}

		Ok(Self { from, to, roots, document })
	}
}

/// A `FormatMigration` converting each listed root through its `Upgrade`, leaving the others' payloads as they are.
pub(crate) fn expand(core: &TokenStream2, step: &FormatStep) -> TokenStream2 {
	let from = step.from;

	let methods = step.roots.iter().map(|(root, frozen)| {
		let method = ROOTS.iter().find(|(name, _)| root == name).map(|(_, method)| format_ident!("{method}"));
		quote! {
			fn #method(&self, payload: &#core::Payload) -> ::core::result::Result<#core::Payload, #core::MigrationError> {
				let previous: #frozen = payload.decode()?;
				payload.encode_as(&#core::Upgrade::upgrade(previous)?)
			}
		}
	});

	let rewrites_history = step.roots.iter().any(|(root, _)| root == "delta");
	let history_policy = rewrites_history.then(|| {
		quote! {
			fn history_policy(&self) -> #core::HistoryPolicy {
				#core::HistoryPolicy::Rewrite
			}
		}
	});

	let document = step.document.as_ref().map(|function| {
		quote! {
			fn migrate_document(&self, document: &mut #core::DocumentPayloads) -> ::core::result::Result<(), #core::MigrationError> {
				#function(document)
			}
		}
	});

	quote! {
		struct FormatStep;

		impl #core::FormatMigration for FormatStep {
			fn migrates_from(&self) -> u32 {
				#from
			}
			#history_policy
			#(#methods)*
			#document
		}
	}
}
