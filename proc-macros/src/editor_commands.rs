use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Error, FnArg, Ident, ItemFn, ItemUse, Pat, Token, Visibility};

pub struct EditorCommands {
	imports: Vec<ItemUse>,
	functions: Vec<ItemFn>,
}

impl Parse for EditorCommands {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut imports = Vec::new();
		let mut functions = Vec::new();
		while !input.is_empty() {
			if input.peek(Token![use]) {
				imports.push(input.parse()?);
			} else {
				functions.push(input.parse()?);
			}
		}
		Ok(Self { imports, functions })
	}
}

pub fn editor_commands_impl(input: EditorCommands) -> syn::Result<TokenStream> {
	let imports = &input.imports;

	let mut variants = TokenStream::new();
	let mut stubs = TokenStream::new();
	let mut arms = TokenStream::new();

	for function in &input.functions {
		for attr in &function.attrs {
			if !attr.path().is_ident("doc") {
				return Err(Error::new(
					attr.span(),
					"command functions may not have attributes; anything that doesn't fit the `fn name(args…) -> Message` contract belongs in a plain impl block",
				));
			}
		}
		if !matches!(function.vis, Visibility::Inherited) {
			return Err(Error::new(
				function.span(),
				"command functions have no visibility modifier; the macro generates the public JS-facing stub",
			));
		}

		let signature = &function.sig;
		if let Some(receiver) = signature.receiver() {
			return Err(Error::new(receiver.span(), "command functions take no `self`; they are pure `args… -> Message` translations"));
		}
		if !signature.generics.params.is_empty() || signature.asyncness.is_some() || signature.unsafety.is_some() {
			return Err(Error::new(signature.span(), "command functions must be plain non-generic, non-async, safe functions"));
		}

		let docs = &function.attrs;
		let fn_name = &signature.ident;
		let variant = Ident::new(&fn_name.to_string().to_case(Case::Pascal), fn_name.span());
		let js_name = Ident::new(&fn_name.to_string().to_case(Case::Camel), fn_name.span());

		let mut param_names = Vec::new();
		let mut param_types = Vec::new();
		for parameter in &signature.inputs {
			let FnArg::Typed(pat_type) = parameter else { unreachable!("receiver is rejected above") };
			let Pat::Ident(pat_ident) = &*pat_type.pat else {
				return Err(Error::new(pat_type.span(), "command parameters must be plain identifiers"));
			};
			param_names.push(&pat_ident.ident);
			param_types.push(&*pat_type.ty);
		}

		let return_type = &signature.output;
		let body = &function.block;

		variants.extend(quote! {
			#(#docs)*
			#variant { #(#param_names: #param_types,)* },
		});
		stubs.extend(quote! {
			#(#docs)*
			#[cfg(not(feature = "native"))]
			#[wasm_bindgen(js_name = #js_name)]
			pub fn #fn_name(&self, #(#param_names: #param_types,)*) {
				self.dispatch((move || #return_type #body)())
			}
			#(#docs)*
			#[cfg(feature = "native")]
			#[wasm_bindgen(js_name = #js_name)]
			pub fn #fn_name(&self, #(#param_names: #param_types,)*) {
				self.send(EditorCommand::#variant { #(#param_names,)* })
			}
		});
		arms.extend(quote! {
			EditorCommand::#variant { #(#param_names,)* } => #body,
		});
	}

	Ok(quote! {
		#(
			#[cfg(feature = "editor")]
			#imports
		)*

		#[cfg(any(feature = "native", not(target_family = "wasm")))]
		#[derive(serde::Serialize, serde::Deserialize)]
		pub enum EditorCommand {
			#variants
		}

		#[cfg(all(feature = "editor", any(feature = "native", not(target_family = "wasm"))))]
		impl From<EditorCommand> for Message {
			fn from(command: EditorCommand) -> Self {
				match command {
					#arms
				}
			}
		}

		#[cfg(target_family = "wasm")]
		#[wasm_bindgen]
		impl EditorWrapper {
			#stubs
		}
	})
}
