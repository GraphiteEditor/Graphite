use cargo_run::Error;
use cargo_run::cmd::prelude::*;
use notify::RecursiveMode;
use notify::event::{EventKind, ModifyKind};
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer, notify::RecommendedWatcher};
use std::collections::HashSet;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

const EXCLUDED_DIRECTORIES: &[&str] = &["target", ".git", "frontend/node_modules", "frontend/dist", "frontend/wrapper/pkg", "tools"];
const INCLUDED_EXTENSIONS: &[&str] = &["rs"];

const DEBOUNCE: Duration = Duration::from_millis(500);

fn main() -> ExitCode {
	let release = std::env::args().nth(1).as_deref() == Some("release");

	let _guard = match watch(release) {
		Ok(guard) => guard,
		Err(e) => {
			eprintln!("Error setting up file watcher: {e}");
			return ExitCode::FAILURE;
		}
	};
	loop {
		std::thread::park();
	}
}

struct WatchGuard {
	_debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

fn watch(release: bool) -> Result<WatchGuard, Error> {
	let root = std::env::current_dir()
		.and_then(|p| p.canonicalize())
		.map_err(|e| Error::Io(e, "Failed to resolve root for file watcher".into()))?;
	let root_clone = root.clone();

	let mut current: Option<Sequence> = None;

	let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: notify_debouncer_full::DebounceEventResult| match result {
		Ok(events) => {
			let mut seen = HashSet::new();
			let mut triggered = false;
			for ev in events {
				if !matches!(
					&ev.event.kind,
					EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Any)
				) {
					continue;
				}
				for path in &ev.event.paths {
					if is_excluded(path, &root) {
						continue;
					}
					if seen.insert(path.clone()) {
						triggered = true;
					}
				}
			}
			if triggered {
				if let Some(c) = current.take() {
					c.kill();
				}
				current = Some(sequence_then(cargo_run::frontend::build_wasm_steps(release, false), move || {
					cargo_run::frontend::heal_steps_if_corrupt(release, false)
				}));
			}
		}
		Err(errors) => {
			for e in errors {
				eprintln!("watch: {e}");
			}
		}
	})
	.map_err(|e| Error::Io(std::io::Error::other(e.to_string()), "file watcher".into()))?;

	debouncer
		.watch(&root_clone, RecursiveMode::Recursive)
		.map_err(|e| Error::Io(std::io::Error::other(e.to_string()), "file watcher".into()))?;

	Ok(WatchGuard { _debouncer: debouncer })
}

fn is_excluded(path: &Path, root: &Path) -> bool {
	let rel = path.strip_prefix(root).unwrap_or(path);
	if EXCLUDED_DIRECTORIES.iter().any(|d| rel.starts_with(d)) {
		return true;
	}
	!path.extension().and_then(|e| e.to_str()).is_some_and(|e| INCLUDED_EXTENSIONS.contains(&e))
}
