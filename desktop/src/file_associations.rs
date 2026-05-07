//! Per-launch file type registration with the OS.
//!
//! Mac handles this declaratively via the bundle's `Info.plist` (see `desktop/bundle/src/mac.rs`),
//! so this module is a no-op there. Windows requires writing registry entries, which this module does
//! idempotently on each launch. It re-registers only when the executable's path has changed.

// TODO: Linux support

#[cfg(target_os = "windows")]
mod win;

pub(crate) fn register_with_os() {
	#[cfg(target_os = "windows")]
	win::register();
}
