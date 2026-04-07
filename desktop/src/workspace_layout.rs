pub(crate) fn write(layout_json: &str) {
	std::fs::write(file_path(), layout_json).unwrap_or_else(|e| {
		tracing::error!("Failed to write workspace layout to disk: {e}");
	});
}

pub(crate) fn read() -> Option<String> {
	std::fs::read_to_string(file_path()).ok()
}

fn file_path() -> std::path::PathBuf {
	let mut path = crate::dirs::app_data_dir();
	path.push("workspace_layout.json");
	path
}
