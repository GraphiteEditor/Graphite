use convert_case::{Case, Casing};
use indoc::indoc;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, ToTokens};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, Error, ExprTuple, FnArg, GenericParam, Ident, ItemFn, LitFloat, LitStr, Meta, Pat, PatIdent, PatType, Path, ReturnType, Type, TypeTuple, WhereClause};

use crate::codegen::generate_node_code;

#[derive(Debug)]
pub(crate) struct ParsedNodeFn {
	pub(crate) attributes: NodeFnAttributes,
	pub(crate) fn_name: Ident,
	pub(crate) struct_name: Ident,
	pub(crate) mod_name: Ident,
	pub(crate) fn_generics: Vec<GenericParam>,
	pub(crate) where_clause: Option<WhereClause>,
	pub(crate) input: Input,
	pub(crate) output_type: Type,
	pub(crate) is_async: bool,
	pub(crate) fields: Vec<ParsedField>,
	pub(crate) body: TokenStream2,
	pub(crate) crate_name: proc_macro_crate::FoundCrate,
}

#[derive(Debug, Default)]
pub(crate) struct NodeFnAttributes {
	pub(crate) category: Option<LitStr>,
	pub(crate) display_name: Option<LitStr>,
	pub(crate) path: Option<Path>,
	pub(crate) skip_impl: bool,
	// Add more attributes as needed
}

#[derive(Debug)]
pub(crate) enum ParsedField {
	Regular {
		pat_ident: PatIdent,
		ty: Type,
		exposed: bool,
		default_value: Option<TokenStream2>,
		number_min: Option<LitFloat>,
		number_max: Option<LitFloat>,
		number_mode_range: Option<ExprTuple>,
		implementations: Punctuated<Type, Comma>,
	},
	Node {
		pat_ident: PatIdent,
		input_type: Type,
		output_type: Type,
		implementations: Punctuated<TypeTuple, Comma>,
	},
}
#[derive(Debug)]
pub(crate) struct Input {
	pub(crate) pat_ident: PatIdent,
	pub(crate) ty: Type,
	pub(crate) implementations: Punctuated<Type, Comma>,
}

impl Parse for NodeFnAttributes {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut category = None;
		let mut display_name = None;
		let mut path = None;
		let mut skip_impl = false;

		let content = input;
		// let content;
		// syn::parenthesized!(content in input);

		let nested = content.call(Punctuated::<Meta, Comma>::parse_terminated)?;
		for meta in nested {
			match meta {
				Meta::List(meta) if meta.path.is_ident("category") => {
					if category.is_some() {
						return Err(Error::new_spanned(meta, "Multiple 'category' attributes are not allowed"));
					}
					let lit: LitStr = meta
						.parse_args()
						.map_err(|_| Error::new_spanned(meta, "Expected a string literal for 'category', e.g., category(\"Value\")"))?;
					category = Some(lit);
				}
				Meta::List(meta) if meta.path.is_ident("name") => {
					if display_name.is_some() {
						return Err(Error::new_spanned(meta, "Multiple 'name' attributes are not allowed"));
					}
					let parsed_name: LitStr = meta.parse_args().map_err(|_| Error::new_spanned(meta, "Expected a string for 'name', e.g., name(\"Memoize\")"))?;
					display_name = Some(parsed_name);
				}
				Meta::List(meta) if meta.path.is_ident("path") => {
					if path.is_some() {
						return Err(Error::new_spanned(meta, "Multiple 'path' attributes are not allowed"));
					}
					let parsed_path: Path = meta
						.parse_args()
						.map_err(|_| Error::new_spanned(meta, "Expected a valid path for 'path', e.g., path(\"crate::MemoizeNode\")"))?;
					path = Some(parsed_path);
				}
				Meta::Path(path) if path.is_ident("skip_impl") => {
					if skip_impl {
						return Err(Error::new_spanned(path, "Multiple 'skip_impl' attributes are not allowed"));
					}
					skip_impl = true;
				}
				_ => {
					return Err(Error::new_spanned(
						meta,
						indoc!(
							r#"
							Unsupported attribute in `node_fn`.
							Supported attributes are 'category', 'path' and 'name'.
							
							Example usage:
							#[node_fn(category("Value"), name("TestNode"))]
							"#
						),
					));
				}
			}
		}

