use convert_case::{Case, Casing};
use indoc::{formatdoc, indoc};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::{Comma, RArrow};
use syn::{
	AttrStyle, Attribute, Error, Expr, ExprTuple, FnArg, GenericParam, Ident, ItemFn, Lit, LitFloat, LitInt, LitStr, Meta, Pat, PatIdent, PatType, Path, ReturnType, TraitBound, Type, TypeImplTrait,
	TypeParam, TypeParamBound, Visibility, WhereClause, parse_quote,
};

use crate::codegen::generate_node_code;
use crate::crate_ident::CrateIdent;
use crate::shader_nodes::ShaderNodeType;

#[derive(Clone, Debug)]
pub(crate) struct Implementation {
	pub(crate) input: Type,
	pub(crate) _arrow: RArrow,
	pub(crate) output: Type,
}

#[derive(Debug)]
pub(crate) struct ParsedNodeFn {
	pub(crate) vis: Visibility,
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
	pub(crate) description: String,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct NodeFnAttributes {
	pub(crate) category: Option<LitStr>,
	pub(crate) display_name: Option<LitStr>,
	pub(crate) path: Option<Path>,
	pub(crate) skip_impl: bool,
	pub(crate) properties_string: Option<LitStr>,
	/// whether to `#[cfg]` gate the node implementation, defaults to None
	pub(crate) cfg: Option<TokenStream2>,
	/// if this node should get a gpu implementation, defaults to None
	pub(crate) shader_node: Option<ShaderNodeType>,
	// Add more attributes as needed
}

#[derive(Clone, Debug, Default)]
pub enum ParsedValueSource {
	#[default]
	None,
	Default(TokenStream2),
	Scope(LitStr),
}

// #[widget(ParsedWidgetOverride::Hidden)]
// #[widget(ParsedWidgetOverride::String = "Some string")]
// #[widget(ParsedWidgetOverride::Custom = "Custom string")]
#[derive(Clone, Debug, Default)]
pub enum ParsedWidgetOverride {
	#[default]
	None,
	Hidden,
	String(LitStr),
	Custom(LitStr),
}

impl Parse for ParsedWidgetOverride {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		// Parse the full path (e.g., ParsedWidgetOverride::Hidden)
		let path: Path = input.parse()?;

		// Ensure the path starts with `ParsedWidgetOverride`
		if path.segments.len() == 2 && path.segments[0].ident == "ParsedWidgetOverride" {
			let variant = &path.segments[1].ident;

			match variant.to_string().as_str() {
				"Hidden" => Ok(ParsedWidgetOverride::Hidden),
				"String" => {
					input.parse::<syn::Token![=]>()?;
					let lit: LitStr = input.parse()?;
					Ok(ParsedWidgetOverride::String(lit))
				}
				"Custom" => {
					input.parse::<syn::Token![=]>()?;
					let lit: LitStr = input.parse()?;
					Ok(ParsedWidgetOverride::Custom(lit))
				}
				_ => Err(Error::new(variant.span(), "Unknown ParsedWidgetOverride variant")),
			}
		} else {
			Err(Error::new(input.span(), "Expected ParsedWidgetOverride::<variant>"))
		}
	}
}

#[derive(Clone, Debug)]
pub struct ParsedField {
	pub pat_ident: PatIdent,
	pub name: Option<LitStr>,
	pub description: String,
	pub widget_override: ParsedWidgetOverride,
	pub ty: ParsedFieldType,
	pub number_display_decimal_places: Option<LitInt>,
	pub number_step: Option<LitFloat>,
	pub unit: Option<LitStr>,
}

#[derive(Clone, Debug)]
pub enum ParsedFieldType {
	Regular(RegularParsedField),
	Node(NodeParsedField),
}

/// a param of any kind, either a concrete type or a generic type with a set of possible types specified via
/// `#[implementation(type)]`
#[derive(Clone, Debug)]
pub struct RegularParsedField {
	pub ty: Type,
	pub exposed: bool,
	pub value_source: ParsedValueSource,
	pub number_soft_min: Option<LitFloat>,
	pub number_soft_max: Option<LitFloat>,
	pub number_hard_min: Option<LitFloat>,
	pub number_hard_max: Option<LitFloat>,
	pub number_mode_range: Option<ExprTuple>,
	pub implementations: Punctuated<Type, Comma>,
	pub gpu_image: bool,
}

/// a param of `impl Node` with `#[implementation(in -> out)]`
#[derive(Clone, Debug)]
pub struct NodeParsedField {
	pub input_type: Type,
	pub output_type: Type,
	pub implementations: Punctuated<Implementation, Comma>,
}

