pub(crate) const APP_NAME: &str = "Graphite";
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) const APP_ID: &str = "art.graphite.Graphite";

#[cfg(target_os = "linux")]
pub(crate) const APP_DIRECTORY_NAME: &str = "graphite";
#[cfg(not(target_os = "linux"))]
pub(crate) const APP_DIRECTORY_NAME: &str = "Graphite";
pub(crate) const APP_LOCK_FILE_NAME: &str = "instance.lock";
pub(crate) const APP_SOCKET_FILE_NAME: &str = "instance.sock";
pub(crate) const APP_STATE_FILE_NAME: &str = "state.ron";
pub(crate) const APP_PREFERENCES_FILE_NAME: &str = "preferences.ron";
pub(crate) const APP_DOCUMENTS_DIRECTORY_NAME: &str = "documents";
pub(crate) const APP_RESOURCES_DIRECTORY_NAME: &str = "resources";