		Ok(NodeFnAttributes {
			category,
			display_name,
			path,
			skip_impl,
		})
	}
}

fn parse_node_fn(attr: TokenStream2, item: TokenStream2) -> syn::Result<ParsedNodeFn> {
	let attributes = syn::parse2::<NodeFnAttributes>(attr.clone()).map_err(|e| Error::new(e.span(), format!("Failed to parse node_fn attributes: {}", e)))?;
	let input_fn = syn::parse2::<ItemFn>(item.clone()).map_err(|e| Error::new(e.span(), format!("Failed to parse function: {}. Make sure it's a valid Rust function.", e)))?;

	let fn_name = input_fn.sig.ident.clone();
	let struct_name = format_ident!("{}", fn_name.to_string().to_case(Case::Pascal));
	let mod_name = fn_name.clone();
	let fn_generics = input_fn.sig.generics.params.into_iter().collect();
	let is_async = input_fn.sig.asyncness.is_some();

	let (input, fields) = parse_inputs(&input_fn.sig.inputs)?;
	let output_type = parse_output(&input_fn.sig.output)?;
	let where_clause = input_fn.sig.generics.where_clause;
	let body = input_fn.block.to_token_stream();
	let crate_name = proc_macro_crate::crate_name("graphene-core").map_err(|e| {
		Error::new(
			proc_macro2::Span::call_site(),
			format!("Failed to find location of graphene_core. Make sure it is imported as a dependency: {}", e),
		)
	})?;

	Ok(ParsedNodeFn {
		attributes,
		fn_name,
		struct_name,
		mod_name,
		fn_generics,
		input,
		output_type,
		is_async,
		fields,
		where_clause,
		body,
		crate_name,
	})
}

fn parse_inputs(inputs: &Punctuated<FnArg, Comma>) -> syn::Result<(Input, Vec<ParsedField>)> {
	let mut fields = Vec::new();
	let mut input = None;

	for (index, arg) in inputs.iter().enumerate() {
		if let FnArg::Typed(PatType { pat, ty, attrs, .. }) = arg {
			if index == 0 {
				if extract_attribute(attrs, "default").is_some() {
					return Err(Error::new_spanned(&attrs[0], "No default values for first argument allowed".to_string()));
				}
				if extract_attribute(attrs, "expose").is_some() {
					return Err(Error::new_spanned(&attrs[0], "Call argument cannot be exposed".to_string()));
				}
				let pat_ident = match (**pat).clone() {
					Pat::Ident(pat_ident) => pat_ident,
					Pat::Wild(wild) => PatIdent {
						attrs: wild.attrs,
						by_ref: None,
						mutability: None,
						ident: wild.underscore_token.into(),
						subpat: None,
					},
					_ => continue,
				};

				let implementations = extract_attribute(attrs, "implementations")
					.map(|attr| parse_implementations(attr, &pat_ident.ident))
					.transpose()?
					.unwrap_or_default();
				input = Some(Input {
					pat_ident,
					ty: (**ty).clone(),
					implementations,
				});
			} else if let Pat::Ident(pat_ident) = &**pat {
				let field = parse_field(pat_ident.clone(), (**ty).clone(), attrs).map_err(|e| Error::new_spanned(pat_ident, format!("Failed to parse argument '{}': {}", pat_ident.ident, e)))?;
				fields.push(field);
			} else {
				return Err(Error::new_spanned(pat, "Expected a simple identifier for the field name"));
			}
		} else {
			return Err(Error::new_spanned(arg, "Expected a typed argument (e.g., `x: i32`)"));
		}
	}

	let input = input.ok_or_else(|| Error::new_spanned(inputs, "Expected at least one input argument. The first argument should be the node input type."))?;
	Ok((input, fields))
}

