//! The `#[graph]` macro, which defines a node as a subgraph written in the graph text format's Rust syntax.
//!
//! The function's parameters become the subgraph's imports, `out!(...)` names its exports, and the body wires nodes
//! together with `>>`. The macro checks the body's shape, then generates code that registers the subgraph's metadata at
//! startup, as the node macro does for proto nodes. Each node, parameter, and output name resolves through the per-node
//! module the node macro (or this macro) generates, so rustc reports a misspelled name at the token the author wrote.

use crate::codegen::{NODE_ID, derived_display_name, field_metadata_tokens, option_tokens, symbol_name};
use crate::crate_ident::CrateIdent;
use crate::parsing::{ParsedField, ParsedFieldType, parse_field};
use crate::validation::{validate_name_isnt_a_value_helper, validate_parameters};
use convert_case::{Case, Casing};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use std::collections::HashMap;
use syn::ext::IdentExt;
use syn::parse::{ParseStream, Parser};
use syn::spanned::Spanned;
use syn::{Attribute, BinOp, Error, Expr, ExprCall, ExprMethodCall, FnArg, Ident, Item, ItemFn, Lit, LitInt, LitStr, Meta, Pat, ReturnType, Stmt, Token, Type, UnOp};

/// The calls a graph body reads as values or input sources rather than as nodes, so no node can share one's name.
pub(crate) const VALUE_HELPERS: &[&str] = &["rgb", "rgba", "scope", "reflect", "exposed", "unexposed"];

/// The parameter attributes an import takes, which are those describing its value and widget rather than a proto node's implementation.
const IMPORT_ATTRIBUTES: &[&str] = &[
	"doc",
	"name",
	"expose",
	"default",
	"widget",
	"unit",
	"hard",
	"soft",
	"range",
	"step",
	"display_decimal_places",
	"multiline",
	"progression",
	"labels",
	"integer",
	"blank_assist",
];

