use std::path::PathBuf;

use crate::dirs::{ensure_dir_exists, graphite_data_dir};

static CEF_DIR_NAME: &str = "browser";

pub(crate) fn create_instance_dir() -> PathBuf {
	let instance_id: String = (0..32).map(|_| format!("{:x}", rand::random::<u8>() % 16)).collect();
	let path = graphite_data_dir().join(CEF_DIR_NAME).join(instance_id);
	ensure_dir_exists(&path);
	path
}
