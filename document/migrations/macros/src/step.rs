use proc_macro2::{TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream};
use syn::{Error, Ident, LitInt, LitStr, Token};

/// A declaration as written in macro input: `"identifier"@version`, or `network "identifier"@version` for a network definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Declaration {
	pub identifier: String,
	pub version: u32,
	pub network: bool,
}

impl Parse for Declaration {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let network = parse_network_keyword(input)?;
		let identifier: LitStr = input.parse()?;
		input.parse::<Token![@]>()?;
		let version: LitInt = input.parse()?;

		Ok(Self {
			identifier: identifier.value(),
			version: version.base10_parse()?,
			network,
		})
	}
}

/// Consumes a leading `network` keyword, returning whether it was there.
fn parse_network_keyword(input: ParseStream) -> syn::Result<bool> {
	if !input.peek(Ident) {
		return Ok(false);
	}

	let keyword: Ident = input.parse()?;
	if keyword != "network" {
		return Err(Error::new(keyword.span(), "expected `network` or a declaration identifier string"));
	}
	Ok(true)
}

/// `"identifier"@n => "identifier"@m`, or `"identifier"@n => @m` for a version step that keeps the identifier and kind. Either side may be a
/// `network` definition.
pub(crate) struct Step {
	pub source: Declaration,
	pub target: Declaration,
}

impl Parse for Step {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let source: Declaration = input.parse()?;
		input.parse::<Token![=>]>()?;

		let (target_identifier, target_network) = match input.peek(LitStr) || input.peek(Ident) {
			true => {
				let network = parse_network_keyword(input)?;
				(input.parse::<LitStr>()?.value(), network)
			}
			false => (source.identifier.clone(), source.network),
		};
		input.parse::<Token![@]>()?;
		let target_version_literal: LitInt = input.parse()?;
		let target_version = target_version_literal.base10_parse()?;

		// Every migration is exactly one step in its lineage, including renames
		if Some(target_version) != source.version.checked_add(1) {
			return Err(Error::new(
				target_version_literal.span(),
				format!("a migration steps one version, so the target must be version {}", source.version.saturating_add(1)),
			));
		}

		let target = Declaration {
			identifier: target_identifier,
			version: target_version,
			network: target_network,
		};
		Ok(Self { source, target })
	}
}

/// Every declaration named by a `decl!` call anywhere in `tokens`, including inside other macro calls, in first-appearance order.
pub(crate) fn scan_pins(tokens: TokenStream) -> syn::Result<Vec<Declaration>> {
	let mut pins = Vec::new();
	collect_pins(tokens, &mut pins)?;
	Ok(pins)
}

fn collect_pins(tokens: TokenStream, pins: &mut Vec<Declaration>) -> syn::Result<()> {
	let tokens: Vec<TokenTree> = tokens.into_iter().collect();

	for (index, token) in tokens.iter().enumerate() {
		match token {
			TokenTree::Ident(ident) if ident == "decl" => {
				let bang = matches!(tokens.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == '!');
				let Some(TokenTree::Group(group)) = tokens.get(index + 2) else { continue };
				if !bang {
					continue;
				}

				let pin: Declaration = syn::parse2(group.stream())?;
				if !pins.contains(&pin) {
					pins.push(pin);
				}
			}
			TokenTree::Group(group) => collect_pins(group.stream(), pins)?,
			_ => {}
		}
	}

	Ok(())
}
