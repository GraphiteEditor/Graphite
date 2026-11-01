//! `#[migration]`, `rename!`, and `format_migration!` for `migration-core`. See `node-graph/rfcs/document-format-migrations.md` for what they expand to.

mod format;
mod step;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use step::{Declaration, Step, scan_pins};
use syn::spanned::Spanned;
use syn::{Error, FnArg, Ident, ItemFn, ReturnType, Type, parse_macro_input};

/// Turns a function into a node migration, named by its source and target declarations:
/// `#[migration("module::node"@1 => @2)]` for a version step, `#[migration("module::old"@1 => "module::new"@2)]` for a replacement.
///
/// Parameters state what the migration reads and writes: `&Node` and `&Graph` read the source state, and each `&mut` parameter is a
/// view (`Inputs`, `Upstream`, `Downstream`) recording edits to its region. The function may return `()` or `Result<(), MigrationError>`.
///
/// Generates a unit struct named after the function in Pascal case implementing `NodeMigration`, and registers it.
#[proc_macro_attribute]
pub fn migration(attribute: TokenStream, item: TokenStream) -> TokenStream {
	let step = parse_macro_input!(attribute as Step);
	let function = parse_macro_input!(item as ItemFn);

	expand_migration(step, function).unwrap_or_else(Error::into_compile_error).into()
}

/// Registers a migration that only switches nodes to another declaration: `rename!("module::old"@1 => "module::new"@2);`.
#[proc_macro]
pub fn rename(input: TokenStream) -> TokenStream {
	let step = parse_macro_input!(input as Step);

	expand_rename(step).unwrap_or_else(Error::into_compile_error).into()
}

/// Registers a format step converting the listed stored roots through their `Upgrade` impls:
/// `format_migration!(1 => 2, registry: v1::Registry, delta: v1::Delta);`. Roots left out keep their payloads.
#[proc_macro]
pub fn format_migration(input: TokenStream) -> TokenStream {
	let step = parse_macro_input!(input as format::FormatStep);

	let expanded = core_path().map(|core| {
		let migration = format::expand(&core, &step);
		let registration = registration(&core, quote!(register_format_migration), quote!(&FormatStep), &format!("format {}=>{}", step.from, step.to));
		quote! {
			const _: () = {
				#migration
				#registration
			};
		}
	});
	expanded.unwrap_or_else(Error::into_compile_error).into()
}

/// What one parameter of a migration function receives.
enum Argument {
	Node,
	Graph,
	View(Box<Type>),
}