#[derive(Clone, Debug)]
pub(crate) struct Input {
	pub(crate) pat_ident: PatIdent,
	pub(crate) ty: Type,
	pub(crate) implementations: Punctuated<Type, Comma>,
	pub(crate) context_features: Vec<Ident>,
}

impl Parse for Implementation {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let input_type: Type = input.parse().map_err(|e| {
			Error::new(
				input.span(),
				formatdoc!(
					"Failed to parse input type for #[implementation(...)]. Expected a valid Rust type.
					Error: {}",
					e,
				),
			)
		})?;
		let arrow: RArrow = input.parse().map_err(|_| {
			Error::new(
				input.span(),
				indoc!(
					"Expected `->` arrow after input type in #[implementations(...)] on a field of type `impl Node`.
					The correct syntax is `InputType -> OutputType`."
				),
			)
		})?;
		let output_type: Type = input.parse().map_err(|e| {
			Error::new(
				input.span(),
				formatdoc!(
					"Failed to parse output type for #[implementation(...)]. Expected a valid Rust type after `->`.
					Error: {}",
					e
				),
			)
		})?;

		Ok(Implementation {
			input: input_type,
			_arrow: arrow,
			output: output_type,
		})
	}
}

impl Parse for NodeFnAttributes {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut category = None;
		let mut display_name = None;
		let mut path = None;
		let mut skip_impl = false;
		let mut properties_string = None;
		let mut cfg = None;
		let mut shader_node = None;

		let content = input;
		// let content;
		// syn::parenthesized!(content in input);

