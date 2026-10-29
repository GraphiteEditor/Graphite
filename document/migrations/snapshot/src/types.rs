//! Reads stored type definitions, from the live schema module or a frozen snapshot, into one normalized form.

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;

/// Types a stored field may name without being part of the schema. Their encoding is owned by std or serde, or, for the
/// resource IDs, promised never to change.
const EXTERNAL_TYPES: &[&str] = &[
	"bool",
	"char",
	"str",
	"u8",
	"u16",
	"u32",
	"u64",
	"u128",
	"usize",
	"i8",
	"i16",
	"i32",
	"i64",
	"i128",
	"isize",
	"f32",
	"f64",
	"String",
	"Vec",
	"Option",
	"Box",
	"Cow",
	"HashMap",
	"BTreeMap",
	"HashSet",
	"BTreeSet",
	"NonZeroU128",
	"ResourceId",
	"ResourceHash",
];

/// One stored type: its definition and the impls on it.
pub struct Entry {
	pub definition: syn::Item,
	pub impls: Vec<syn::ItemImpl>,
}

impl Entry {
	/// Normalized tokens, equal exactly when the definition and its impls are. Trailing commas don't count, since rustfmt
	/// drops them when a snapshot's doc-less definitions fit on one line.
	pub fn fingerprint(&self) -> String {
		let text = |item: &dyn ToTokens| strip_trailing_commas(item.to_token_stream()).to_string();
		let mut impls: Vec<String> = self.impls.iter().map(|implementation| text(implementation)).collect();
		impls.sort();

		format!("{} {}", text(&self.definition), impls.join(" "))
	}

	pub fn items(&self) -> impl Iterator<Item = syn::Item> + '_ {
		std::iter::once(self.definition.clone()).chain(self.impls.iter().cloned().map(syn::Item::Impl))
	}

	/// Every identifier a path in the definition or its impls names.
	pub fn named_paths(&self) -> BTreeSet<String> {
		let mut collector = PathIdents::default();
		collector.visit_item(&self.definition);
		for implementation in &self.impls {
			collector.visit_item_impl(implementation);
		}
		collector.idents
	}

	/// The types the stored fields name, without the definition's own generic parameters.
	fn stored_types(&self) -> BTreeSet<String> {
		let mut collector = StoredTypes::default();
		let generics = match &self.definition {
			syn::Item::Struct(definition) => {
				collector.visit_fields(&definition.fields);
				&definition.generics
			}
			syn::Item::Enum(definition) => {
				for variant in &definition.variants {
					collector.visit_fields(&variant.fields);
				}
				&definition.generics
			}
			syn::Item::Type(definition) => {
				collector.visit_type(&definition.ty);
				&definition.generics
			}
			_ => return BTreeSet::new(),
		};

		let parameters: BTreeSet<String> = generics.type_params().map(|parameter| parameter.ident.to_string()).collect();
		collector.idents.difference(&parameters).cloned().collect()
	}
}

/// The stored types of one module, in source order, with the imports their definitions use.
#[derive(Default)]
pub struct Types {
	pub entries: BTreeMap<String, Entry>,
	pub order: Vec<String>,
	pub imports: Vec<syn::ItemUse>,
	/// Names a snapshot re-exports from an earlier version, and which.
	pub reexports: BTreeMap<String, u32>,
}

impl Types {
	/// Every `.rs` file of the live schema module, except its `mod.rs`.
	pub fn read_schema(directory: &Path) -> Result<Self, String> {
		let mut files: Vec<_> = std::fs::read_dir(directory)
			.map_err(|error| format!("{}: {error}", directory.display()))?
			.filter_map(|entry| entry.ok().map(|entry| entry.path()))
			.filter(|path| path.extension().is_some_and(|extension| extension == "rs") && path.file_name().is_some_and(|name| name != "mod.rs"))
			.collect();
		files.sort();

		let mut types = Self::default();
		for file in files {
			types.read_file(&file)?;
		}
		types.check_stored_types()?;

		Ok(types)
	}

	/// One frozen snapshot file.
	pub fn read_snapshot(file: &Path) -> Result<Self, String> {
		let mut types = Self::default();
		types.read_file(file)?;
		Ok(types)
	}

