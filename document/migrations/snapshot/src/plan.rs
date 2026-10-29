//! What the next format version must declare again, and the snapshot file that declares it.

use crate::types::{Entry, ImportLeaf, Types, flatten};
use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Every frozen version, by number.
pub struct Snapshots {
	pub versions: BTreeMap<u32, Types>,
}

impl Snapshots {
	/// Every `v<N>.rs` in `directory`.
	pub fn read(directory: &Path) -> Result<Self, String> {
		let mut versions = BTreeMap::new();
		for entry in std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))? {
			let path = entry.map_err(|error| error.to_string())?.path();
			let Some(version) = path
				.file_stem()
				.and_then(|stem| stem.to_str())
				.and_then(|stem| stem.strip_prefix('v'))
				.and_then(|number| number.parse().ok())
			else {
				continue;
			};
			versions.insert(version, Types::read_snapshot(&path)?);
		}

		Ok(Self { versions })
	}

	pub fn latest(&self) -> Option<u32> {
		self.versions.keys().next_back().copied()
	}

	/// `name` as of `version`, following re-exports to the version defining it.
	pub fn entry(&self, version: u32, name: &str) -> Option<&Entry> {
		let types = self.versions.get(&version)?;
		if let Some(entry) = types.entries.get(name) {
			return Some(entry);
		}
		types.reexports.get(name).and_then(|&earlier| self.entry(earlier, name))
	}

	fn fingerprint(&self, version: u32, name: &str) -> Option<String> {
		self.entry(version, name).map(Entry::fingerprint)
	}

	fn names(&self, version: u32) -> BTreeSet<String> {
		let Some(types) = self.versions.get(&version) else { return BTreeSet::new() };
		types.entries.keys().chain(types.reexports.keys()).cloned().collect()
	}
}

/// How the live schema differs from the latest snapshot.
pub struct Plan {
	pub version: u32,
	pub previous: Option<u32>,
	/// Types whose definition or impls changed, or that are new.
	pub changed: BTreeSet<String>,
	pub removed: BTreeSet<String>,
	/// `changed` plus every type reaching one, which must be declared again to refer to the new definitions.
	pub redeclared: BTreeSet<String>,
	pub reexported: BTreeSet<String>,
}

impl Plan {
	pub fn new(live: &Types, snapshots: &Snapshots) -> Self {
		let previous = snapshots.latest();
		let changed: BTreeSet<String> = live
			.entries
			.iter()
			.filter(|(name, entry)| previous.and_then(|version| snapshots.fingerprint(version, name)) != Some(entry.fingerprint()))
			.map(|(name, _)| name.clone())
			.collect();
		let live_names: BTreeSet<String> = live.entries.keys().cloned().collect();
		let removed = previous.map(|version| snapshots.names(version)).unwrap_or_default().difference(&live_names).cloned().collect();

		// Fixpoint over "names a redeclared type", O(types²) per round
		let mut redeclared = changed.clone();
		loop {
			let reaching: Vec<String> = live
				.entries
				.iter()
				.filter(|(name, entry)| !redeclared.contains(*name) && !entry.named_paths().is_disjoint(&redeclared))
				.map(|(name, _)| name.clone())
				.collect();
			if reaching.is_empty() {
				break;
			}
			redeclared.extend(reaching);
		}
		let reexported = live_names.difference(&redeclared).cloned().collect();

		Self {
			version: previous.map_or(1, |version| version + 1),
			previous,
			changed,
			removed,
			redeclared,
			reexported,
		}
	}

	pub fn is_unchanged(&self) -> bool {
		self.redeclared.is_empty() && self.removed.is_empty()
	}

	/// The snapshot file, before rustfmt.
	pub fn render(&self, live: &Types) -> String {
		let items: Vec<syn::Item> = live.order.iter().filter(|name| self.redeclared.contains(*name)).flat_map(|name| live.entries[name].items()).collect();
		let tokens: TokenStream = items.iter().map(ToTokens::to_token_stream).collect();

		let reexports = self.previous.filter(|_| !self.reexported.is_empty()).map(|previous| {
			let module = format_ident!("v{previous}");
			let names = self.reexported.iter().map(|name| format_ident!("{name}"));
			quote!(pub use super::#module::{#(#names),*};)
		});
		let imports = imports(live, &tokens);

		let header = format!(
			"//! Format version {}, frozen by `cargo run migration snapshot`. Never edit it: old documents decode through these definitions.\n\n",
			self.version
		);
		// Separate items by blank lines, which rustfmt keeps
		let items: Vec<String> = items.iter().map(|item| item.to_token_stream().to_string()).collect();
		format!("{header}{}\n\n{}\n", quote!(#reexports #imports), items.join("\n\n"))
	}

	pub fn describe(&self) -> String {
		let list = |names: &BTreeSet<String>| names.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
		let mut lines = Vec::new();

		if !self.changed.is_empty() {
			lines.push(format!("  changed: {}", list(&self.changed)));
		}
		if !self.removed.is_empty() {
			lines.push(format!("  removed: {}", list(&self.removed)));
		}
		let carried: BTreeSet<String> = self.redeclared.difference(&self.changed).cloned().collect();
		if !carried.is_empty() {
			lines.push(format!("  declared again because they reach a changed type: {}", list(&carried)));
		}

		lines.join("\n")
	}
}

/// The external imports `items` use, grouped by path.
fn imports(live: &Types, items: &TokenStream) -> TokenStream {
	let mut used = BTreeSet::new();
	collect_idents(items.clone(), &mut used);

	let mut leaves: Vec<ImportLeaf> = Vec::new();
	for import in &live.imports {
		flatten(&import.tree, &mut Vec::new(), &mut leaves);
	}

	let mut grouped: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
	for leaf in leaves.into_iter().filter(|leaf| leaf.binding == "*" || used.contains(&leaf.binding)) {
		grouped.entry(leaf.prefix.join("::")).or_default().insert(leaf.written);
	}

	grouped
		.into_iter()
		.map(|(prefix, names)| {
			let names: Vec<String> = names.into_iter().collect();
			let source = match names.as_slice() {
				[single] => format!("use {prefix}::{single};"),
				_ => format!("use {prefix}::{{{}}};", names.join(", ")),
			};
			source.parse::<TokenStream>().unwrap_or_default()
		})
		.collect()
}

fn collect_idents(tokens: TokenStream, idents: &mut BTreeSet<String>) {
	for token in tokens {
		match token {
			TokenTree::Ident(ident) => {
				idents.insert(ident.to_string());
			}
			TokenTree::Group(group) => collect_idents(group.stream(), idents),
			_ => {}
		}
	}
}