fn expand_migration(step: Step, function: ItemFn) -> syn::Result<TokenStream2> {
	let core = core_path()?;

	let signature = &function.sig;
	if let Some(asyncness) = signature.asyncness {
		return Err(Error::new(asyncness.span(), "migrations run synchronously"));
	}
	if !signature.generics.params.is_empty() {
		return Err(Error::new(signature.generics.span(), "migrations can't be generic"));
	}

	let arguments = signature.inputs.iter().map(classify).collect::<syn::Result<Vec<_>>>()?;
	let views: Vec<&Type> = arguments
		.iter()
		.filter_map(|argument| match argument {
			Argument::View(view) => Some(view.as_ref()),
			_ => None,
		})
		.collect();
	let reads_graph = arguments.iter().any(|argument| matches!(argument, Argument::Graph));

	// Call the function with the source state and one extracted view per `&mut` parameter
	let view_bindings: Vec<Ident> = (0..views.len()).map(|index| format_ident!("__view_{index}")).collect();
	let mut view_index = 0;
	let call_arguments = arguments.iter().map(|argument| match argument {
		Argument::Node => quote!(context.source_node()),
		Argument::Graph => quote!(&__graph),
		Argument::View(_) => {
			let binding = &view_bindings[view_index];
			view_index += 1;
			quote!(&mut #binding)
		}
	});
	let function_name = &signature.ident;
	let propagate = match &signature.output {
		ReturnType::Default => quote!(),
		ReturnType::Type(..) => quote!(?),
	};

	let graph_binding = reads_graph.then(|| quote!(let __graph = context.graph();));
	let collect_edits = match view_bindings.as_slice() {
		[] => quote!(Ok(#core::Edits::default())),
		[binding] => quote!(#core::View::into_edits(#binding)),
		bindings => quote! {
			let mut edits = #core::Edits::default();
			#(edits.merge(#core::View::into_edits(#bindings)?);)*
			Ok(edits)
		},
	};
	let run = quote! {
		fn run(&self, context: &#core::AnchorContext) -> ::core::result::Result<#core::Edits, #core::MigrationError> {
			#graph_binding
			#(let mut #view_bindings = <#views as #core::View>::extract(context);)*
			#function_name(#(#call_arguments),*) #propagate;
			#collect_edits
		}
	};

	let network_region = reads_graph.then(|| quote!(.union(#core::Region::NETWORK)));
	let footprint = quote! {
		fn footprint(&self) -> #core::Region {
			#core::Region::ANCHOR #(.union(<#views as #core::View>::REGION))* #network_region
		}
	};

	// Every `decl!` in the body orders this migration against the ones producing and consuming that declaration
	let pins: Vec<Declaration> = scan_pins(quote!(#function))?.into_iter().filter(|pin| *pin != step.source && *pin != step.target).collect();
	let pins = (!pins.is_empty()).then(|| {
		let pins = pins.iter().map(|pin| declaration(&core, pin));
		quote! {
			fn pins(&self) -> &'static [#core::Declaration] {
				const { &[#(#pins),*] }
			}
		}
	});

	let disjoint_views = (views.len() > 1).then(|| {
		quote! {
			const _: () = ::core::assert!(
				#core::Region::disjoint(&[#(<#views as #core::View>::REGION),*]),
				"two mutable views of one migration edit overlapping regions",
			);
		}
	});

	let struct_name = format_ident!("{}", function_name.to_string().to_case(Case::Pascal), span = function_name.span());
	let visibility = &function.vis;
	let docs = function.attrs.iter().filter(|attribute| attribute.path().is_ident("doc"));
	let source = declaration(&core, &step.source);
	let target = declaration(&core, &step.target);
	let registration = registration(&core, quote!(register_migration), quote!(&#struct_name), &step_name(&step));

	Ok(quote! {
		#function

		#(#docs)*
		#visibility struct #struct_name;

		impl #core::NodeMigration for #struct_name {
			fn source(&self) -> #core::Declaration {
				#source
			}
			fn target(&self) -> #core::Declaration {
				#target
			}
			#pins
			#footprint
			#run
		}

		#disjoint_views

		const _: () = {
			#registration
		};
	})
}

fn classify(argument: &FnArg) -> syn::Result<Argument> {
	let FnArg::Typed(typed) = argument else {
		return Err(Error::new(argument.span(), "migrations are free functions"));
	};
	let Type::Reference(reference) = &*typed.ty else {
		return Err(Error::new(typed.ty.span(), "migration parameters are `&Node`, `&Graph`, or `&mut` views"));
	};

	if reference.mutability.is_some() {
		return Ok(Argument::View(reference.elem.clone()));
	}

	let Type::Path(path) = &*reference.elem else {
		return Err(Error::new(reference.elem.span(), "shared references read the source state: `&Node` or `&Graph`"));
	};
	match path.path.segments.last().map(|segment| segment.ident.to_string()).as_deref() {
		Some("Node") => Ok(Argument::Node),
		Some("Graph") => Ok(Argument::Graph),
		_ => Err(Error::new(reference.elem.span(), "shared references read the source state: `&Node` or `&Graph`")),
	}
}

fn expand_rename(step: Step) -> syn::Result<TokenStream2> {
	let core = core_path()?;
	let source = declaration(&core, &step.source);
	let target = declaration(&core, &step.target);
	let registration = registration(&core, quote!(register_migration), quote!(&MIGRATION), &step_name(&step));

	Ok(quote! {
		const _: () = {
			static MIGRATION: #core::Rename = #core::Rename::new(#source, #target);
			#registration
		};
	})
}

fn step_name(step: &Step) -> String {
	format!("{}@{}=>{}@{}", step.source.identifier, step.source.version, step.target.identifier, step.target.version)
}

/// Passes `migration` to `register` at startup: through `ctor` on native targets, and through an exported function the host calls
/// on wasm, where `ctor` isn't available. The export's name derives from `step_name`, so registering one step twice fails to link.
fn registration(core: &TokenStream2, register: TokenStream2, migration: TokenStream2, step_name: &str) -> TokenStream2 {
	let hash = blake3::hash(step_name.as_bytes()).to_hex();
	let export = format_ident!("__migration_registry_{}", &hash[..16]);

	quote! {
		use #core::ctor::ctor;

		#[cfg_attr(not(target_family = "wasm"), ctor)]
		fn register() {
			#core::#register(#migration);
		}

		#[cfg(target_family = "wasm")]
		#[unsafe(no_mangle)]
		extern "C" fn #export() {
			register();
		}
	}
}

fn declaration(core: &TokenStream2, declaration: &Declaration) -> TokenStream2 {
	let Declaration { identifier, version } = declaration;
	quote!(#core::Declaration::new(#identifier, #version))
}

/// How the expansion names `migration-core`: `crate` inside it, its (possibly renamed) dependency name elsewhere.
fn core_path() -> syn::Result<TokenStream2> {
	match proc_macro_crate::crate_name("migration-core") {
		Ok(proc_macro_crate::FoundCrate::Itself) => Ok(quote!(crate)),
		Ok(proc_macro_crate::FoundCrate::Name(name)) => {
			let name = Ident::new(&name, Span::call_site());
			Ok(quote!(::#name))
		}
		Err(error) => Err(Error::new(Span::call_site(), format!("migrations need `migration-core` as a dependency: {error}"))),
	}
}