	fn read_file(&mut self, file: &Path) -> Result<(), String> {
		let source = std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
		let parsed = syn::parse_file(&source).map_err(|error| format!("{}: {error}", file.display()))?;

		// Impls attach to definitions, which may come later in the file
		let mut impls = Vec::new();
		for item in parsed.items {
			let item = normalize(item).map_err(|error| format!("{}: {error}", file.display()))?;
			match item {
				syn::Item::Use(import) => self.read_import(import),
				syn::Item::Struct(syn::ItemStruct { ref ident, .. }) | syn::Item::Enum(syn::ItemEnum { ref ident, .. }) | syn::Item::Type(syn::ItemType { ref ident, .. }) => {
					let name = ident.to_string();
					if self.entries.contains_key(&name) {
						return Err(format!("{}: `{name}` is defined twice", file.display()));
					}
					self.order.push(name.clone());
					self.entries.insert(name, Entry { definition: item, impls: Vec::new() });
				}
				syn::Item::Impl(implementation) => impls.push(implementation),
				other => {
					let kind = other.to_token_stream().into_iter().next().map(|token| token.to_string()).unwrap_or_default();
					return Err(format!("{}: `{kind}` item: the schema holds type definitions, their impls, and imports only", file.display()));
				}
			}
		}

		for implementation in impls {
			let Some(name) = self_type_name(&implementation) else {
				return Err(format!("{}: an impl on a type that isn't a plain path", file.display()));
			};
			let Some(entry) = self.entries.get_mut(&name) else {
				return Err(format!("{}: impl on `{name}`, which the schema doesn't define", file.display()));
			};
			entry.impls.push(implementation);
		}

		Ok(())
	}

	/// Keeps external imports, records a snapshot's re-exports, and drops imports of sibling schema modules.
	fn read_import(&mut self, import: syn::ItemUse) {
		let syn::UseTree::Path(root) = &import.tree else {
			self.imports.push(import);
			return;
		};

		let root_name = root.ident.to_string();
		if root_name == "super"
			&& let syn::UseTree::Path(version) = &*root.tree
			&& let Some(version_number) = version.ident.to_string().strip_prefix('v').and_then(|number| number.parse().ok())
		{
			let mut leaves = Vec::new();
			flatten(&version.tree, &mut Vec::new(), &mut leaves);
			for leaf in leaves {
				self.reexports.insert(leaf.binding, version_number);
			}
			return;
		}
		if matches!(root_name.as_str(), "super" | "crate" | "self") {
			return;
		}

		self.imports.push(import);
	}

	/// Every type a stored field names must be frozen with the schema, or have an encoding nobody here can change.
	fn check_stored_types(&self) -> Result<(), String> {
		for (name, entry) in &self.entries {
			for stored in entry.stored_types() {
				if !self.entries.contains_key(&stored) && !EXTERNAL_TYPES.contains(&stored.as_str()) {
					return Err(format!("`{name}` stores `{stored}`, which is defined outside the schema module"));
				}
			}
		}
		Ok(())
	}
}

/// One imported name: the path leading to it, the name as written (with any rename), and what it binds.
pub struct ImportLeaf {
	pub prefix: Vec<String>,
	pub written: String,
	pub binding: String,
}

pub fn flatten(tree: &syn::UseTree, prefix: &mut Vec<String>, leaves: &mut Vec<ImportLeaf>) {
	match tree {
		syn::UseTree::Path(path) => {
			prefix.push(path.ident.to_string());
			flatten(&path.tree, prefix, leaves);
			prefix.pop();
		}
		syn::UseTree::Name(name) => {
			let written = name.ident.to_string();
			let binding = if written == "self" { prefix.last().cloned().unwrap_or_default() } else { written.clone() };
			leaves.push(ImportLeaf {
				prefix: prefix.clone(),
				written,
				binding,
			});
		}
		syn::UseTree::Rename(rename) => leaves.push(ImportLeaf {
			prefix: prefix.clone(),
			written: format!("{} as {}", rename.ident, rename.rename),
			binding: rename.rename.to_string(),
		}),
		syn::UseTree::Glob(_) => leaves.push(ImportLeaf {
			prefix: prefix.clone(),
			written: "*".to_string(),
			binding: "*".to_string(),
		}),
		syn::UseTree::Group(group) => {
			for tree in &group.items {
				flatten(tree, prefix, leaves);
			}
		}
	}
}

fn self_type_name(implementation: &syn::ItemImpl) -> Option<String> {
	let syn::Type::Path(path) = &*implementation.self_ty else { return None };
	path.path.segments.last().map(|segment| segment.ident.to_string())
}

