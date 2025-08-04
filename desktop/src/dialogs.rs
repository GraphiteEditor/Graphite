use std::path::PathBuf;

use rfd::AsyncFileDialog;

pub(crate) async fn dialog_open_graphite_file() -> Option<PathBuf> {
	let file = AsyncFileDialog::new().add_filter("Graphite", &["graphite"]).set_title("Open Graphite Document").pick_file().await;
	file.map(|f| f.path().to_path_buf())
}