pub fn new_graph_fn(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
	let function: ItemFn = syn::parse2(item)?;
	let attributes = parse_graph_attributes(attr);

	// The symbols naming the graph stay defined while it has errors, so those errors don't also break every use of it
	let display_name = attributes.as_ref().ok().and_then(|attributes| attributes.display_name.as_ref());
	let symbols = symbol_module(&function, &graph_identifier(&function.sig.ident, display_name));
	proc_macro_error3::set_dummy(symbols.clone());

	let crate_ident = CrateIdent::default();
	let core_types = crate_ident.gcore()?.clone();

	let graph = match attributes.and_then(|attributes| Graph::parse(&function, attributes, &core_types)) {
		Ok(graph) => graph,
		Err(error) => {
			let error = error.to_compile_error();
			return Ok(quote!(#error #symbols));
		}
	};
	validate_parameters(&graph.imports.iter().map(|import| import.field.clone()).collect::<Vec<_>>());
	validate_name_isnt_a_value_helper(&function.sig.ident);

	Ok(graph.generate(&function, &core_types, symbols))
}

/// The attributes inside `#[graph(...)]`.
#[derive(Default)]
struct GraphAttributes {
	category: Option<LitStr>,
	icon: Option<LitStr>,
	display_name: Option<LitStr>,
	version: Option<u32>,
	layer: bool,
}

fn parse_graph_attributes(attr: TokenStream2) -> syn::Result<GraphAttributes> {
	let mut attributes = GraphAttributes::default();

	let parser = syn::meta::parser(|meta| {
		let string_argument = |meta: &syn::meta::ParseNestedMeta| -> syn::Result<LitStr> {
			let content;
			syn::parenthesized!(content in meta.input);
			content.parse()
		};
		let repeated = |name: &str| meta.error(format!("Multiple '{name}' attributes are not allowed"));

		if meta.path.is_ident("category") {
			if attributes.category.is_some() {
				return Err(repeated("category"));
			}
			attributes.category = Some(string_argument(&meta)?);
		} else if meta.path.is_ident("icon") {
			if attributes.icon.is_some() {
				return Err(repeated("icon"));
			}
			attributes.icon = Some(string_argument(&meta)?);
		} else if meta.path.is_ident("name") {
			if attributes.display_name.is_some() {
				return Err(repeated("name"));
			}
			attributes.display_name = Some(string_argument(&meta)?);
		} else if meta.path.is_ident("version") {
			if attributes.version.is_some() {
				return Err(repeated("version"));
			}
			let content;
			syn::parenthesized!(content in meta.input);
			let literal: LitInt = content.parse()?;
			let version: u32 = literal.base10_parse()?;
			if version == 0 {
				return Err(Error::new_spanned(literal, "Versions start at 1"));
			}
			attributes.version = Some(version);
		} else if meta.path.is_ident("layer") {
			if attributes.layer {
				return Err(repeated("layer"));
			}
			attributes.layer = true;
		} else {
			return Err(meta.error("Unknown `#[graph]` attribute. Expected `category(\"…\")`, `icon(\"…\")`, `name(\"…\")`, `version(N)`, or `layer`"));
		}
		Ok(())
	});
	parser.parse2(attr)?;

	if attributes.category.is_none() {
		return Err(Error::new(Span::call_site(), "A `#[graph]` needs a category, like `#[graph(category(\"Vector: Deform\"))]`"));
	}

	Ok(attributes)
}

/// The `///` lines of an item, each trimmed and joined by line breaks.
fn doc_comment(attributes: &[Attribute]) -> String {
	attributes
		.iter()
		.filter_map(|attribute| {
			let Meta::NameValue(name_value) = &attribute.meta else { return None };
			if !name_value.path.is_ident("doc") {
				return None;
			}
			let Expr::Lit(expression) = &name_value.value else { return None };
			let Lit::Str(text) = &expression.lit else { return None };
			Some(text.value().trim().to_string())
		})
		.collect::<Vec<_>>()
		.join("\n")
		.trim()
		.to_string()
}

/// The `Type` expression for a type written in a signature.
fn type_tokens(ty: &Type, core_types: &TokenStream2) -> syn::Result<TokenStream2> {
	let span = ty.span();

	match ty {
		Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(quote_spanned!(span=> #core_types::concrete!(()))),
		Type::Paren(inner) => type_tokens(&inner.elem, core_types),
		Type::FnPtr(function) => {
			if function.inputs.len() > 1 || function.variadic.is_some() || function.abi.is_some() || function.unsafety.is_some() || function.lifetimes.is_some() {
				return Err(Error::new_spanned(function, "A function type takes at most one input, like `fn(Context) -> Item<f64>`"));
			}

			let input = match function.inputs.first() {
				Some(input) => type_tokens(&input.ty, core_types)?,
				None => quote!(#core_types::concrete!(())),
			};
			let output = match &function.output {
				ReturnType::Type(_, output) => type_tokens(output.as_ref(), core_types)?,
				ReturnType::Default => quote!(#core_types::concrete!(())),
			};
			Ok(quote_spanned!(span=> #core_types::Type::Fn(::std::boxed::Box::new(#input), ::std::boxed::Box::new(#output))))
		}
		// Nothing matches a graph's generic types by name, so every `impl Any` takes the same one
		Type::ImplTrait(_) if is_any_type(ty) => Ok(quote_spanned!(span=> #core_types::generic!(T))),
		Type::ImplTrait(_) => Err(Error::new_spanned(ty, "A generic type is written `impl Any`")),
		Type::Path(path) if path.qself.is_none() => {
			let Some(last) = path.path.segments.last() else {
				return Ok(quote_spanned!(span=> #core_types::concrete!(#ty)));
			};

			if is_lone_capital(path) {
				return Err(Error::new_spanned(ty, format!("A generic type is written `impl Any`, rather than a name like `{}`", last.ident)));
			}
			let name = last.ident.to_string();

			// The element type recurses, so a generic element like the one in `List<impl Any>` stays generic
			if matches!(name.as_str(), "Item" | "List")
				&& let syn::PathArguments::AngleBracketed(arguments) = &last.arguments
			{
				let [syn::GenericArgument::Type(element)] = arguments.args.iter().collect::<Vec<_>>().as_slice() else {
					return Err(Error::new_spanned(arguments, format!("`{name}` takes one element type, like `{name}<f64>`")));
				};
				let element = type_tokens(element, core_types)?;

				return Ok(match name.as_str() {
					"Item" => quote_spanned!(span=> #core_types::Type::Item(::std::boxed::Box::new(#element))),
					_ => quote_spanned!(span=> #core_types::Type::List(::std::boxed::Box::new(#element))),
				});
			}

			Ok(quote_spanned!(span=> #core_types::concrete!(#ty)))
		}
		_ => Ok(quote_spanned!(span=> #core_types::concrete!(#ty))),
	}
}

/// The graph's identifier and display name, from `#[graph(name("…"))]` or else its function name in Title Case.
fn graph_identifier(ident: &Ident, display_name: Option<&LitStr>) -> String {
	match display_name {
		Some(name) => name.value(),
		None => ident.unraw().to_string().to_case(Case::Pascal).to_case(Case::Title),
	}
}

/// The `#[cfg(...)]` attributes on the function, which apply to everything generated from it.
fn cfg_attributes(function: &ItemFn) -> Vec<&Attribute> {
	function.attrs.iter().filter(|attribute| attribute.path().is_ident("cfg")).collect()
}

/// The module named after the graph, holding its identifier, its parameter count, and the symbols naming its inputs and outputs.
fn symbol_module(function: &ItemFn, identifier: &str) -> TokenStream2 {
	let visibility = &function.vis;
	let ident = &function.sig.ident;
	let cfg_attributes = cfg_attributes(function);
	let module_doc = format!("The identifier, parameter, and output symbols of the **{identifier}** graph.");
	let parameter_count = function.sig.inputs.len();

	let parameter_structs = function.sig.inputs.iter().enumerate().filter_map(|(index, argument)| {
		let FnArg::Typed(typed) = argument else { return None };
		let Pat::Ident(pattern) = &*typed.pat else { return None };

		let struct_name = symbol_name(&pattern.ident, "Input");
		Some(quote! {
			pub struct #struct_name;
			impl #struct_name {
				/// Position of this parameter among the graph's inputs.
				pub const INDEX: usize = #index;
			}
		})
	});

	// A signature with errors names no outputs, and `Graph::parse` reports the errors
	let exports = parse_exports(&function.sig.output).unwrap_or_default();
	let output_structs = exports.iter().enumerate().map(|(index, export)| {
		let struct_name = symbol_name(&export.ident, "Output");
		quote! {
			pub struct #struct_name;
			impl #struct_name {
				/// Position of this export among the graph's outputs.
				pub const INDEX: usize = #index;
			}
		}
	});

	quote! {
		#(#cfg_attributes)*
		#[doc = #module_doc]
		#visibility mod #ident {
			/// The graph's identifier in the node catalog.
			pub const IDENTIFIER: &str = #identifier;
			/// How many parameters (imports) the graph has.
			pub const PARAMETER_COUNT: usize = #parameter_count;

			#(#parameter_structs)*

			#(#output_structs)*
		}
	}
}

/// One of the subgraph's imports, from a function parameter.
struct Import {
	ident: Ident,
	field: ParsedField,
	/// The type the import was declared with, which the wires from it record.
	ty: TokenStream2,
	/// The import's `FieldMetadata`, built as the node macro builds a proto node parameter's.
	metadata: TokenStream2,
}

/// One of the subgraph's exports, from a name in `out!(...)`.
struct Export {
	ident: Ident,
	name: String,
	description: String,
}

/// What feeds an input or export, as the macro tracks it while reading the body.
#[derive(Clone)]
enum Feed {
	/// A node's output, chosen by name like `.captures`, or else its primary output.
	Node {
		node: usize,
		output: Option<Ident>,
	},
	Import {
		index: usize,
		ty: Option<TokenStream2>,
	},
	Value {
		value: TokenStream2,
		exposed: Option<bool>,
	},
	/// The scope key, as a string literal or a node module's `IDENTIFIER`, as `#[scope(...)]` takes it.
	Scope(Expr),
	NodePath,
	SourceId,
}

/// Which input of a node a definition sets.
enum Slot {
	Primary,
	Named(Ident),
}

/// A node created by a call in the body.
struct Node {
	/// The module the node macro (or this macro) generates for the called node, which names its identifier and parameters.
	module: syn::Path,
	inputs: Vec<(Slot, Feed)>,
	primary_span: Option<Span>,
	position: Option<(i32, i32)>,
	layer: bool,
	display_name: Option<LitStr>,
	hidden: bool,
	locked: bool,
	pinned: bool,
	call_argument: Option<TokenStream2>,
}

struct Graph {
	ident: Ident,
	identifier: String,
	attributes: GraphAttributes,
	description: String,
	imports: Vec<Import>,
	exports: Vec<Export>,
	nodes: Vec<Node>,
	export_sources: Vec<Feed>,
	/// Items written after the `return`, such as constants, which the generated code keeps in scope for the values that use them.
	items: Vec<Item>,
}

/// Reads the body, building the subgraph's nodes and wires.
struct BodyReader<'a> {
	core_types: &'a TokenStream2,
	/// The function name of the graph being read, which its body can't call.
	graph_ident: &'a Ident,
	imports: &'a [Import],
	bindings: HashMap<String, Feed>,
	nodes: Vec<Node>,
}

impl Graph {
	fn parse(function: &ItemFn, attributes: GraphAttributes, core_types: &TokenStream2) -> syn::Result<Self> {
		let signature = &function.sig;
		if let Some(asyncness) = &signature.asyncness {
			return Err(Error::new_spanned(asyncness, "A graph can't be `async`, since the nodes inside it decide how they run"));
		}
		if !signature.generics.params.is_empty() {
			return Err(Error::new_spanned(&signature.generics, "A graph doesn't declare generics. Write a generic type as `impl Any`"));
		}
		if let Some(where_clause) = &signature.generics.where_clause {
			return Err(Error::new_spanned(where_clause, "A graph can't have a `where` clause"));
		}
		if signature.constness.is_some() || !matches!(signature.safety, syn::Safety::Default) || signature.abi.is_some() {
			return Err(Error::new_spanned(signature.fn_token, "A graph is written as a plain `fn`"));
		}

		// Any other attribute would be dropped, since the function itself isn't emitted
		if let Some(attribute) = function.attrs.iter().find(|attribute| !attribute.path().is_ident("doc") && !attribute.path().is_ident("cfg")) {
			return Err(Error::new_spanned(attribute, "Only doc comments and `#[cfg(...)]` apply to a graph"));
		}

		let ident = function.sig.ident.clone();
		let identifier = graph_identifier(&ident, attributes.display_name.as_ref());
		let description = doc_comment(&function.attrs);

		let imports = function
			.sig
			.inputs
			.iter()
			.enumerate()
			.map(|(index, argument)| parse_import(index, argument, core_types))
			.collect::<syn::Result<Vec<_>>>()?;

		let exports = parse_exports(&function.sig.output)?;

		let mut reader = BodyReader {
			core_types,
			graph_ident: &ident,
			imports: &imports,
			bindings: HashMap::new(),
			nodes: Vec::new(),
		};

		let mut export_sources = None;
		let mut items = Vec::new();
		let statements = &function.block.stmts;
		for (index, statement) in statements.iter().enumerate() {
			match statement {
				Stmt::Local(local) => {
					if export_sources.is_some() {
						return Err(Error::new_spanned(local, "Bindings go before the `return`, and only definitions and data go after it"));
					}
					reader.read_binding(local)?;
				}
				Stmt::Expr(Expr::Return(return_expression), _) => {
					if export_sources.is_some() {
						return Err(Error::new_spanned(return_expression, "A graph has one `return`"));
					}
					let Some(returned) = &return_expression.expr else {
						return Err(Error::new_spanned(return_expression, "Return the export values, like `return card;`"));
					};
					export_sources = Some(reader.read_exports(returned, &exports)?);
				}
				Stmt::Item(Item::Fn(nested)) => {
					return Err(Error::new_spanned(
						&nested.sig.ident,
						"Nested graph definitions aren't supported yet, so define this graph beside the one using it",
					));
				}
				Stmt::Item(item) => items.push(item.clone()),
				// A final expression without a semicolon gives the exports, as it returns a value in Rust
				Stmt::Expr(expression, None) if index + 1 == statements.len() => {
					if export_sources.is_some() {
						return Err(Error::new_spanned(expression, "A graph gives its exports once, with `return` or a final expression"));
					}
					export_sources = Some(reader.read_exports(expression, &exports)?);
				}
				Stmt::Expr(expression, _) => {
					return Err(Error::new_spanned(expression, "Each statement in a graph is a `let` binding or the exports it returns"));
				}
				Stmt::Macro(statement) => {
					return Err(Error::new_spanned(statement, "Macros aren't supported in a graph body"));
				}
			}
		}

		let Some(export_sources) = export_sources else {
			return Err(Error::new_spanned(
				&function.sig.ident,
				"A graph ends its body with one value per export, as a final expression or with `return`",
			));
		};
		let nodes = reader.nodes;

		Ok(Self {
			ident,
			identifier,
			attributes,
			description,
			imports,
			exports,
			nodes,
			export_sources,
			items,
		})
	}

	fn generate(&self, function: &ItemFn, core_types: &TokenStream2, symbols: TokenStream2) -> TokenStream2 {
		let Self {
			ident,
			identifier,
			attributes,
			description,
			imports,
			exports,
			nodes,
			export_sources,
			items,
		} = self;

		let cfg_attributes = cfg_attributes(function);
		let category = attributes.category.as_ref().map(LitStr::value).unwrap_or_default();
		let icon = option_tokens(attributes.icon.as_ref().map(ToTokens::to_token_stream));
		let version = attributes.version.unwrap_or(1);
		let layer = attributes.layer;

		let import_tokens = imports.iter().map(|import| &import.metadata);

		let export_tokens = exports.iter().zip(export_sources).map(|(export, source)| {
			let Export { name, description, .. } = export;
			let source = source_tokens(source, imports, nodes, core_types);
			quote! {
				#core_types::registry::GraphExportMetadata {
					name: #name,
					description: #description,
					source: #source,
				}
			}
		});

		let node_tokens = nodes.iter().map(|node| node_tokens(node, imports, nodes, core_types));
		let primary_input_checks = nodes.iter().map(|node| primary_input_checks(node, core_types));

		let module_name = format_ident!("_{}_graph", ident);
		let metadata_name = format_ident!("_{}_graph_metadata", ident);
		let registry_name = format_ident!(
			"__node_registry_{}_{}Graph",
			NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
			ident.unraw().to_string().to_case(Case::Pascal)
		);

		quote! {
			// Built in the module the graph is written in, so the paths its body uses resolve as written. The generated
			// code names everything by full path, so the author's names can't stand in for what it refers to.
			#(#cfg_attributes)*
			#[doc(hidden)]
			fn #metadata_name() -> #core_types::registry::GraphMetadata {
				#(#items)*
				#(#primary_input_checks)*

				#core_types::registry::GraphMetadata {
					identifier: #identifier,
					category: #category,
					icon: #icon,
					description: #description,
					version: #version,
					layer: #layer,
					imports: ::std::vec![#(#import_tokens,)*],
					exports: ::std::vec![#(#export_tokens,)*],
					nodes: ::std::vec![#(#node_tokens,)*],
				}
			}

			#(#cfg_attributes)*
			#[doc(hidden)]
			mod #module_name {
				use #core_types as gcore;
				use gcore::ctor::ctor;

				#[cfg_attr(not(target_family = "wasm"), ctor(unsafe, crate_path = gcore::ctor))]
				pub(super) fn register_graph() {
					gcore::registry::GRAPH_METADATA.lock().unwrap().push(super::#metadata_name());
				}
			}

			// On Wasm, `ctor` isn't available, so JS invokes this exported shim to register the graph
			#(#cfg_attributes)*
			#[cfg(target_family = "wasm")]
			#[unsafe(no_mangle)]
			extern "C" fn #registry_name() {
				#module_name::register_graph();
			}

			#symbols
		}
	}
}

fn parse_import(index: usize, argument: &FnArg, core_types: &TokenStream2) -> syn::Result<Import> {
	let FnArg::Typed(typed) = argument else {
		return Err(Error::new_spanned(argument, "A graph's imports are named parameters, like `content: List<Graphic>`"));
	};
	let is_unit = matches!(&*typed.ty, Type::Tuple(tuple) if tuple.elems.is_empty());

	// A primary import that takes nothing can go unnamed as `_: ()`, since nothing can wire into it
	let pattern = match &*typed.pat {
		Pat::Ident(pattern) => pattern.clone(),
		Pat::Wild(wild) if index == 0 && is_unit => syn::PatIdent {
			attrs: wild.attrs.clone(),
			by_ref: None,
			mutability: None,
			ident: wild.underscore_token.into(),
			subpat: None,
		},
		_ => {
			return Err(Error::new_spanned(
				&typed.pat,
				"Name each import with an identifier, or write `_: ()` for a primary import that takes nothing",
			));
		}
	};
	if let Some(problem) = binding_pattern_problem(&pattern) {
		return Err(Error::new_spanned(&pattern, format!("An import is a plain name, {problem}")));
	}
	let ident = pattern.ident.clone();

	for attribute in &typed.attrs {
		let attribute_name = attribute.path().get_ident().map(Ident::to_string).unwrap_or_default();
		if !IMPORT_ATTRIBUTES.contains(&attribute_name.as_str()) {
			return Err(Error::new_spanned(attribute, format!("Unknown import attribute `#[{}]`", attribute.path().to_token_stream())));
		}
	}

	let field = parse_field(pattern, (*typed.ty).clone(), &typed.attrs)?;
	if matches!(field.ty, ParsedFieldType::Node(_)) {
		return Err(Error::new_spanned(&typed.ty, "An import is a value, rather than an `impl Node`"));
	}

	let ty = type_tokens(&typed.ty, core_types)?;

	// Like a proto node parameter, an import carries a ranked wire, except a primary import that takes nothing
	if !(is_ranked_type(&typed.ty) || (index == 0 && is_unit)) {
		let message = match is_unit {
			true => "Only the primary import can be `()`, since it means the graph takes nothing there",
			false => "An import's type is an `Item` or `List`, like `width: Item<f64>`, or `()` for a primary import that takes nothing",
		};
		return Err(Error::new_spanned(&typed.ty, message));
	}

	// A parameter named with a leading `_` is internal state, except the primary one, as for a proto node
	let internal = index > 0 && ident.to_string().starts_with('_');
	let metadata = field_metadata_tokens(&field, internal, &option_tokens(Some(ty.clone())), core_types);

	Ok(Import { ident, field, ty, metadata })
}

/// `impl Any`, which is how a graph writes a generic type.
fn is_any_type(ty: &Type) -> bool {
	let Type::ImplTrait(impl_trait) = ty else { return false };
	let [syn::TypeParamBound::Trait(bound)] = impl_trait.bounds.iter().collect::<Vec<_>>().as_slice() else {
		return false;
	};
	bound.maybe.is_none() && bound.lifetimes.is_none() && bound.path.segments.last().is_some_and(|last| last.ident == "Any" && last.arguments.is_none())
}

/// A lone capital letter, like `T`, which Rust would use to name a generic type.
fn is_lone_capital(path: &syn::TypePath) -> bool {
	let Some(segment) = path.path.segments.first().filter(|_| path.path.segments.len() == 1) else {
		return false;
	};
	let name = segment.ident.to_string();
	path.qself.is_none() && segment.arguments.is_none() && name.len() == 1 && name.chars().all(|character| character.is_ascii_uppercase())
}

/// Whether a type is a ranked wire (an `Item` or `List`), or a generic or lazy one, which resolves to a ranked wire.
fn is_ranked_type(ty: &Type) -> bool {
	match ty {
		Type::Paren(inner) => is_ranked_type(&inner.elem),
		Type::FnPtr(_) => true,
		Type::ImplTrait(_) => is_any_type(ty),
		Type::Path(path) => path.path.segments.last().is_some_and(|last| last.ident == "Item" || last.ident == "List"),
		_ => false,
	}
}

/// What makes a binding or import pattern more than a plain name, which a graph has no use for.
fn binding_pattern_problem(pattern: &syn::PatIdent) -> Option<&'static str> {
	if pattern.by_ref.is_some() {
		Some("without `ref`")
	} else if pattern.mutability.is_some() {
		Some("without `mut`")
	} else if pattern.subpat.is_some() {
		Some("without an `@` pattern")
	} else {
		None
	}
}

/// Reads the names in `-> out!(...)`, each with its optional `///` description and `#[name("…")]`.
fn parse_exports(output: &ReturnType) -> syn::Result<Vec<Export>> {
	let ReturnType::Type(_, ty) = output else {
		return Err(Error::new(output.span(), "Name the graph's exports, like `-> out!(out)`"));
	};
	let Type::Macro(type_macro) = &**ty else {
		return Err(Error::new_spanned(ty, "Name the graph's exports with `out!(...)`, like `-> out!(out)`"));
	};
	if !type_macro.mac.path.is_ident("out") {
		return Err(Error::new_spanned(&type_macro.mac.path, "Name the graph's exports with `out!(...)`"));
	}

	let parser = |input: ParseStream| {
		let mut exports = Vec::new();
		while !input.is_empty() {
			let attributes = input.call(Attribute::parse_outer)?;
			if input.peek(Token![_]) {
				let underscore: Token![_] = input.parse()?;
				return Err(Error::new(underscore.span, "A hidden primary export isn't supported yet"));
			}
			let ident: Ident = input.call(Ident::parse_any)?;

			let mut name = None;
			for attribute in &attributes {
				if attribute.path().is_ident("name") {
					if name.is_some() {
						return Err(Error::new_spanned(attribute, "Multiple 'name' attributes are not allowed"));
					}
					name = Some(attribute.parse_args::<LitStr>()?.value());
				} else if !attribute.path().is_ident("doc") {
					return Err(Error::new_spanned(attribute, "Unknown export attribute"));
				}
			}
			let name = name.unwrap_or_else(|| derived_display_name(&ident));

			exports.push(Export {
				ident,
				name,
				description: doc_comment(&attributes),
			});

			if input.is_empty() {
				break;
			}
			input.parse::<Token![,]>()?;
		}
		Ok(exports)
	};

	let exports = parser.parse2(type_macro.mac.tokens.clone())?;
	if exports.is_empty() {
		return Err(Error::new_spanned(type_macro, "Name at least one export"));
	}
	Ok(exports)
}

impl BodyReader<'_> {
	fn read_binding(&mut self, local: &syn::Local) -> syn::Result<()> {
		let Pat::Ident(pattern) = &local.pat else {
			return Err(Error::new_spanned(&local.pat, "Bind each node to a plain name, like `let spacing = ...;`"));
		};
		if let Some(problem) = binding_pattern_problem(pattern) {
			return Err(Error::new_spanned(pattern, format!("Bind each node to a plain name, {problem}")));
		}
		let Some(initializer) = &local.init else {
			return Err(Error::new_spanned(local, "Give each binding a value"));
		};
		if let Some((else_token, _)) = &initializer.diverge {
			return Err(Error::new_spanned(else_token, "A binding names a node, so it has no `else`"));
		}
		if !local.attrs.is_empty() {
			return Err(Error::new_spanned(&local.attrs[0], "Put node state after the call, like `.at(x, y)`, rather than as an attribute"));
		}

		let name = pattern.ident.to_string();
		if self.bindings.contains_key(&name) || self.imports.iter().any(|import| import.ident == pattern.ident) {
			return Err(Error::new_spanned(&pattern.ident, format!("`{name}` is already the name of a binding or import")));
		}

		let source = self.read(&initializer.expr)?;
		self.bindings.insert(name, source);
		Ok(())
	}

	fn read_exports(&mut self, returned: &Expr, exports: &[Export]) -> syn::Result<Vec<Feed>> {
		let values: Vec<&Expr> = match returned {
			Expr::Tuple(tuple) => tuple.elems.iter().collect(),
			single => vec![single],
		};

		if values.len() != exports.len() {
			let message = format!("The graph names {} exports in `out!(...)`, but returns {} values", exports.len(), values.len());
			return Err(Error::new_spanned(returned, message));
		}

		values
			.into_iter()
			.map(|value| match self.read(value)? {
				Feed::Import { .. } | Feed::Scope(_) => Err(Error::new_spanned(value, "An export can't come straight from an import or scope yet, so pass it through a node first")),
				feed => Ok(feed),
			})
			.collect()
	}

	/// Reads an expression, creating any nodes it calls, and returns what its result feeds.
	fn read(&mut self, expression: &Expr) -> syn::Result<Feed> {
		match expression {
			Expr::Paren(inner) => self.read(&inner.expr),
			Expr::Group(inner) => self.read(&inner.expr),
			Expr::Path(path) if path.qself.is_none() => {
				if let Some(ident) = path.path.get_ident() {
					let name = ident.to_string();
					if let Some(source) = self.bindings.get(&name) {
						return Ok(source.clone());
					}
					if let Some(index) = self.imports.iter().position(|import| *ident == import.ident) {
						return Ok(Feed::Import { index, ty: None });
					}
					if name == "None" {
						return Err(Error::new_spanned(ident, "Type defaults like `None` aren't supported yet, so leave the input out to keep its default"));
					}

					// A name in capitals is a constant, used as a value
					let is_constant = name.chars().any(|character| character.is_ascii_uppercase()) && !name.chars().any(|character| character.is_ascii_lowercase());
					if !is_constant {
						return Err(Error::new_spanned(ident, format!("`{name}` isn't a binding or import. Call a node with parentheses, like `{name}()`")));
					}
				}
				Ok(self.value(expression))
			}
			Expr::Lit(_) | Expr::Unary(syn::ExprUnary { op: UnOp::Neg(_), .. }) => self.literal(expression),
			Expr::Array(array) => {
				let numbers = array.elems.iter().map(number_literal).collect::<Option<Vec<_>>>();
				match numbers.as_deref() {
					Some([x, y]) => {
						let core_types = self.core_types;
						Ok(Feed::Value {
							value: quote_spanned!(array.span()=> #core_types::registry::GraphValue::vec2(#x, #y)),
							exposed: None,
						})
					}
					_ => Err(Error::new_spanned(
						array,
						"Several wires into one input aren't supported until variadic inputs land, and a value array is a two-number Vec2",
					)),
				}
			}
			Expr::Cast(cast) => {
				let Feed::Import { index, .. } = self.read(&cast.expr)? else {
					return Err(Error::new_spanned(cast, "Only an import can be cast, to set the type its wire records"));
				};
				Ok(Feed::Import {
					index,
					ty: Some(type_tokens(&cast.ty, self.core_types)?),
				})
			}
			Expr::Call(call) => self.read_call(call),
			Expr::MethodCall(method_call) => self.read_state(method_call),
			Expr::Binary(binary) if matches!(binary.op, BinOp::Shr(_)) => {
				let upstream = self.read(&binary.left)?;
				let (node, output) = self.read_node(&binary.right)?;

				// A binding on the right is a node made earlier, which the left side may already be fed by
				if let Feed::Node { node: upstream_node, .. } = upstream
					&& self.is_fed_by(upstream_node, node)
				{
					return Err(Error::new_spanned(binary, "This wire makes a loop, since the node on the right already feeds the left side"));
				}

				let node_entry = &mut self.nodes[node];
				if let Some(previous) = node_entry.primary_span {
					let mut error = Error::new_spanned(&binary.right, "This node's primary input is already fed. Several wires into one input are written together as an array");
					error.combine(Error::new(previous, "It's fed here"));
					return Err(error);
				}
				node_entry.primary_span = Some(binary.left.span());
				node_entry.inputs.push((Slot::Primary, upstream));

				Ok(Feed::Node { node, output })
			}
			Expr::Binary(binary) => Err(Error::new_spanned(
				binary,
				"Graphs use only `>>`, which feeds its left side into the next node. Do math with nodes, like `multiply(multiplicand = 2)`",
			)),
			Expr::Field(field) => self.read_output(field),
			_ => Err(Error::new_spanned(expression, "This expression isn't part of the graph syntax")),
		}
	}

	/// Whether `node` is `source` or feeds it through any chain of wires. O(n) in the nodes upstream of `node`.
	fn is_fed_by(&self, node: usize, source: usize) -> bool {
		let mut visited = vec![false; self.nodes.len()];
		let mut stack = vec![node];

		while let Some(current) = stack.pop() {
			if current == source {
				return true;
			}
			if std::mem::replace(&mut visited[current], true) {
				continue;
			}
			stack.extend(self.nodes[current].inputs.iter().filter_map(|(_, feed)| match feed {
				Feed::Node { node, .. } => Some(*node),
				_ => None,
			}));
		}

		false
	}

	/// Reads the right side of `>>`, which must be a node (a call, or a binding naming one) and may choose one of its outputs.
	fn read_node(&mut self, expression: &Expr) -> syn::Result<(usize, Option<Ident>)> {
		match self.read(expression)? {
			Feed::Node { node, output } => Ok((node, output)),
			_ => Err(Error::new_spanned(expression, "The right side of `>>` is a node, like `fill()`, or a binding naming one")),
		}
	}

	/// Reads the choice of one of a node's outputs by name, like `.captures`.
	fn read_output(&mut self, field: &syn::ExprField) -> syn::Result<Feed> {
		let syn::Member::Named(output) = &field.member else {
			return Err(Error::new_spanned(&field.member, "Choose an output by its name, like `.captures`"));
		};

		match self.read(&field.base)? {
			Feed::Node { node, output: None } => Ok(Feed::Node { node, output: Some(output.clone()) }),
			Feed::Node { output: Some(_), .. } => Err(Error::new_spanned(output, "An output is already chosen here")),
			_ => Err(Error::new_spanned(&field.base, "Only a node has outputs to choose from")),
		}
	}

	fn literal(&self, expression: &Expr) -> syn::Result<Feed> {
		let core_types = self.core_types;
		let span = expression.span();

		// The input an integer feeds sets its type, which a suffix would contradict
		let integer = match expression {
			Expr::Lit(syn::ExprLit { lit: Lit::Int(integer), .. }) => Some(integer),
			Expr::Unary(syn::ExprUnary { expr, .. }) => match &**expr {
				Expr::Lit(syn::ExprLit { lit: Lit::Int(integer), .. }) => Some(integer),
				_ => None,
			},
			_ => None,
		};
		if let Some(integer) = integer
			&& !integer.suffix().is_empty()
		{
			return Err(Error::new_spanned(integer, "Write the integer without a type suffix, since the input it feeds sets its type"));
		}

		let value = match expression {
			Expr::Lit(literal) => match &literal.lit {
				Lit::Int(_) => quote_spanned!(span=> #core_types::registry::GraphValue::Integer(#expression)),
				Lit::Float(_) => quote_spanned!(span=> #core_types::registry::GraphValue::Number(#expression)),
				Lit::Bool(_) => quote_spanned!(span=> #core_types::registry::GraphValue::Bool(#expression)),
				Lit::Str(_) => quote_spanned!(span=> #core_types::registry::GraphValue::String(::std::string::String::from(#expression))),
				_ => return Err(Error::new_spanned(literal, "Unsupported literal")),
			},
			Expr::Unary(unary) => match &*unary.expr {
				Expr::Lit(syn::ExprLit { lit: Lit::Int(_), .. }) => quote_spanned!(span=> #core_types::registry::GraphValue::Integer(#expression)),
				Expr::Lit(syn::ExprLit { lit: Lit::Float(_), .. }) => quote_spanned!(span=> #core_types::registry::GraphValue::Number(#expression)),
				_ => return Err(Error::new_spanned(unary, "Only a number can be negated")),
			},
			_ => return Err(Error::new_spanned(expression, "Unsupported literal")),
		};

		Ok(Feed::Value { value, exposed: None })
	}

	/// A constant or other Rust expression used as a value.
	fn value(&self, expression: &Expr) -> Feed {
		let core_types = self.core_types;
		Feed::Value {
			value: quote_spanned!(expression.span()=> #core_types::registry::GraphValue::from(#expression)),
			exposed: None,
		}
	}

	fn read_call(&mut self, call: &ExprCall) -> syn::Result<Feed> {
		let Expr::Path(function) = &*call.func else {
			return Err(Error::new_spanned(&call.func, "Call a node by its name, like `fill()`"));
		};
		let module = function.path.clone();

		// Value helpers, which build values and input sources rather than nodes
		if let Some(name) = module.get_ident().map(Ident::to_string) {
			let core_types = self.core_types;
			let single_argument = || -> syn::Result<&Expr> {
				match call.args.iter().collect::<Vec<_>>().as_slice() {
					[argument] => Ok(argument),
					_ => Err(Error::new_spanned(call, format!("`{name}(...)` takes one argument"))),
				}
			};

			match name.as_str() {
				"rgb" | "rgba" => {
					let hex = single_argument()?;

					// `rgb` takes six hex digits, and a larger literal would lose its top byte
					if name == "rgb"
						&& let Expr::Lit(syn::ExprLit { lit: Lit::Int(literal), .. }) = hex
						&& literal.base10_parse::<u64>().is_ok_and(|value| value > 0xFF_FFFF)
					{
						return Err(Error::new_spanned(hex, "`rgb(...)` takes six hex digits, like `rgb(0xE85D75)`. Use `rgba(...)` to give an alpha too"));
					}

					let constructor = format_ident!("{name}");
					return Ok(Feed::Value {
						value: quote_spanned!(call.span()=> #core_types::registry::GraphValue::#constructor(#hex)),
						exposed: None,
					});
				}
				"scope" => return Ok(Feed::Scope(single_argument()?.clone())),
				"reflect" => {
					let argument = single_argument()?;
					let Expr::Path(reflected) = argument else {
						return Err(Error::new_spanned(argument, "Reflect `NodePath` or `SourceId`"));
					};
					return match reflected.path.get_ident().map(Ident::to_string).as_deref() {
						Some("NodePath") => Ok(Feed::NodePath),
						Some("SourceId") => Ok(Feed::SourceId),
						_ => Err(Error::new_spanned(argument, "Reflect `NodePath` or `SourceId`")),
					};
				}
				"exposed" | "unexposed" => {
					let Feed::Value { value, .. } = self.read(single_argument()?)? else {
						return Err(Error::new_spanned(call, format!("`{name}(...)` wraps a value")));
					};
					return Ok(Feed::Value {
						value,
						exposed: Some(name == "exposed"),
					});
				}
				_ => {}
			}
		}

		if module.get_ident() == Some(self.graph_ident) {
			return Err(Error::new_spanned(&call.func, "A graph can't contain itself"));
		}

		// Read each named argument before creating the node, so the nodes feeding it come first
		let mut inputs: Vec<(Slot, Feed)> = Vec::new();
		for argument in &call.args {
			let (parameter, value) = match argument {
				Expr::Assign(assignment) => {
					let parameter = match &*assignment.left {
						Expr::Path(parameter) => parameter.path.get_ident(),
						_ => None,
					};
					let Some(parameter) = parameter else {
						return Err(Error::new_spanned(&assignment.left, "Name the parameter, like `weight = 6`"));
					};
					(parameter.clone(), &*assignment.right)
				}
				// The shorthand `progression` feeds the binding or import of that name into the parameter of the same name
				Expr::Path(shorthand) if shorthand.path.get_ident().is_some() => (shorthand.path.segments[0].ident.clone(), argument),
				_ => return Err(Error::new_spanned(argument, "Name each argument, like `weight = 6`, or write a bare name for the shorthand")),
			};

			if let Some((Slot::Named(previous), _)) = inputs.iter().find(|(slot, _)| matches!(slot, Slot::Named(named) if *named == parameter)) {
				let mut error = Error::new_spanned(
					&parameter,
					format!("`{parameter}` is already set in this call. Several wires into one input are written together as an array"),
				);
				error.combine(Error::new_spanned(previous, "It's set here"));
				return Err(error);
			}

			inputs.push((Slot::Named(parameter), self.read(value)?));
		}

		self.nodes.push(Node {
			module,
			inputs,
			primary_span: None,
			position: None,
			layer: false,
			display_name: None,
			hidden: false,
			locked: false,
			pinned: false,
			call_argument: None,
		});
		Ok(Feed::Node {
			node: self.nodes.len() - 1,
			output: None,
		})
	}

	/// Reads a node state method after a call, like `.at(x, y)` or `.hidden()`.
	fn read_state(&mut self, method_call: &ExprMethodCall) -> syn::Result<Feed> {
		let mut receiver = &*method_call.receiver;
		while let Expr::Paren(syn::ExprParen { expr: inner, .. }) | Expr::Group(syn::ExprGroup { expr: inner, .. }) = receiver {
			receiver = inner;
		}

		if matches!(receiver, Expr::Field(_)) {
			return Err(Error::new_spanned(
				&method_call.method,
				"Node state goes before the output it chooses, like `regex_find().at(0, 0).captures`",
			));
		}
		if !matches!(receiver, Expr::Call(_) | Expr::MethodCall(_)) {
			return Err(Error::new_spanned(&method_call.method, "Node state goes directly after the call it modifies, not on a binding"));
		}

		let source = self.read(receiver)?;
		let Feed::Node { node, .. } = source else {
			return Err(Error::new_spanned(&method_call.method, "Node state goes after a node call"));
		};

		let arguments: Vec<&Expr> = method_call.args.iter().collect();
		let method = method_call.method.to_string();
		let no_arguments = |arguments: &[&Expr]| {
			if arguments.is_empty() {
				Ok(())
			} else {
				Err(Error::new_spanned(&method_call.args, format!("`.{method}()` takes no arguments")))
			}
		};

		// Only `.call_argument::<...>()` takes a type
		if method != "call_argument"
			&& let Some(turbofish) = &method_call.turbofish
		{
			return Err(Error::new_spanned(turbofish, format!("`.{method}()` takes no type")));
		}

		let core_types = self.core_types;
		let entry = &mut self.nodes[node];

		// Each piece of state is set once, as each `#[graph(...)]` attribute is
		let already_set = match method.as_str() {
			"at" => entry.position.is_some(),
			"layer" => entry.layer,
			"hidden" => entry.hidden,
			"locked" => entry.locked,
			"pinned" => entry.pinned,
			"named" => entry.display_name.is_some(),
			"call_argument" => entry.call_argument.is_some(),
			_ => false,
		};
		if already_set {
			return Err(Error::new_spanned(&method_call.method, format!("`.{method}()` is already set on this node")));
		}

		match method.as_str() {
			"at" => {
				let [x, y] = arguments.as_slice() else {
					return Err(Error::new_spanned(method_call, "Give a position in grid units, like `.at(-50, 8)`"));
				};
				entry.position = Some((grid_coordinate(x)?, grid_coordinate(y)?));
			}
			"layer" => {
				no_arguments(&arguments)?;
				entry.layer = true;
			}
			"hidden" => {
				no_arguments(&arguments)?;
				entry.hidden = true;
			}
			"locked" => {
				no_arguments(&arguments)?;
				entry.locked = true;
			}
			"pinned" => {
				no_arguments(&arguments)?;
				entry.pinned = true;
			}
			"named" => {
				let [Expr::Lit(syn::ExprLit { lit: Lit::Str(name), .. })] = arguments.as_slice() else {
					return Err(Error::new_spanned(method_call, "Give a display name, like `.named(\"Candle spacing\")`"));
				};
				entry.display_name = Some(name.clone());
			}
			"call_argument" => {
				no_arguments(&arguments)?;
				let Some(turbofish) = &method_call.turbofish else {
					return Err(Error::new_spanned(method_call, "Name the call argument's type, like `.call_argument::<Footprint>()`"));
				};
				let [syn::GenericArgument::Type(ty)] = turbofish.args.iter().collect::<Vec<_>>().as_slice() else {
					return Err(Error::new_spanned(turbofish, "Name one call argument type, like `.call_argument::<Footprint>()`"));
				};
				entry.call_argument = Some(type_tokens(ty, core_types)?);
			}
			_ => {
				return Err(Error::new_spanned(
					&method_call.method,
					format!("Unknown node state `.{method}()`. Parameters go inside the call's parentheses, like `node({method} = ...)`"),
				));
			}
		}

		Ok(source)
	}
}

fn grid_coordinate(expression: &Expr) -> syn::Result<i32> {
	let (negative, literal) = match expression {
		Expr::Unary(syn::ExprUnary { op: UnOp::Neg(_), expr, .. }) => (true, &**expr),
		other => (false, other),
	};
	let Expr::Lit(syn::ExprLit { lit: Lit::Int(integer), .. }) = literal else {
		return Err(Error::new_spanned(expression, "Positions are integers in grid units"));
	};
	if !integer.suffix().is_empty() {
		return Err(Error::new_spanned(integer, "Write the position without a type suffix"));
	}
	let magnitude: i32 = integer.base10_parse()?;
	Ok(if negative { -magnitude } else { magnitude })
}

/// A number literal, possibly negative, as an `f64` expression.
fn number_literal(expression: &Expr) -> Option<TokenStream2> {
	match expression {
		Expr::Lit(syn::ExprLit { lit: Lit::Int(integer), .. }) if integer.suffix().is_empty() => {
			let value = integer.base10_parse::<f64>().ok()?;
			Some(quote!(#value))
		}
		Expr::Lit(syn::ExprLit { lit: Lit::Float(float), .. }) => Some(quote!(#float)),
		Expr::Unary(syn::ExprUnary { op: UnOp::Neg(_), expr, .. }) => number_literal(expr).map(|number| quote!(-#number)),
		_ => None,
	}
}

fn source_tokens(source: &Feed, imports: &[Import], nodes: &[Node], core_types: &TokenStream2) -> TokenStream2 {
	match source {
		Feed::Node { node, output } => {
			let output = match output {
				// A misspelled output is reported at the name the author wrote
				Some(output) => {
					let module = &nodes[*node].module;
					let struct_name = symbol_name(output, "Output");
					quote_spanned!(output.span()=> #module::#struct_name::INDEX)
				}
				None => quote!(0),
			};
			quote!(#core_types::registry::GraphInputSource::Node { node: #node, output: #output })
		}
		Feed::Import { index, ty } => {
			let ty = ty.clone().unwrap_or_else(|| imports[*index].ty.clone());
			quote!(#core_types::registry::GraphInputSource::Import { index: #index, ty: #ty })
		}
		Feed::Value { value, exposed } => {
			let exposed = option_tokens(exposed.map(|exposed| quote!(#exposed)));
			quote!(#core_types::registry::GraphInputSource::Value { value: #value, exposed: #exposed })
		}
		// A string literal is the key itself, and anything else is an identifier, as for `#[scope(...)]`
		Feed::Scope(key @ Expr::Lit(syn::ExprLit { lit: Lit::Str(_), .. })) => quote!(#core_types::registry::GraphInputSource::Scope(#key)),
		Feed::Scope(key) => quote!(#core_types::registry::GraphInputSource::Scope(#key.as_static_str())),
		Feed::NodePath => quote!(#core_types::registry::GraphInputSource::NodePath),
		Feed::SourceId => quote!(#core_types::registry::GraphInputSource::SourceId),
	}
}

/// The index of the input a slot names, from the symbols in the called node's module.
fn input_index(module: &syn::Path, slot: &Slot, core_types: &TokenStream2) -> TokenStream2 {
	match slot {
		// A node's primary input is its first, since the inputs the compiler fills in follow the parameters
		Slot::Primary => quote!(0),
		Slot::Named(parameter) => {
			let struct_name = symbol_name(parameter, "Input");
			quote_spanned! {parameter.span()=>
				{
					#[allow(unused_imports)]
					use #core_types::NodeParameter as _;
					#module::#struct_name::INDEX
				}
			}
		}
	}
}

/// Compile-time checks that a node `>>` feeds has a primary input, and that no argument names that input too.
fn primary_input_checks(node: &Node, core_types: &TokenStream2) -> TokenStream2 {
	if node.primary_span.is_none() {
		return TokenStream2::new();
	}

	let module = &node.module;
	let node_name = module.segments.last().map(|segment| segment.ident.to_string()).unwrap_or_default();
	let no_parameters = format!("`{node_name}` has no parameters, so `>>` has no input to feed");
	let has_primary = quote_spanned! {module.span()=>
		const _: () = assert!(#module::PARAMETER_COUNT > 0, #no_parameters);
	};

	let primary = input_index(&node.module, &Slot::Primary, core_types);
	let checks = node.inputs.iter().filter_map(|(slot, _)| {
		let Slot::Named(parameter) = slot else { return None };
		let named = input_index(&node.module, slot, core_types);
		let message = format!("`{parameter}` is the primary input, which `>>` already feeds, so it's set twice");
		Some(quote_spanned! {parameter.span()=>
			const _: () = assert!(#named != #primary, #message);
		})
	});
	quote!(#has_primary #(#checks)*)
}

fn node_tokens(node: &Node, imports: &[Import], nodes: &[Node], core_types: &TokenStream2) -> TokenStream2 {
	let Node {
		module,
		inputs,
		position,
		layer,
		display_name,
		hidden,
		locked,
		pinned,
		call_argument,
		..
	} = node;

	// A misspelled node or parameter is reported at the name the author wrote
	let implementation = quote_spanned!(module.span()=> #core_types::registry::GraphNodeImplementation::from(#module::IDENTIFIER));
	let inputs = inputs.iter().map(|(slot, source)| {
		let index = input_index(module, slot, core_types);
		let source = source_tokens(source, imports, nodes, core_types);
		quote!((#index, #source))
	});

	let position = option_tokens(position.map(|(x, y)| quote!((#x, #y))));
	let display_name = option_tokens(display_name.as_ref().map(ToTokens::to_token_stream));
	let call_argument = option_tokens(call_argument.clone());

	quote! {
		#core_types::registry::GraphNodeMetadata {
			implementation: #implementation,
			inputs: ::std::vec![#(#inputs,)*],
			position: #position,
			layer: #layer,
			display_name: #display_name,
			hidden: #hidden,
			locked: #locked,
			pinned: #pinned,
			call_argument: #call_argument,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::codegen::parameter_display_name;

	fn parse(function: ItemFn) -> syn::Result<Graph> {
		let attributes = parse_graph_attributes(quote!(category("Test"))).expect("the attributes should parse");
		Graph::parse(&function, attributes, &quote!(core_types))
	}

	fn error_message(function: ItemFn) -> String {
		match parse(function) {
			Ok(_) => panic!("the graph should be rejected"),
			Err(error) => error.to_string(),
		}
	}

	#[test]
	fn reads_chains_named_arguments_shorthand_and_state() {
		let graph = parse(syn::parse_quote! {
			/// Outlines its content.
			fn outlined(content: List<Graphic>, #[unit(" px")] width: Item<f64>) -> out!(out) {
				let thickness = width >> multiply(multiplicand = 2).at(1, 2);
				return content >> stroke(weight = thickness, width).hidden();
			}
		})
		.expect("the graph should parse");

		assert_eq!(graph.identifier, "Outlined");
		assert_eq!(graph.description, "Outlines its content.");
		assert_eq!(graph.imports.iter().map(|import| parameter_display_name(&import.field)).collect::<Vec<_>>(), ["Content", "Width"]);
		assert_eq!(graph.nodes.len(), 2);

		let (multiply, stroke) = (&graph.nodes[0], &graph.nodes[1]);
		assert_eq!(multiply.position, Some((1, 2)));
		assert!(stroke.hidden);
		assert!(matches!(&stroke.inputs[0], (Slot::Named(name), Feed::Node { node: 0, .. }) if name == "weight"));
		assert!(matches!(&stroke.inputs[1], (Slot::Named(name), Feed::Import { index: 1, .. }) if name == "width"));
		assert!(matches!(&stroke.inputs[2], (Slot::Primary, Feed::Import { index: 0, .. })));
	}

	#[test]
	fn a_final_expression_gives_the_exports_as_return_does() {
		for function in [
			syn::parse_quote! {
				fn outlined(content: List<Graphic>) -> out!(out) {
					return content >> stroke(weight = 2);
				}
			},
			syn::parse_quote! {
				fn outlined(content: List<Graphic>) -> out!(out) {
					content >> stroke(weight = 2)
				}
			},
		] {
			let graph = parse(function).expect("the graph should parse");
			assert_eq!(graph.nodes.len(), 1);
			assert!(matches!(graph.export_sources.as_slice(), [Feed::Node { node: 0, .. }]));
		}
	}

	#[test]
	fn rejects_exports_given_twice() {
		let message = error_message(syn::parse_quote! {
			fn twice(content: List<Graphic>) -> out!(out) {
				return content >> stroke();
				content >> stroke()
			}
		});
		assert!(message.contains("exports once"), "{message}");
	}

	#[test]
	fn rejects_a_parameter_set_twice() {
		let message = error_message(syn::parse_quote! {
			fn twice(content: List<Graphic>) -> out!(out) {
				return content >> stroke(weight = 1, weight = 2);
			}
		});
		assert!(message.contains("already set"), "{message}");
	}

	#[test]
	fn rejects_a_primary_input_fed_twice() {
		let message = error_message(syn::parse_quote! {
			fn twice(content: List<Graphic>) -> out!(out) {
				let outline = stroke();
				let lower = content >> outline;
				return content >> outline;
			}
		});
		assert!(message.contains("already fed"), "{message}");
	}

	#[test]
	fn rejects_unknown_node_state() {
		let message = error_message(syn::parse_quote! {
			fn state(content: List<Graphic>) -> out!(out) {
				return content >> stroke().weight(2);
			}
		});
		assert!(message.contains("Unknown node state"), "{message}");
	}

	#[test]
	fn rejects_a_return_that_doesnt_match_the_exports() {
		let message = error_message(syn::parse_quote! {
			fn mismatch(content: List<Graphic>) -> out!(first, second) {
				return content;
			}
		});
		assert!(message.contains("2 exports"), "{message}");
	}

	#[test]
	fn rejects_an_export_straight_from_an_import() {
		let message = error_message(syn::parse_quote! {
			fn passthrough(content: List<Graphic>) -> out!(out) {
				let alias = content;
				return alias;
			}
		});
		assert!(message.contains("straight from an import"), "{message}");
	}

	#[test]
	fn rejects_a_suffixed_integer() {
		let message = error_message(syn::parse_quote! {
			fn suffixed(content: List<Graphic>) -> out!(out) {
				return content >> stroke(weight = -2_i64);
			}
		});
		assert!(message.contains("without a type suffix"), "{message}");
	}

	#[test]
	fn rejects_type_arguments_it_would_drop() {
		let list = error_message(syn::parse_quote! {
			fn extra(content: List<Graphic, Vector>) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(list.contains("takes one element type"), "{list}");

		let state = error_message(syn::parse_quote! {
			fn extra(content: List<Graphic>) -> out!(out) {
				return content >> stroke().hidden::<T>();
			}
		});
		assert!(state.contains("takes no type"), "{state}");
	}

	#[test]
	fn rejects_repeated_attributes_and_a_zero_version() {
		let attribute_error = |attributes: TokenStream2| match parse_graph_attributes(attributes) {
			Ok(_) => panic!("the attributes should be rejected"),
			Err(error) => error.to_string(),
		};

		let repeated = attribute_error(quote!(category("A"), category("B")));
		assert!(repeated.contains("Multiple 'category'"), "{repeated}");

		let zero = attribute_error(quote!(category("A"), version(0)));
		assert!(zero.contains("Versions start at 1"), "{zero}");
	}

	#[test]
	fn rejects_signatures_a_graph_cant_have() {
		let asynchronous = error_message(syn::parse_quote! {
			async fn later(content: List<Graphic>) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(asynchronous.contains("can't be `async`"), "{asynchronous}");

		let generic = error_message(syn::parse_quote! {
			fn generic<T>(content: List<T>) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(generic.contains("doesn't declare generics"), "{generic}");
	}

	#[test]
	fn rejects_proto_node_only_attributes_on_an_import() {
		let message = error_message(syn::parse_quote! {
			fn constant(#[constant] content: List<Graphic>) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(message.contains("Unknown import attribute `#[constant]`"), "{message}");
	}

	#[test]
	fn reads_outputs_chosen_by_name() {
		let graph = parse(syn::parse_quote! {
			fn digits(text: Item<String>) -> out!(first, runs, run_count) {
				let found = regex_find(string = text).at(1, 0);
				let runs = text >> regex_find().captures;
				return (found.r#match, runs, found.captures >> count());
			}
		})
		.expect("the graph should parse");

		let chosen = |feed: &Feed| match feed {
			Feed::Node { node, output } => (*node, output.as_ref().map(Ident::to_string)),
			_ => panic!("the feed should be a node"),
		};
		assert_eq!(chosen(&graph.export_sources[0]), (0, Some("r#match".into())));
		assert_eq!(chosen(&graph.export_sources[1]), (1, Some("captures".into())));
		assert_eq!(chosen(&graph.export_sources[2]), (2, None));

		assert!(matches!(&graph.nodes[1].inputs[0], (Slot::Primary, Feed::Import { index: 0, .. })));
		assert_eq!(chosen(&graph.nodes[2].inputs[0].1), (0, Some("captures".into())));
	}

	#[test]
	fn rejects_outputs_chosen_wrongly() {
		let numbered = error_message(syn::parse_quote! {
			fn numbered(text: Item<String>) -> out!(out) {
				return text >> regex_find().1;
			}
		});
		assert!(numbered.contains("by its name"), "{numbered}");

		let twice = error_message(syn::parse_quote! {
			fn twice(text: Item<String>) -> out!(out) {
				return text >> regex_find().captures.groups;
			}
		});
		assert!(twice.contains("already chosen"), "{twice}");

		let import = error_message(syn::parse_quote! {
			fn import(text: Item<String>) -> out!(out) {
				return text.length >> count();
			}
		});
		assert!(import.contains("Only a node has outputs"), "{import}");

		let state = error_message(syn::parse_quote! {
			fn state(text: Item<String>) -> out!(out) {
				return text >> regex_find().captures.at(1, 2);
			}
		});
		assert!(state.contains("before the output it chooses"), "{state}");
	}

	#[test]
	fn rejects_a_wire_that_makes_a_loop() {
		let around = error_message(syn::parse_quote! {
			fn looped(content: List<Graphic>) -> out!(out) {
				let outline = stroke();
				let shadow = outline >> blur();
				return shadow >> outline;
			}
		});
		assert!(around.contains("makes a loop"), "{around}");

		let itself = error_message(syn::parse_quote! {
			fn looped(content: List<Graphic>) -> out!(out) {
				let outline = stroke();
				return outline >> outline;
			}
		});
		assert!(itself.contains("makes a loop"), "{itself}");
	}

	#[test]
	fn rejects_a_graph_that_contains_itself() {
		let message = error_message(syn::parse_quote! {
			fn outlined(content: List<Graphic>) -> out!(out) {
				return content >> outlined();
			}
		});
		assert!(message.contains("can't contain itself"), "{message}");
	}

	#[test]
	fn rejects_state_set_twice() {
		let message = error_message(syn::parse_quote! {
			fn twice(content: List<Graphic>) -> out!(out) {
				return content >> stroke().at(1, 2).at(3, 4);
			}
		});
		assert!(message.contains("`.at()` is already set"), "{message}");
	}

	#[test]
	fn reads_state_after_a_parenthesized_call() {
		let graph = parse(syn::parse_quote! {
			fn grouped(content: List<Graphic>) -> out!(out) {
				return content >> (stroke()).hidden();
			}
		})
		.expect("the graph should parse");
		assert!(graph.nodes[0].hidden);
	}

	#[test]
	fn rejects_bindings_and_imports_that_arent_plain_names() {
		let binding = error_message(syn::parse_quote! {
			fn mutable(content: List<Graphic>) -> out!(out) {
				let mut outline = content >> stroke();
				return outline;
			}
		});
		assert!(binding.contains("without `mut`"), "{binding}");

		let diverging = error_message(syn::parse_quote! {
			fn diverging(content: List<Graphic>) -> out!(out) {
				let outline = content >> stroke() else { return };
				return outline;
			}
		});
		assert!(diverging.contains("no `else`"), "{diverging}");

		let import = error_message(syn::parse_quote! {
			fn mutable(mut content: List<Graphic>) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(import.contains("without `mut`"), "{import}");
	}

	#[test]
	fn reads_an_unnamed_primary_import_that_takes_nothing() {
		let graph = parse(syn::parse_quote! {
			fn generated(_: (), seed: Item<f64>) -> out!(out) {
				return random(seed);
			}
		})
		.expect("the graph should parse");
		assert_eq!(parameter_display_name(&graph.imports[0].field), "");

		let named_elsewhere = error_message(syn::parse_quote! {
			fn generated(content: List<Graphic>, _: ()) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(named_elsewhere.contains("Name each import"), "{named_elsewhere}");
	}

	#[test]
	fn reads_generic_types_written_as_impl_any() {
		let graph = parse(syn::parse_quote! {
			fn generic(content: impl Any, items: List<impl Any>) -> out!(out) {
				return content >> passthrough().call_argument::<impl Any>();
			}
		})
		.expect("the graph should parse");
		assert!(graph.imports[0].ty.to_string().contains("generic"));
		assert!(graph.imports[1].ty.to_string().contains("generic"));

		let named = error_message(syn::parse_quote! {
			fn generic(content: T) -> out!(out) {
				return content >> passthrough();
			}
		});
		assert!(named.contains("written `impl Any`"), "{named}");

		let other_trait = error_message(syn::parse_quote! {
			fn generic(content: impl Clone) -> out!(out) {
				return content >> passthrough();
			}
		});
		assert!(other_trait.contains("written `impl Any`"), "{other_trait}");
	}

	#[test]
	fn rejects_imports_that_arent_ranked() {
		let unranked = error_message(syn::parse_quote! {
			fn unranked(content: List<Graphic>, count: i64) -> out!(out) {
				return content >> repeat(count);
			}
		});
		assert!(unranked.contains("`Item` or `List`"), "{unranked}");

		let unit = error_message(syn::parse_quote! {
			fn unit(content: List<Graphic>, nothing: ()) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(unit.contains("Only the primary import can be `()`"), "{unit}");
	}

	#[test]
	fn rejects_values_the_macro_would_misread() {
		let suffixed_position = error_message(syn::parse_quote! {
			fn suffixed(content: List<Graphic>) -> out!(out) {
				return content >> stroke().at(5_i32, 0);
			}
		});
		assert!(suffixed_position.contains("position without a type suffix"), "{suffixed_position}");

		let alpha_in_rgb = error_message(syn::parse_quote! {
			fn translucent(content: List<Graphic>) -> out!(out) {
				return content >> fill(paint = rgb(0xE85D7580));
			}
		});
		assert!(alpha_in_rgb.contains("six hex digits"), "{alpha_in_rgb}");

		let type_default = error_message(syn::parse_quote! {
			fn unpainted(content: List<Graphic>) -> out!(out) {
				return content >> fill(paint = None);
			}
		});
		assert!(type_default.contains("Type defaults like `None`"), "{type_default}");
	}

	#[test]
	fn reads_a_scope_named_by_a_node_identifier() {
		let graph = parse(syn::parse_quote! {
			fn scoped(content: List<Graphic>) -> out!(out) {
				return content >> stroke(weight = scope(editor_api::IDENTIFIER));
			}
		})
		.expect("the graph should parse");
		assert!(matches!(&graph.nodes[0].inputs[0], (Slot::Named(_), Feed::Scope(Expr::Path(_)))));
	}

	#[test]
	fn rejects_attributes_a_graph_would_drop() {
		let message = error_message(syn::parse_quote! {
			#[allow(dead_code)]
			fn kept(content: List<Graphic>) -> out!(out) {
				return content >> stroke();
			}
		});
		assert!(message.contains("Only doc comments and `#[cfg(...)]`"), "{message}");

		let export_names = error_message(syn::parse_quote! {
			fn named(content: List<Graphic>) -> out!(#[name("A")] #[name("B")] out) {
				return content >> stroke();
			}
		});
		assert!(export_names.contains("Multiple 'name'"), "{export_names}");
	}

	#[test]
	fn rejects_state_on_a_binding() {
		let message = error_message(syn::parse_quote! {
			fn state(content: List<Graphic>) -> out!(out) {
				let outline = content >> stroke();
				return outline.hidden();
			}
		});
		assert!(message.contains("not on a binding"), "{message}");
	}
}
