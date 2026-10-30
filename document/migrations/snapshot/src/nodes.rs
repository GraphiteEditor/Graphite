//! Finds every `#[node_macro::node]` function in the node graph crates, with the identifier and version the macro gives it.

use convert_case::{Case, Casing};
use std::path::{Path, PathBuf};
use syn::punctuated::Punctuated;

/// One node as its source declares it.
pub struct NodeSource {
	/// What `ProtoNodeIdentifier` the macro generates: the `path(...)` override or the module path, then the Pascal-case
	/// function name with a `Node` suffix.
	pub identifier: String,
	pub version: u32,
	pub function: String,
	pub file: PathBuf,
	/// The parameters and output as written, without their attributes. Comparing it tells whether stored inputs still fit.
	pub signature: String,
}

/// Every node in a crate under `root` with a `src/lib.rs`. Files that don't parse are skipped, since they don't build either.
pub fn find_nodes(root: &Path) -> Result<Vec<NodeSource>, String> {
	let mut manifests = Vec::new();
	collect_manifests(root, &mut manifests)?;

	let mut nodes = Vec::new();
	for manifest in manifests {
		let Some(directory) = manifest.parent() else { continue };
		let library = directory.join("src/lib.rs");
		if !library.exists() {
			continue;
		}

		let crate_name = library_name(&manifest)?;
		walk_file(&library, &directory.join("src"), &[crate_name], &mut nodes);
	}

	nodes.sort_by(|a, b| a.identifier.cmp(&b.identifier));
	Ok(nodes)
}

fn collect_manifests(directory: &Path, manifests: &mut Vec<PathBuf>) -> Result<(), String> {
	let entries = std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
	for entry in entries.filter_map(Result::ok) {
		let path = entry.path();
		if path.is_dir() && path.file_name().is_some_and(|name| name != "target" && name != "node_modules") {
			collect_manifests(&path, manifests)?;
		} else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
			manifests.push(path);
		}
	}
	Ok(())
}

/// The crate's name in paths: `[lib] name`, or the package name with dashes as underscores.
fn library_name(manifest: &Path) -> Result<String, String> {
	let source = std::fs::read_to_string(manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;

	let mut section = "";
	let (mut package, mut library) = (None, None);
	for line in source.lines().map(str::trim) {
		if line.starts_with('[') {
			section = line;
			continue;
		}
		let Some(value) = line.strip_prefix("name").map(str::trim_start).and_then(|rest| rest.strip_prefix('=')) else {
			continue;
		};
		let value = value.trim().trim_matches('"').to_string();
		match section {
			"[package]" => package = Some(value),
			"[lib]" => library = Some(value),
			_ => {}
		}
	}

	library.or(package).map(|name| name.replace('-', "_")).ok_or_else(|| format!("{}: no package name", manifest.display()))
}

/// `file` is the module at `module_path`, whose `mod child;` declarations live in `children`.
fn walk_file(file: &Path, children: &Path, module_path: &[String], nodes: &mut Vec<NodeSource>) {
	let Ok(source) = std::fs::read_to_string(file) else { return };
	let Ok(parsed) = syn::parse_file(&source) else { return };
	walk_items(&parsed.items, file, children, module_path, nodes);
}

fn walk_items(items: &[syn::Item], file: &Path, children: &Path, module_path: &[String], nodes: &mut Vec<NodeSource>) {
	for item in items {
		match item {
			syn::Item::Fn(function) => {
				if let Some(node) = node_source(function, file, module_path) {
					nodes.push(node);
				}
			}
			syn::Item::Mod(module) => {
				let name = module.ident.to_string();
				let path: Vec<String> = module_path.iter().cloned().chain(std::iter::once(name.clone())).collect();
				let directory = children.join(&name);

				match &module.content {
					Some((_, items)) => walk_items(items, file, &directory, &path, nodes),
					None => {
						let flat = children.join(format!("{name}.rs"));
						let nested = directory.join("mod.rs");
						let module_file = if flat.exists() { flat } else { nested };
						walk_file(&module_file, &directory, &path, nodes);
					}
				}
			}
			_ => {}
		}
	}
}

fn node_source(function: &syn::ItemFn, file: &Path, module_path: &[String]) -> Option<NodeSource> {
	let attribute = function.attrs.iter().find(|attribute| {
		let segments: Vec<String> = attribute.path().segments.iter().map(|segment| segment.ident.to_string()).collect();
		matches!(segments.iter().map(String::as_str).collect::<Vec<_>>().as_slice(), ["node_macro", "node"] | ["node"])
	})?;
	let arguments = attribute.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated).ok()?;

	let mut path = module_path.join("::");
	let mut version = 1;
	for argument in arguments {
		let syn::Meta::List(list) = argument else { continue };
		if list.path.is_ident("path") {
			path = list.tokens.to_string().replace(' ', "");
		} else if list.path.is_ident("version")
			&& let Ok(literal) = syn::parse2::<syn::LitInt>(list.tokens.clone())
		{
			version = literal.base10_parse().ok()?;
		}
	}

	let function_name = function.sig.ident.to_string();
	let parameters = function.sig.inputs.iter().map(|parameter| match parameter {
		syn::FnArg::Typed(typed) => {
			let (pattern, ty) = (&typed.pat, &typed.ty);
			quote::quote!(#pattern: #ty).to_string()
		}
		syn::FnArg::Receiver(receiver) => quote::quote!(#receiver).to_string(),
	});
	let output = &function.sig.output;
	let signature = format!("({}) {}", parameters.collect::<Vec<_>>().join(", "), quote::quote!(#output));

	Some(NodeSource {
		identifier: format!("{path}::{}Node", function_name.to_case(Case::Pascal)),
		version,
		function: function_name,
		file: file.to_path_buf(),
		signature,
	})
}
