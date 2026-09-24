use crate::wrapper::messages::PersistedState;

pub(crate) fn read_state() -> PersistedState {
	if let Err(error) = migrate_documents() {
		tracing::error!("Autosave migration failed: {error}");
	}
	let path = state_file_path();
	let data = match std::fs::read_to_string(&path) {
		Ok(d) => d,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
			tracing::info!("No persistent data file found at {path:?}, starting fresh");
			return PersistedState::default();
		}
		Err(e) => {
			tracing::error!("Failed to read persistent data from disk: {e}");
			return PersistedState::default();
		}
	};
	match ron::from_str(&data) {
		Ok(d) => d,
		Err(e) => {
			tracing::error!("Failed to deserialize persistent data: {e}");
			PersistedState::default()
		}
	}
}

pub(crate) fn write_state(state: PersistedState) {
	let state: &PersistedState = &state;
	let data = match ron::ser::to_string_pretty(state, Default::default()) {
		Ok(d) => d,
		Err(e) => {
			tracing::error!("Failed to serialize persistent data: {e}");
			return;
		}
	};
	if let Err(e) = std::fs::write(state_file_path(), data) {
		tracing::error!("Failed to write persistent data to disk: {e}");
	}
}

fn state_file_path() -> std::path::PathBuf {
	let mut path = crate::dirs::app_data_dir();
	path.push(crate::consts::APP_STATE_FILE_NAME);
	path
}

// TODO: Remove this migration code
fn migrate_documents() -> std::io::Result<()> {
	let root = crate::dirs::app_autosave_documents_dir();
	let entries = match std::fs::read_dir(&root) {
		Ok(entries) => entries,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => return Err(error),
	};
	for entry in entries {
		let entry = entry?;
		let path = entry.path();
		if !entry.file_type()?.is_file() || path.extension().and_then(|s| s.to_str()) != Some(graphite_desktop_wrapper::FILE_EXTENSION) {
			continue;
		}
		let Some(id) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| u64::from_str_radix(s, 16).ok()) else {
			continue;
		};
		let destination = root.join(format!("{id:016x}"));
		if !destination.join("legacy.graphite").exists() && !destination.join("manifest.json").exists() {
			std::fs::create_dir_all(&destination)?;
			let staging = destination.join("legacy.graphite.migrating");
			std::fs::copy(&path, &staging)?;
			std::fs::rename(staging, destination.join("legacy.graphite"))?;
		}
		let backup = path.with_extension(format!("{}.migrated", graphite_desktop_wrapper::FILE_EXTENSION));
		if backup.exists() {
			return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("Migration backup already exists: {}", backup.display())));
		}
		std::fs::rename(path, backup)?;
	}
	Ok(())
}