fn parse_implementations<T: Parse>(attr: &Attribute, name: &Ident) -> syn::Result<Punctuated<T, Comma>> {
	let content: TokenStream2 = attr
		.parse_args()
		.map_err(|e| Error::new_spanned(attr, format!("Invalid implementations for argument '{}': {}", name, e)))?;
	let parser = Punctuated::<T, Comma>::parse_terminated;
	parser
		.parse2(content)
		.map_err(|e| Error::new_spanned(attr, format!("Failed to parse implementations for argument '{}': {}", name, e)))
}

fn parse_field(pat_ident: PatIdent, ty: Type, attrs: &[Attribute]) -> syn::Result<ParsedField> {
	let name = &pat_ident.ident;
	let default_value = extract_attribute(attrs, "default").and_then(|attr| {
		attr.parse_args()
			.map_err(|e| Error::new_spanned(attr, format!("Invalid `default` value for argument '{}': {}", name, e)))
			.ok()
	});
	let number_min = extract_attribute(attrs, "min").and_then(|attr| {
		attr.parse_args()
			.map_err(|e| Error::new_spanned(attr, format!("Invalid numerical `min` value for argument '{}': {}", name, e)))
			.ok()
	});
	let number_max = extract_attribute(attrs, "max").and_then(|attr| {
		attr.parse_args()
			.map_err(|e| Error::new_spanned(attr, format!("Invalid numerical `max` value for argument '{}': {}", name, e)))
			.ok()
	});
	let exposed = extract_attribute(attrs, "expose").is_some();
	let number_mode_range: Option<ExprTuple> = extract_attribute(attrs, "mode_range").and_then(|attr| {
		attr.parse_args()
			.map_err(|e| Error::new_spanned(attr, format!("Invalid `mode_range` tuple of min and max range slider values for argument '{}': {}", name, e)))
			.ok()
	});
	if let Some(range) = &number_mode_range {
		if range.elems.len() != 2 {
			return Err(Error::new_spanned(range, "Expected a tuple of two values for `mode_range` for the min and max, respectively"));
		}
	}

	let implementations = extract_attribute(attrs, "implementations")
		.map(|attr| parse_implementations(attr, name))
		.transpose()?
		.unwrap_or_default();

	let (is_node, node_input_type, node_output_type) = parse_node_type(&ty);

	if is_node {
		let (input_type, output_type) = node_input_type
			.zip(node_output_type)
			.ok_or_else(|| Error::new_spanned(&ty, "Invalid Node type. Expected `impl Node<Input, Output = OutputType>`"))?;
		if default_value.is_some() {
			return Err(Error::new_spanned(&ty, "No default values for `impl Node` allowed"));
		}
		let implementations = extract_attribute(attrs, "implementations")
			.map(|attr| parse_implementations(attr, name))
			.transpose()?
			.unwrap_or_default();

		Ok(ParsedField::Node {
			pat_ident,
			input_type,
			output_type,
			implementations,
		})
	} else {
		Ok(ParsedField::Regular {
			pat_ident,
			exposed,
			number_min,
			number_max,
			number_mode_range,
			ty,
			default_value,
			implementations,
		})
	}
}

fn parse_node_type(ty: &Type) -> (bool, Option<Type>, Option<Type>) {
	if let Type::ImplTrait(impl_trait) = ty {
		for bound in &impl_trait.bounds {
			if let syn::TypeParamBound::Trait(trait_bound) = bound {
				if trait_bound.path.segments.last().map_or(false, |seg| seg.ident == "Node") {
					if let syn::PathArguments::AngleBracketed(args) = &trait_bound.path.segments.last().unwrap().arguments {
						let input_type = args.args.iter().find_map(|arg| if let syn::GenericArgument::Type(ty) = arg { Some(ty.clone()) } else { None });
						let output_type = args.args.iter().find_map(|arg| {
							if let syn::GenericArgument::AssocType(assoc_type) = arg {
								if assoc_type.ident == "Output" {
									Some(assoc_type.ty.clone())
								} else {
									None
								}
							} else {
								None
							}
						});
						return (true, input_type, output_type);
					}
				}
			}
		}
	}
	(false, None, None)
}