		let nested = content.call(Punctuated::<Meta, Comma>::parse_terminated)?;
		for meta in nested {
			let name = meta.path().get_ident().ok_or_else(|| Error::new_spanned(meta.path(), "Node macro expects a known Ident, not a path"))?;
			match name.to_string().as_str() {
				"category" => {
					let meta = meta.require_list()?;
					if category.is_some() {
						return Err(Error::new_spanned(meta, "Multiple 'category' attributes are not allowed"));
					}
					let lit: LitStr = meta
						.parse_args()
						.map_err(|_| Error::new_spanned(meta, "Expected a string literal for 'category', e.g., category(\"Value\")"))?;
					category = Some(lit);
				}
				"name" => {
					let meta = meta.require_list()?;
					if display_name.is_some() {
						return Err(Error::new_spanned(meta, "Multiple 'name' attributes are not allowed"));
					}
					let parsed_name: LitStr = meta.parse_args().map_err(|_| Error::new_spanned(meta, "Expected a string for 'name', e.g., name(\"Memoize\")"))?;
					display_name = Some(parsed_name);
				}
				"path" => {
					let meta = meta.require_list()?;
					if path.is_some() {
						return Err(Error::new_spanned(meta, "Multiple 'path' attributes are not allowed"));
					}
					let parsed_path: Path = meta
						.parse_args()
						.map_err(|_| Error::new_spanned(meta, "Expected a valid path for 'path', e.g., path(crate::MemoizeNode)"))?;
					path = Some(parsed_path);
				}
				"skip_impl" => {
					let path = meta.require_path_only()?;
					if skip_impl {
						return Err(Error::new_spanned(path, "Multiple 'skip_impl' attributes are not allowed"));
					}
					skip_impl = true;
				}
				"properties" => {
					let meta = meta.require_list()?;
					if properties_string.is_some() {
						return Err(Error::new_spanned(path, "Multiple 'properties_string' attributes are not allowed"));
					}
					let parsed_properties_string: LitStr = meta
						.parse_args()
						.map_err(|_| Error::new_spanned(meta, "Expected a string for 'properties', e.g., name(\"channel_mixer_properties\")"))?;

					properties_string = Some(parsed_properties_string);
				}
				"cfg" => {
					if cfg.is_some() {
						return Err(Error::new_spanned(path, "Multiple 'feature' attributes are not allowed"));
					}
					let meta = meta.require_list()?;
					cfg = Some(meta.tokens.clone());
				}
				"shader_node" => {
					if shader_node.is_some() {
						return Err(Error::new_spanned(path, "Multiple 'feature' attributes are not allowed"));
					}
					let meta = meta.require_list()?;
					shader_node = Some(syn::parse2(meta.tokens.to_token_stream())?);
				}
				_ => {
					return Err(Error::new_spanned(
						meta,
						indoc!(
							r#"
							Unsupported attribute in `node`.
							Supported attributes are 'category', 'path' 'name', 'skip_impl', 'cfg' and 'properties'.

							Example usage:
							#[node_macro::node(category("Value"), name("Test Node"))]
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
			properties_string,
			cfg,
			shader_node,
		})
	}
}

fn parse_node_fn(attr: TokenStream2, item: TokenStream2) -> syn::Result<ParsedNodeFn> {
	let attributes = syn::parse2::<NodeFnAttributes>(attr.clone()).map_err(|e| Error::new(e.span(), format!("Failed to parse node_fn attributes: {e}")))?;
	let input_fn = syn::parse2::<ItemFn>(item.clone()).map_err(|e| Error::new(e.span(), format!("Failed to parse function: {e}. Make sure it's a valid Rust function.")))?;

	let vis = input_fn.vis;
	let fn_name = input_fn.sig.ident.clone();
	let struct_name = format_ident!("{}", fn_name.to_string().to_case(Case::Pascal));
	let mod_name = fn_name.clone();
	let fn_generics = input_fn.sig.generics.params.into_iter().collect();
	let is_async = input_fn.sig.asyncness.is_some();

	let (input, fields) = parse_inputs(&input_fn.sig.inputs)?;
	let output_type = parse_output(&input_fn.sig.output)?;
	let where_clause = input_fn.sig.generics.where_clause;
	let body = input_fn.block.to_token_stream();
	let description = input_fn
		.attrs
		.iter()
		.filter_map(|a| {
			if a.style != AttrStyle::Outer {
				return None;
			}
			let Meta::NameValue(name_val) = &a.meta else { return None };
			if name_val.path.get_ident().map(|x| x.to_string()) != Some("doc".into()) {
				return None;
			}
			let Expr::Lit(expr_lit) = &name_val.value else { return None };
			let Lit::Str(ref text) = expr_lit.lit else { return None };
			Some(text.value().trim().to_string())
		})
		.fold(String::new(), |acc, b| acc + &b + "\n");

	Ok(ParsedNodeFn {
		vis,
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
		description,
	})
}

fn parse_inputs(inputs: &Punctuated<FnArg, Comma>) -> syn::Result<(Input, Vec<ParsedField>)> {
	let mut fields = Vec::new();
	let mut input = None;

	for (index, arg) in inputs.iter().enumerate() {
		if let FnArg::Typed(PatType { pat, ty, attrs, .. }) = arg {
			// Call argument
			if index == 0 {
				if extract_attribute(attrs, "default").is_some() {
					return Err(Error::new_spanned(&attrs[0], "Call argument cannot be given a default value".to_string()));
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
				let context_features = parse_context_feature_idents(ty);
				input = Some(Input {
					pat_ident,
					ty: (**ty).clone(),
					implementations,
					context_features,
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

/// Parse context feature identifiers from the trait bounds of a context parameter.
fn parse_context_feature_idents(ty: &Type) -> Vec<Ident> {
	let mut features = Vec::new();

	// Check if this is an impl trait (impl Ctx + ...)
	if let Type::ImplTrait(TypeImplTrait { bounds, .. }) = ty {
		for bound in bounds {
			if let TypeParamBound::Trait(TraitBound { path, .. }) = bound {
				// Extract the last segment of the trait path
				if let Some(segment) = path.segments.last() {
					match segment.ident.to_string().as_str() {
						"ExtractFootprint"
						| "ExtractRealTime"
						| "ExtractAnimationTime"
						| "ExtractIndex"
						| "ExtractVarArgs"
						| "InjectFootprint"
						| "InjectRealTime"
						| "InjectAnimationTime"
						| "InjectIndex"
						| "InjectVarArgs" => {
							features.push(segment.ident.clone());
						}
						// Skip Modify* traits as they don't affect usage tracking
						// Also ignore other traits like Ctx, ExtractAll, etc.
						_ => {}
					}
				}
			}
		}
	}

	features
}

fn parse_implementations(attr: &Attribute, name: &Ident) -> syn::Result<Punctuated<Type, Comma>> {
	let content: TokenStream2 = attr.parse_args()?;
	let parser = Punctuated::<Type, Comma>::parse_terminated;
	parser.parse2(content.clone()).map_err(|e| {
		let span = e.span(); // Get the span of the error
		Error::new(span, format!("Failed to parse implementations for argument '{name}': {e}"))
	})
}

fn parse_node_implementations<T: Parse>(attr: &Attribute, name: &Ident) -> syn::Result<Punctuated<T, Comma>> {
	let content: TokenStream2 = attr.parse_args()?;
	let parser = Punctuated::<T, Comma>::parse_terminated;
	parser.parse2(content.clone()).map_err(|e| {
		Error::new(
			e.span(),
			formatdoc!(
				"Invalid #[implementations(...)] for argument `{}`.
				Expected a comma-separated list of `InputType -> OutputType` pairs.
				Example: #[implementations(i32 -> f64, String -> Vec<u8>)]
				Error: {}",
				name,
				e
			),
		)
	})
}

fn parse_field(pat_ident: PatIdent, ty: Type, attrs: &[Attribute]) -> syn::Result<ParsedField> {
	let ident = &pat_ident.ident;

	let default_value = extract_attribute(attrs, "default")
		.map(|attr| attr.parse_args().map_err(|e| Error::new_spanned(attr, format!("Invalid `default` value for argument '{ident}': {e}"))))
		.transpose()?;

	let scope = extract_attribute(attrs, "scope")
		.map(|attr| attr.parse_args().map_err(|e| Error::new_spanned(attr, format!("Invalid `scope` value for argument '{ident}': {e}"))))
		.transpose()?;

	let name = extract_attribute(attrs, "name")
		.map(|attr| attr.parse_args().map_err(|e| Error::new_spanned(attr, format!("Invalid `name` value for argument '{ident}': {e}"))))
		.transpose()?;

	let widget_override = extract_attribute(attrs, "widget")
		.map(|attr| {
			attr.parse_args()
				.map_err(|e| Error::new_spanned(attr, format!("Invalid `widget override` value for argument '{ident}': {e}")))
		})
		.transpose()?
		.unwrap_or_default();

	let exposed = extract_attribute(attrs, "expose").is_some();

	let value_source = match (default_value, scope) {
		(Some(_), Some(_)) => return Err(Error::new_spanned(&pat_ident, "Cannot have both `default` and `scope` attributes")),
		(Some(default_value), _) => ParsedValueSource::Default(default_value),
		(_, Some(scope)) => ParsedValueSource::Scope(scope),
		_ => ParsedValueSource::None,
	};

	let number_soft_min = extract_attribute(attrs, "soft_min")
		.map(|attr| {
			attr.parse_args()
				.map_err(|e| Error::new_spanned(attr, format!("Invalid numerical `soft_min` value for argument '{ident}': {e}")))
		})
		.transpose()?;
	let number_soft_max = extract_attribute(attrs, "soft_max")
		.map(|attr| {
			attr.parse_args()
				.map_err(|e| Error::new_spanned(attr, format!("Invalid numerical `soft_max` value for argument '{ident}': {e}")))
		})
		.transpose()?;

	let number_hard_min = extract_attribute(attrs, "hard_min")
		.map(|attr| {
			attr.parse_args()
				.map_err(|e| Error::new_spanned(attr, format!("Invalid numerical `hard_min` value for argument '{ident}': {e}")))
		})
		.transpose()?;
	let number_hard_max = extract_attribute(attrs, "hard_max")
		.map(|attr| {
			attr.parse_args()
				.map_err(|e| Error::new_spanned(attr, format!("Invalid numerical `hard_max` value for argument '{ident}': {e}")))
		})
		.transpose()?;

	let number_mode_range = extract_attribute(attrs, "range")
		.map(|attr| {
			attr.parse_args::<ExprTuple>().map_err(|e| {
				Error::new_spanned(
					attr,
					format!("Invalid `range` tuple of min and max range slider values for argument '{ident}': {e}\nUSAGE EXAMPLE: #[range((0., 100.))]"),
				)
			})
		})
		.transpose()?;
	if let Some(range) = &number_mode_range {
		if range.elems.len() != 2 {
			return Err(Error::new_spanned(range, "Expected a tuple of two values for `range` for the min and max, respectively"));
		}
	}

	let unit = extract_attribute(attrs, "unit")
		.map(|attr| attr.parse_args::<LitStr>().map_err(|_e| Error::new_spanned(attr, "Expected a unit type as string".to_string())))
		.transpose()?;

	let number_display_decimal_places = extract_attribute(attrs, "display_decimal_places")
		.map(|attr| {
			attr.parse_args::<LitInt>().map_err(|e| {
				Error::new_spanned(
					attr,
					format!("Invalid `integer` for number of decimals for argument '{ident}': {e}\nUSAGE EXAMPLE: #[display_decimal_places(2)]"),
				)
			})
		})
		.transpose()?
		.map(|f| {
			if let Err(e) = f.base10_parse::<u32>() {
				Err(Error::new_spanned(f, format!("Expected a `u32` for `display_decimal_places` for '{ident}': {e}")))
			} else {
				Ok(f)
			}
		})
		.transpose()?;
	let number_step = extract_attribute(attrs, "step")
		.map(|attr| {
			attr.parse_args::<LitFloat>()
				.map_err(|e| Error::new_spanned(attr, format!("Invalid `step` for argument '{ident}': {e}\nUSAGE EXAMPLE: #[step(2.)]")))
		})
		.transpose()?;
	let gpu_image = extract_attribute(attrs, "gpu_image").is_some();

	let (is_node, node_input_type, node_output_type) = parse_node_type(&ty);
	let description = attrs
		.iter()
		.filter_map(|a| {
			if a.style != AttrStyle::Outer {
				return None;
			}
			let Meta::NameValue(name_val) = &a.meta else { return None };
			if name_val.path.get_ident().map(|x| x.to_string()) != Some("doc".into()) {
				return None;
			}
			let Expr::Lit(expr_lit) = &name_val.value else { return None };
			let Lit::Str(ref text) = expr_lit.lit else { return None };
			Some(text.value().trim().to_string())
		})
		.fold(String::new(), |acc, b| acc + &b + "\n");

	if is_node {
		let (input_type, output_type) = node_input_type
			.zip(node_output_type)
			.ok_or_else(|| Error::new_spanned(&ty, "Invalid Node type. Expected `impl Node<Input, Output = OutputType>`"))?;
		if !matches!(&value_source, ParsedValueSource::None) {
			return Err(Error::new_spanned(&ty, "No default values for `impl Node` allowed"));
		}
		let implementations = extract_attribute(attrs, "implementations")
			.map(|attr| parse_node_implementations(attr, ident))
			.transpose()?
			.unwrap_or_default();

		Ok(ParsedField {
			pat_ident,
			ty: ParsedFieldType::Node(NodeParsedField {
				input_type,
				output_type,
				implementations,
			}),
			name,
			description,
			widget_override,
			number_display_decimal_places,
			number_step,
			unit,
		})
	} else {
		let implementations = extract_attribute(attrs, "implementations")
			.map(|attr| parse_implementations(attr, ident))
			.transpose()?
			.unwrap_or_default();
		Ok(ParsedField {
			pat_ident,
			ty: ParsedFieldType::Regular(RegularParsedField {
				exposed,
				number_soft_min,
				number_soft_max,
				number_hard_min,
				number_hard_max,
				number_mode_range,
				ty,
				value_source,
				implementations,
				gpu_image,
			}),
			name,
			description,
			widget_override,
			number_display_decimal_places,
			number_step,
			unit,
		})
	}
}

fn parse_node_type(ty: &Type) -> (bool, Option<Type>, Option<Type>) {
	if let Type::ImplTrait(impl_trait) = ty {
		for bound in &impl_trait.bounds {
			if let syn::TypeParamBound::Trait(trait_bound) = bound {
				if trait_bound.path.segments.last().is_some_and(|seg| seg.ident == "Node") {
					if let syn::PathArguments::AngleBracketed(args) = &trait_bound.path.segments.last().unwrap().arguments {
						let input_type = args.args.iter().find_map(|arg| if let syn::GenericArgument::Type(ty) = arg { Some(ty.clone()) } else { None });
						let output_type = args.args.iter().find_map(|arg| {
							if let syn::GenericArgument::AssocType(assoc_type) = arg {
								if assoc_type.ident == "Output" { Some(assoc_type.ty.clone()) } else { None }
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
pub fn new_node_fn(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
	let crate_ident = CrateIdent::default();
	let mut parsed_node = parse_node_fn(attr, item.clone()).map_err(|e| Error::new(e.span(), format!("Failed to parse node function: {e}")))?;
	parsed_node.replace_impl_trait_in_input();
	crate::validation::validate_node_fn(&parsed_node).map_err(|e| Error::new(e.span(), format!("Validation Error: {e}")))?;
	generate_node_code(&crate_ident, &parsed_node).map_err(|e| Error::new(e.span(), format!("Failed to generate node code: {e}")))
}

impl ParsedNodeFn {
	pub fn replace_impl_trait_in_input(&mut self) {
		if let Type::ImplTrait(impl_trait) = self.input.ty.clone() {
			let ident = Ident::new("_Input", impl_trait.span());
			let mut bounds = impl_trait.bounds;
			bounds.push(parse_quote!('n));
			self.fn_generics.push(GenericParam::Type(TypeParam {
				attrs: Default::default(),
				ident: ident.clone(),
				colon_token: Some(Default::default()),
				bounds,
				eq_token: None,
				default: None,
			}));
			self.input.ty = parse_quote!(#ident);
			if self.input.implementations.is_empty() {
				self.input.implementations.push(parse_quote!(gcore::Context));
			}
		}
		if self.input.pat_ident.ident == "_" {
			self.input.pat_ident.ident = Ident::new("__ctx", self.input.pat_ident.ident.span());
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use proc_macro2::Span;
	use quote::{quote, quote_spanned};
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
		assert_eq!(parsed.description, expected.description);

		for (parsed_field, expected_field) in parsed.fields.iter().zip(expected.fields.iter()) {
			match (parsed_field, expected_field) {
				(
					ParsedField {
						pat_ident: p_name,
						ty: ParsedFieldType::Regular(RegularParsedField {
							ty: p_ty,
							exposed: p_exp,
							value_source: p_default,
							..
						}),
						..
					},
					ParsedField {
						pat_ident: e_name,
						ty: ParsedFieldType::Regular(RegularParsedField {
							ty: e_ty,
							exposed: e_exp,
							value_source: e_default,
							..
						}),
						..
					},
				) => {
					assert_eq!(p_name, e_name);
					assert_eq!(p_exp, e_exp);
					match (p_default, e_default) {
						(ParsedValueSource::None, ParsedValueSource::None) => {}
						(ParsedValueSource::Default(p), ParsedValueSource::Default(e)) => {
							assert_eq!(p.to_token_stream().to_string(), e.to_token_stream().to_string());
						}
						(ParsedValueSource::Scope(p), ParsedValueSource::Scope(e)) => {
							assert_eq!(p.value(), e.value());
						}
						_ => panic!("Mismatched default values"),
					}
					assert_eq!(format!("{p_ty:?}"), format!("{:?}", e_ty));
				}
				(
					ParsedField {
						pat_ident: p_name,
						ty: ParsedFieldType::Node(NodeParsedField {
							input_type: p_input,
							output_type: p_output,
							..
						}),
						..
					},
					ParsedField {
						pat_ident: e_name,
						ty: ParsedFieldType::Node(NodeParsedField {
							input_type: e_input,
							output_type: e_output,
							..
						}),
						..
					},
				) => {
					assert_eq!(p_name, e_name);
					assert_eq!(format!("{p_input:?}"), format!("{:?}", e_input));
					assert_eq!(format!("{p_output:?}"), format!("{:?}", e_output));
				}
				_ => panic!("Mismatched field types"),
			}
		}
	}

	#[test]
	fn test_basic_node() {
		let attr = quote!(category("Math: Arithmetic"), path(graphene_core::TestNode), skip_impl);
		let input = quote!(
			/// Multi
			/// Line
			fn add(a: f64, b: f64) -> f64 {
				a + b
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Math: Arithmetic")),
				display_name: None,
				path: Some(parse_quote!(graphene_core::TestNode)),
				skip_impl: true,
				properties_string: None,
				cfg: None,
				shader_node: None,
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
				context_features: vec![],
			},
			output_type: parse_quote!(f64),
			is_async: false,
			fields: vec![ParsedField {
				pat_ident: pat_ident("b"),
				name: None,
				description: String::new(),
				widget_override: ParsedWidgetOverride::None,
				ty: ParsedFieldType::Regular(RegularParsedField {
					ty: parse_quote!(f64),
					exposed: false,
					value_source: ParsedValueSource::None,
					number_soft_min: None,
					number_soft_max: None,
					number_hard_min: None,
					number_hard_max: None,
					number_mode_range: None,
					implementations: Punctuated::new(),
					gpu_image: false,
				}),
				number_display_decimal_places: None,
				number_step: None,
				unit: None,
			}],
			body: TokenStream2::new(),
			description: String::from("Multi\nLine\n"),
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_impl_node() {
		let attr = quote!(category("General"));
		let input = quote!(
			/**
				Hello
				World
			*/
			fn transform<T: 'static>(footprint: Footprint, transform_target: impl Node<Footprint, Output = T>, translate: DVec2) -> T {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("General")),
				display_name: None,
				path: None,
				skip_impl: false,
				properties_string: None,
				cfg: None,
				shader_node: None,
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
				context_features: vec![],
			},
			output_type: parse_quote!(T),
			is_async: false,
			fields: vec![
				ParsedField {
					pat_ident: pat_ident("transform_target"),
					name: None,
					description: String::new(),
					widget_override: ParsedWidgetOverride::None,
					ty: ParsedFieldType::Node(NodeParsedField {
						input_type: parse_quote!(Footprint),
						output_type: parse_quote!(T),
						implementations: Punctuated::new(),
					}),
					number_display_decimal_places: None,
					number_step: None,
					unit: None,
				},
				ParsedField {
					pat_ident: pat_ident("translate"),
					name: None,
					description: String::new(),
					widget_override: ParsedWidgetOverride::None,
					ty: ParsedFieldType::Regular(RegularParsedField {
						ty: parse_quote!(DVec2),
						exposed: false,
						value_source: ParsedValueSource::None,
						number_soft_min: None,
						number_soft_max: None,
						number_hard_min: None,
						number_hard_max: None,
						number_mode_range: None,
						implementations: Punctuated::new(),
						gpu_image: false,
					}),
					number_display_decimal_places: None,
					number_step: None,
					unit: None,
				},
			],
			body: TokenStream2::new(),
			description: String::from("Hello\n\t\t\t\tWorld\n"),
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_default_values() {
		let attr = quote!(category("Vector: Shape"));
		let input = quote!(
			/// Test
			fn circle(_: impl Ctx + ExtractFootprint, #[default(50.)] radius: f64) -> Vector {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Vector: Shape")),
				display_name: None,
				path: None,
				skip_impl: false,
				properties_string: None,
				cfg: None,
				shader_node: None,
			},
			fn_name: Ident::new("circle", Span::call_site()),
			struct_name: Ident::new("Circle", Span::call_site()),
			mod_name: Ident::new("circle", Span::call_site()),
			fn_generics: vec![],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("_"),
				ty: parse_quote!(impl Ctx + ExtractFootprint),
				implementations: Punctuated::new(),
				context_features: vec![format_ident!("ExtractFootprint")],
			},
			output_type: parse_quote!(Vector),
			is_async: false,
			fields: vec![ParsedField {
				pat_ident: pat_ident("radius"),
				name: None,
				description: String::new(),
				widget_override: ParsedWidgetOverride::None,
				ty: ParsedFieldType::Regular(RegularParsedField {
					ty: parse_quote!(f64),
					exposed: false,
					value_source: ParsedValueSource::Default(quote!(50.)),
					number_soft_min: None,
					number_soft_max: None,
					number_hard_min: None,
					number_hard_max: None,
					number_mode_range: None,
					implementations: Punctuated::new(),
					gpu_image: false,
				}),
				number_display_decimal_places: None,
				number_step: None,
				unit: None,
			}],
			body: TokenStream2::new(),
			description: "Test\n".into(),
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_node_with_implementations() {
		let attr = quote!(category("Raster: Adjustment"));
		let input = quote!(
			fn levels<P: Pixel>(image: Table<Raster<P>>, #[implementations(f32, f64)] shadows: f64) -> Table<Raster<P>> {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Raster: Adjustment")),
				display_name: None,
				path: None,
				skip_impl: false,
				properties_string: None,
				cfg: None,
				shader_node: None,
			},
			fn_name: Ident::new("levels", Span::call_site()),
			struct_name: Ident::new("Levels", Span::call_site()),
			mod_name: Ident::new("levels", Span::call_site()),
			fn_generics: vec![parse_quote!(P: Pixel)],
			where_clause: None,
			input: Input {
				pat_ident: pat_ident("image"),
				ty: parse_quote!(Table<Raster<P>>),
				implementations: Punctuated::new(),
				context_features: vec![],
			},
			output_type: parse_quote!(Table<Raster<P>>),
			is_async: false,
			fields: vec![ParsedField {
				pat_ident: pat_ident("shadows"),
				name: None,
				description: String::new(),
				widget_override: ParsedWidgetOverride::None,
				ty: ParsedFieldType::Regular(RegularParsedField {
					ty: parse_quote!(f64),
					exposed: false,
					value_source: ParsedValueSource::None,
					number_soft_min: None,
					number_soft_max: None,
					number_hard_min: None,
					number_hard_max: None,
					number_mode_range: None,
					implementations: {
						let mut p = Punctuated::new();
						p.push(parse_quote!(f32));
						p.push(parse_quote!(f64));
						p
					},
					gpu_image: false,
				}),
				number_display_decimal_places: None,
				number_step: None,
				unit: None,
			}],
			body: TokenStream2::new(),
			description: String::new(),
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_number_min_max_range_mode() {
		let attr = quote!(category("Math: Arithmetic"), path(graphene_core::TestNode));
		let input = quote!(
			fn add(
				a: f64,
				/// b
				#[range((0., 100.))]
				#[soft_min(-500.)]
				#[soft_max(500.)]
				b: f64,
			) -> f64 {
				a + b
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Math: Arithmetic")),
				display_name: None,
				path: Some(parse_quote!(graphene_core::TestNode)),
				skip_impl: false,
				properties_string: None,
				cfg: None,
				shader_node: None,
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
				context_features: vec![],
			},
			output_type: parse_quote!(f64),
			is_async: false,
			fields: vec![ParsedField {
				pat_ident: pat_ident("b"),
				name: None,
				description: String::from("b"),
				widget_override: ParsedWidgetOverride::None,
				ty: ParsedFieldType::Regular(RegularParsedField {
					ty: parse_quote!(f64),
					exposed: false,
					value_source: ParsedValueSource::None,
					number_soft_min: Some(parse_quote!(-500.)),
					number_soft_max: Some(parse_quote!(500.)),
					number_hard_min: None,
					number_hard_max: None,
					number_mode_range: Some(parse_quote!((0., 100.))),
					implementations: Punctuated::new(),
					gpu_image: false,
				}),
				number_display_decimal_places: None,
				number_step: None,
				unit: None,
			}],
			body: TokenStream2::new(),
			description: String::new(),
		};

		assert_parsed_node_fn(&parsed, &expected);
	}

	#[test]
	fn test_async_node() {
		let attr = quote!(category("IO"));
		let input = quote!(
			async fn load_image(api: &WasmEditorApi, #[expose] path: String) -> Table<Raster<CPU>> {
				// Implementation details...
			}
		);

		let parsed = parse_node_fn(attr, input).unwrap();
		let expected = ParsedNodeFn {
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("IO")),
				display_name: None,
				path: None,
				skip_impl: false,
				properties_string: None,
				cfg: None,
				shader_node: None,
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
				context_features: vec![],
			},
			output_type: parse_quote!(Table<Raster<CPU>>),
			is_async: true,
			fields: vec![ParsedField {
				pat_ident: pat_ident("path"),
				name: None,
				description: String::new(),
				widget_override: ParsedWidgetOverride::None,
				ty: ParsedFieldType::Regular(RegularParsedField {
					ty: parse_quote!(String),
					exposed: true,
					value_source: ParsedValueSource::None,
					number_soft_min: None,
					number_soft_max: None,
					number_hard_min: None,
					number_hard_max: None,
					number_mode_range: None,
					implementations: Punctuated::new(),
					gpu_image: false,
				}),
				number_display_decimal_places: None,
				number_step: None,
				unit: None,
			}],
			body: TokenStream2::new(),
			description: String::new(),
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
			vis: Visibility::Inherited,
			attributes: NodeFnAttributes {
				category: Some(parse_quote!("Custom")),
				display_name: Some(parse_quote!("CustomNode2")),
				path: None,
				skip_impl: false,
				properties_string: None,
				cfg: None,
				shader_node: None,
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
				context_features: vec![],
			},
			output_type: parse_quote!(i32),
			is_async: false,
			fields: vec![],
			body: TokenStream2::new(),
			description: String::new(),
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
	#[should_panic(expected = "Call argument cannot be given a default value")]
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
	#[should_panic(expected = "Unsupported attribute in `node`")]
	fn test_unsupported_attribute() {
		let attr = quote!(unsupported("Value"));
		let input = quote!(
			fn test_node(input: i32) -> i32 {
				input
			}
		);
		parse_node_fn(attr, input).unwrap();
	}

	#[test]
	fn test_invalid_implementation_syntax() {
		let attr = quote!(category("Test"));
		let input = quote!(
			fn test_node(_: (), #[implementations((Footprint, Color), (Footprint, Table<Raster<CPU>>))] input: impl Node<Footprint, Output = T>) -> T {
				// Implementation details...
			}
		);

		let result = parse_node_fn(attr, input);
		assert!(result.is_err());
		let error = result.unwrap_err();
		let error_message = error.to_string();
		assert!(error_message.contains("Invalid #[implementations(...)] for argument `input`"));
		assert!(error_message.contains("Expected a comma-separated list of `InputType -> OutputType` pairs"));
		assert!(error_message.contains("Expected `->` arrow after input type in #[implementations(...)] on a field of type `impl Node`"));
	}

	#[test]
	fn test_implementation_on_first_arg() {
		let attr = quote!(category("Test"));

		// Use quote_spanned! to attach a specific span to the problematic part
		let problem_span = Span::call_site(); // You could create a custom span here if needed
		let tuples = quote_spanned!(problem_span=> () ());
		let input = quote! {
			fn test_node(
				#[implementations((), #tuples, Footprint)]
				footprint: F,
				#[implementations(
					() -> Table<Raster<CPU>>,
					() -> Table<Color>,
					() -> Table<GradientStops>,
					() -> GradientStops,
					Footprint -> Table<Raster<CPU>>,
					Footprint -> Table<Color>,
					Footprint -> Table<GradientStops>,
					Footprint -> GradientStops,
				)]
				image: impl Node<F, Output = T>,
			) -> T {
				// Implementation details...
			}
		};

		let result = parse_node_fn(attr, input);
		assert!(result.is_err(), "Expected an error, but parsing succeeded");

		let error = result.unwrap_err();
		let error_string = error.to_string();
		assert!(error_string.contains("Failed to parse implementations for argument 'footprint'"));
		assert!(error_string.contains("expected `,`"));

		// Instead of checking for exact line and column,
		// verify that the error span is the one we specified
		assert_eq!(error.span().start(), problem_span.start());
	}
}
