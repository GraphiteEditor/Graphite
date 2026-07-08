use std::fs::File;
#[cfg(feature = "embedded_resources")]
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct Resource {
	pub(crate) reader: ResourceReader,
	pub(crate) mimetype: Option<String>,
}

#[derive(Clone)]
pub(crate) enum ResourceReader {
	#[cfg(feature = "embedded_resources")]
	Embedded(io::Cursor<&'static [u8]>),
	File(Arc<File>),
}
impl Read for ResourceReader {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		match self {
			#[cfg(feature = "embedded_resources")]
			ResourceReader::Embedded(cursor) => cursor.read(buf),
			ResourceReader::File(file) => file.as_ref().read(buf),
		}
	}
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WebResources {
	Embedded,
	External(PathBuf),
}

pub(crate) fn load(path: PathBuf) -> Option<Resource> {
	let resources = if cfg!(feature = "embedded_resources") {
		WebResources::Embedded
	} else {
		let path = std::env::var("GRAPHITE_RESOURCES").expect("GRAPHITE_RESOURCES must point to the frontend assets when embedded resources are disabled");
		WebResources::External(path.into())
	};

	let path = if path.as_os_str().is_empty() { PathBuf::from("index.html") } else { path };

	let mimetype = match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
		"html" => Some("text/html".to_string()),
		"css" => Some("text/css".to_string()),
		"txt" => Some("text/plain".to_string()),
		"wasm" => Some("application/wasm".to_string()),
		"js" => Some("application/javascript".to_string()),
		"png" => Some("image/png".to_string()),
		"jpg" | "jpeg" => Some("image/jpeg".to_string()),
		"svg" => Some("image/svg+xml".to_string()),
		"xml" => Some("application/xml".to_string()),
		"json" => Some("application/json".to_string()),
		"ico" => Some("image/x-icon".to_string()),
		"woff" => Some("font/woff".to_string()),
		"woff2" => Some("font/woff2".to_string()),
		"ttf" => Some("font/ttf".to_string()),
		"otf" => Some("font/otf".to_string()),
		"webmanifest" => Some("application/manifest+json".to_string()),
		"graphite" => Some("application/graphite+json".to_string()),
		_ => None,
	};

	match resources {
		WebResources::Embedded => {
			#[cfg(feature = "embedded_resources")]
			{
				if let Some(resources) = &graphite_desktop_embedded_resources::EMBEDDED_RESOURCES
					&& let Some(file) = resources.get_file(&path)
				{
					return Some(Resource {
						reader: ResourceReader::Embedded(io::Cursor::new(file.contents())),
						mimetype,
					});
				}
				None
			}
			#[cfg(not(feature = "embedded_resources"))]
			{
				tracing::error!("Embedded resources requested but the embedded_resources feature is disabled");
				None
			}
		}
		WebResources::External(dir) => {
			let file_path = dir.join(path.strip_prefix("/").unwrap_or(&path));
			if file_path.exists()
				&& file_path.is_file()
				&& let Ok(file) = std::fs::File::open(file_path)
			{
				return Some(Resource {
					reader: ResourceReader::File(file.into()),
					mimetype,
				});
			}
			None
		}
	}
}
