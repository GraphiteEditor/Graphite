use std::path::PathBuf;

use rfd::AsyncFileDialog;

pub(crate) async fn dialog_open_graphite_file() -> Option<PathBuf> {
	AsyncFileDialog::new()
		.add_filter("Graphite", &["graphite"])
		.set_title("Open Graphite Document")
		.pick_file()
		.await
		.map(|f| f.path().to_path_buf())
}

pub(crate) async fn dialog_save_graphite_file(name: String) -> Option<PathBuf> {
	AsyncFileDialog::new()
		.add_filter("Graphite", &["graphite"])
		.set_title("Save Graphite Document")
		.set_file_name(name)
		.save_file()
		.await
		.map(|f| f.path().to_path_buf())
}