/// Strips docs, makes every definition and field public, and resolves paths into the schema to bare names, so the live
/// module and a frozen snapshot of it read the same.
fn normalize(item: syn::Item) -> Result<syn::Item, String> {
	let tokens = strip_docs(item.to_token_stream());
	let mut item: syn::Item = syn::parse2(tokens).map_err(|error| error.to_string())?;

	SchemaPaths.visit_item_mut(&mut item);

	let public = || syn::parse_quote!(pub);
	match &mut item {
		syn::Item::Struct(definition) => {
			definition.vis = public();
			for field in definition.fields.iter_mut() {
				field.vis = public();
			}
		}
		syn::Item::Enum(definition) => definition.vis = public(),
		syn::Item::Type(definition) => definition.vis = public(),
		syn::Item::Impl(implementation) if implementation.trait_.is_none() => {
			for member in &mut implementation.items {
				match member {
					syn::ImplItem::Fn(function) => function.vis = public(),
					syn::ImplItem::Const(constant) => constant.vis = public(),
					_ => {}
				}
			}
		}
		_ => {}
	}

	Ok(item)
}

/// Removes `#[doc = ...]` attributes at any depth. Doc edits never change the format.
fn strip_docs(tokens: TokenStream) -> TokenStream {
	let mut output = Vec::new();
	let mut tokens = tokens.into_iter().peekable();

	while let Some(token) = tokens.next() {
		if let TokenTree::Punct(punct) = &token
			&& punct.as_char() == '#'
			&& let Some(TokenTree::Group(group)) = tokens.peek()
			&& is_doc_attribute(group)
		{
			tokens.next();
			continue;
		}

		match token {
			TokenTree::Group(group) => {
				let mut stripped = Group::new(group.delimiter(), strip_docs(group.stream()));
				stripped.set_span(group.span());
				output.push(TokenTree::Group(stripped));
			}
			other => output.push(other),
		}
	}

	output.into_iter().collect()
}

fn strip_trailing_commas(tokens: TokenStream) -> TokenStream {
	let mut output: Vec<TokenTree> = tokens
		.into_iter()
		.map(|token| match token {
			TokenTree::Group(group) => {
				let mut stripped = Group::new(group.delimiter(), strip_trailing_commas(group.stream()));
				stripped.set_span(group.span());
				TokenTree::Group(stripped)
			}
			other => other,
		})
		.collect();

	if matches!(output.last(), Some(TokenTree::Punct(punct)) if punct.as_char() == ',') {
		output.pop();
	}
	output.into_iter().collect()
}

fn is_doc_attribute(group: &Group) -> bool {
	group.delimiter() == Delimiter::Bracket && matches!(group.stream().into_iter().next(), Some(TokenTree::Ident(ident)) if ident == "doc")
}

/// Turns `crate::Node`, `super::model::Node` and the like into `Node`. Schema types are capitalized and modules aren't, so
/// leading `crate`, `super`, and lowercase segments go.
struct SchemaPaths;

impl VisitMut for SchemaPaths {
	fn visit_path_mut(&mut self, path: &mut syn::Path) {
		let starts_in_crate = path.leading_colon.is_none() && path.segments.first().is_some_and(|segment| segment.ident == "crate" || segment.ident == "super");
		if starts_in_crate {
			let kept: Vec<_> = path
				.segments
				.iter()
				.skip_while(|segment| segment.ident == "crate" || segment.ident == "super" || segment.ident.to_string().starts_with(|character: char| character.is_lowercase()))
				.cloned()
				.collect();
			if !kept.is_empty() {
				path.segments = kept.into_iter().collect();
			}
		}

		syn::visit_mut::visit_path_mut(self, path);
	}
}

#[derive(Default)]
struct PathIdents {
	idents: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PathIdents {
	fn visit_path(&mut self, path: &'ast syn::Path) {
		self.idents.extend(path.segments.iter().map(|segment| segment.ident.to_string()));
		syn::visit::visit_path(self, path);
	}
}

/// The last segment of every type path in the visited fields, generic arguments included.
#[derive(Default)]
struct StoredTypes {
	idents: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for StoredTypes {
	fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
		if let Some(segment) = path.path.segments.last() {
			self.idents.insert(segment.ident.to_string());
		}
		syn::visit::visit_type_path(self, path);
	}

	// Field attributes like `#[serde(default)]` name no stored types
	fn visit_attribute(&mut self, _: &'ast syn::Attribute) {}
}
