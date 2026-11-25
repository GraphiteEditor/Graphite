// TODO(Timon): Replace with generalized icon crate

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub fn svg(name: &str) -> Option<&'static str> {
	let index = SOURCES.binary_search_by_key(&name, |&(name, _)| name).ok()?;
	Some(SOURCES[index].1)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_icons_are_found_by_the_names_the_frontend_gives_them() {
		let icons_ts = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../frontend/src/icons.ts")).unwrap();
		let imported = icons_ts.lines().filter_map(|line| line.strip_prefix("import ")?.split_once(" from \"/../branding/assets/"));
		let names = imported
			.filter(|(_, path)| path.starts_with("16x16-nodes/") || path.starts_with("16x16-ui-general/"))
			.map(|(name, _)| name);
		let missing: Vec<_> = names.filter(|name| svg(name).is_none()).collect();
		assert!(SOURCES.is_empty() || missing.is_empty(), "{missing:?}");
	}
}