fn parse_output(output: &ReturnType) -> syn::Result<Type> {
	match output {
		ReturnType::Default => Ok(syn::parse_quote!(())),
		ReturnType::Type(_, ty) => Ok((**ty).clone()),
	}
}

fn extract_attribute<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
	attrs.iter().find(|attr| attr.path().is_ident(name))
}

// Modify the new_node_fn function to use the code generation
pub fn new_node_fn(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
	match parse_node_fn(attr, item.clone()).and_then(|x| generate_node_code(&x)) {
		Ok(parsed) => {
			/*let generated_code = generate_node_code(&parsed);
			// panic!("{}", generated_code.to_string());
			quote! {
				// #item
				#generated_code
			}*/
			parsed
		}
		Err(e) => {
			// Return the error as a compile error
			Error::new(e.span(), format!("Failed to parse node function: {}", e)).to_compile_error()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use proc_macro2::Span;
	use proc_macro_crate::FoundCrate;
	use quote::quote;
	use syn::parse_quote;
	fn pat_ident(name: &str) -> PatIdent {
		PatIdent {
			attrs: Vec::new(),
			by_ref: None,
			mutability: None,
			ident: Ident::new(name, Span::call_site()),
			subpat: None,
		}
	}

	fn assert_parsed_node_fn(parsed: &ParsedNodeFn, expected: &ParsedNodeFn) {
		assert_eq!(parsed.fn_name, expected.fn_name);
		assert_eq!(parsed.struct_name, expected.struct_name);
		assert_eq!(parsed.mod_name, expected.mod_name);
		assert_eq!(parsed.is_async, expected.is_async);
		assert_eq!(format!("{:?}", parsed.input), format!("{:?}", expected.input));
		assert_eq!(format!("{:?}", parsed.output_type), format!("{:?}", expected.output_type));
		assert_eq!(parsed.attributes.category, expected.attributes.category);
		assert_eq!(parsed.attributes.display_name, expected.attributes.display_name);
		assert_eq!(parsed.attributes.path, expected.attributes.path);
		assert_eq!(parsed.attributes.skip_impl, expected.attributes.skip_impl);
		assert_eq!(parsed.fields.len(), expected.fields.len());

		for (parsed_field, expected_field) in parsed.fields.iter().zip(expected.fields.iter()) {
			match (parsed_field, expected_field) {
				(
					ParsedField::Regular {
						pat_ident: p_name,
						ty: p_ty,
						exposed: p_exp,
						default_value: p_default,
						..
					},
					ParsedField::Regular {
						pat_ident: e_name,
						ty: e_ty,
						exposed: e_exp,
						default_value: e_default,
						..
					},
				) => {
					assert_eq!(p_name, e_name);
					assert_eq!(p_exp, e_exp);
					assert_eq!(p_default.to_token_stream().to_string(), e_default.to_token_stream().to_string());
					assert_eq!(format!("{:?}", p_ty), format!("{:?}", e_ty));
				}
				(
					ParsedField::Node {
						pat_ident: p_name,
						input_type: p_input,
						output_type: p_output,
						..
					},
					ParsedField::Node {
						pat_ident: e_name,
						input_type: e_input,
						output_type: e_output,
						..
					},
				) => {
					assert_eq!(p_name, e_name);
					assert_eq!(format!("{:?}", p_input), format!("{:?}", e_input));
					assert_eq!(format!("{:?}", p_output), format!("{:?}", e_output));
				}
				_ => panic!("Mismatched field types"),
			}
		}
	}

	#[test]
	fn test_basic_node() {
		let attr = quote!(category("Math: Arithmetic"), path(graphene_core::TestNode), skip_impl);
		let input = quote!(
			fn add(a: f64, b: f64) -> f64 {
				a + b
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Math: Arithmetic")),
				display_name: None,
				path: Some(parse_quote!(graphene_core::TestNode)),
				skip_impl: true,
			},
			fn_name: Ident::new("add", Span::call_site()),
			struct_name: Ident::new("Add", Span::call_site()),
			mod_name: Ident::new("add", Span::call_site()),
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("a"),
				ty: parse_quote!(f64),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(f64),
			is_async: false,
			fields: vec![ParsedField::Regular {
				pat_ident: pat_ident("b"),
				ty: parse_quote!(f64),
				exposed: false,
				default_value: None,
				number_min: None,
				number_max: None,
				number_mode_range: None,
				implementations: Punctuated::new(),
			}],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_impl_node() {
		let attr = quote!(category("General"));
		let input = quote!(
			fn transform<T: 'static>(footprint: Footprint, transform_target: impl Node<Footprint, Output = T>, translate: DVec2) -> T {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("General")),
				display_name: None,
				path: None,
				skip_impl: false,
			},
			fn_name: Ident::new("transform", Span::call_site()),
			struct_name: Ident::new("Transform", Span::call_site()),
			mod_name: Ident::new("transform", Span::call_site()),
			fn_generics: vec![parse_quote!(T: 'static)],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("footprint"),
				ty: parse_quote!(Footprint),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(T),
			is_async: false,
			fields: vec![
				ParsedField::Node {
					pat_ident: pat_ident("transform_target"),
					input_type: parse_quote!(Footprint),
					output_type: parse_quote!(T),
					implementations: Punctuated::new(),
				},
				ParsedField::Regular {
					pat_ident: pat_ident("translate"),
					ty: parse_quote!(DVec2),
					exposed: false,
					default_value: None,
					number_min: None,
					number_max: None,
					number_mode_range: None,
					implementations: Punctuated::new(),
				},
			],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_default_values() {
		let attr = quote!(category("Vector: Generator"));
		let input = quote!(
			fn circle(_: (), #[default(50.0)] radius: f64) -> VectorData {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Vector: Generator")),
				display_name: None,
				path: None,
				skip_impl: false,
			},
			fn_name: Ident::new("circle", Span::call_site()),
			struct_name: Ident::new("Circle", Span::call_site()),
			mod_name: Ident::new("circle", Span::call_site()),
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("_"),
				ty: parse_quote!(()),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(VectorData),
			is_async: false,
			fields: vec![ParsedField::Regular {
				pat_ident: pat_ident("radius"),
				ty: parse_quote!(f64),
				exposed: false,
				default_value: Some(quote!(50.0)),
				number_min: None,
				number_max: None,
				number_mode_range: None,
				implementations: Punctuated::new(),
			}],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_implementations() {
		let attr = quote!(category("Raster: Adjustment"));
		let input = quote!(
			fn levels<P: Pixel>(image: ImageFrame<P>, #[implementations(f32, f64)] shadows: f64) -> ImageFrame<P> {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Raster: Adjustment")),
				display_name: None,
				path: None,
				skip_impl: false,
			},
			fn_name: Ident::new("levels", Span::call_site()),
			struct_name: Ident::new("Levels", Span::call_site()),
			mod_name: Ident::new("levels", Span::call_site()),
			fn_generics: vec![parse_quote!(P: Pixel)],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("image"),
				ty: parse_quote!(ImageFrame<P>),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(ImageFrame<P>),
			is_async: false,
			fields: vec![ParsedField::Regular {
				pat_ident: pat_ident("shadows"),
				ty: parse_quote!(f64),
				exposed: false,
				default_value: None,
				number_min: None,
				number_max: None,
				number_mode_range: None,
				implementations: {
					let mut p = Punctuated::new();
					p.push(parse_quote!(f32));
					p.push(parse_quote!(f64));
					p
				},
			}],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_number_min_max_range_mode() {
		let attr = quote!(category("Math: Arithmetic"), path(graphene_core::TestNode));
		let input = quote!(
			fn add(
				a: f64,
				#[mode_range(0., 100.)]
				#[min(-500.)]
				#[max(500.)]
				b: f64,
			) -> f64 {
				a + b
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Math: Arithmetic")),
				display_name: None,
				path: Some(parse_quote!(graphene_core::TestNode)),
				skip_impl: false,
			},
			fn_name: Ident::new("add", Span::call_site()),
			struct_name: Ident::new("Add", Span::call_site()),
			mod_name: Ident::new("add", Span::call_site()),
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("a"),
				ty: parse_quote!(f64),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(f64),
			is_async: false,
			fields: vec![ParsedField::Regular {
				pat_ident: pat_ident("b"),
				ty: parse_quote!(f64),
				exposed: false,
				default_value: None,
				number_min: Some(parse_quote!(-500.)),
				number_max: Some(parse_quote!(500.)),
				number_mode_range: Some(parse_quote!((0., 100.))),
				implementations: Punctuated::new(),
			}],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_async_node() {
		let attr = quote!(category("IO"));
		let input = quote!(
			async fn load_image(api: &WasmEditorApi, #[expose] path: String) -> ImageFrame<Color> {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("IO")),
				display_name: None,
				path: None,
				skip_impl: false,
			},
			fn_name: Ident::new("load_image", Span::call_site()),
			struct_name: Ident::new("LoadImage", Span::call_site()),
			mod_name: Ident::new("load_image", Span::call_site()),
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("api"),
				ty: parse_quote!(&WasmEditorApi),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(ImageFrame<Color>),
			is_async: true,
			fields: vec![ParsedField::Regular {
				pat_ident: pat_ident("path"),
				ty: parse_quote!(String),
				exposed: true,
				default_value: None,
				number_min: None,
				number_max: None,
				number_mode_range: None,
				implementations: Punctuated::new(),
			}],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_custom_name() {
		let attr = quote!(category("Custom"), name("CustomNode2"));
		let input = quote!(
			fn custom_node(input: i32) -> i32 {
				input * 2
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Custom")),
				display_name: Some(parse_quote!("CustomNode2")),
				path: None,
				skip_impl: false,
			},
			fn_name: Ident::new("custom_node", Span::call_site()),
			struct_name: Ident::new("CustomNode", Span::call_site()),
			mod_name: Ident::new("custom_node", Span::call_site()),
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("input"),
				ty: parse_quote!(i32),
				implementations: Punctuated::new(),
			},
			output_type: parse_quote!(i32),
			is_async: false,
			fields: vec![],
			body: TokenStream2::new(),
			crate_name: FoundCrate::Itself,
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	#[should_panic(expected = "Multiple 'category' attributes are not allowed")]
	fn test_multiple_categories() {
		let attr = quote!(category("Math: Arithmetic"), category("General"));
		let input = quote!(
			fn add(a: i32, b: i32) -> i32 {
				a + b
			}
		);
		parse_node_fn(attr, input).unwrap();
	}

	#[test]
	#[should_panic(expected = "No default values for first argument allowed")]
	fn test_default_value_for_first_arg() {
		let attr = quote!(category("Invalid"));
		let input = quote!(
			fn invalid_node(#[default(())] node: impl Node<(), Output = i32>) -> i32 {
				node.eval(())
			}
		);
		parse_node_fn(attr, input).unwrap();
	}

	#[test]
	#[should_panic(expected = "No default values for `impl Node` allowed")]
	fn test_default_value_for_impl_node() {
		let attr = quote!(category("Invalid"));
		let input = quote!(
			fn invalid_node(_: (), #[default(())] node: impl Node<(), Output = i32>) -> i32 {
				node.eval(())
			}
		);
		parse_node_fn(attr, input).unwrap();
	}

	#[test]
	#[should_panic(expected = "Unsupported attribute in `node_fn`")]
	fn test_unsupported_attribute() {
		let attr = quote!(unsupported("Value"));
		let input = quote!(
			fn test_node(input: i32) -> i32 {
				input
			}
		);
		parse_node_fn(attr, input).unwrap();
	}
}
