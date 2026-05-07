pub(crate) const APP_NAME: &str = "Graphite";
pub(crate) const APP_DESCRIPTION: &str = "Vector graphics editor and procedural design engine";
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) const APP_ID: &str = "art.graphite.Graphite";

#[cfg(target_os = "linux")]
pub(crate) const APP_DIRECTORY_NAME: &str = "graphite";
#[cfg(not(target_os = "linux"))]
pub(crate) const APP_DIRECTORY_NAME: &str = "Graphite";
pub(crate) const APP_LOCK_FILE_NAME: &str = "instance.lock";
pub(crate) const APP_STATE_FILE_NAME: &str = "state.ron";
pub(crate) const APP_PREFERENCES_FILE_NAME: &str = "preferences.ron";
pub(crate) const APP_DOCUMENTS_DIRECTORY_NAME: &str = "documents";

// Document type identifiers, used by per-platform OS file-type registration.
// Keep these in sync with Mac's `Info.plist` in the `graphite-desktop-bundle` crate.
pub(crate) const DOCUMENT_TYPE_IDENTIFIER: &str = "art.graphite.document";
pub(crate) const DOCUMENT_FRIENDLY_NAME: &str = "Graphite Document";
pub(crate) const DOCUMENT_MIME_TYPE: &str = "application/graphite+json";

// CEF configuration constants
pub(crate) const CEF_WINDOWLESS_FRAME_RATE: i32 = 60;
pub(crate) const CEF_MESSAGE_LOOP_MAX_ITERATIONS: usize = 10;
