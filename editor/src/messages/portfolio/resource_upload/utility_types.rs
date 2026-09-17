use crate::messages::frontend::utility_types::FileFilter;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::NodeId;

/// The raster image formats the editor decodes, by file extension.
pub const RASTER_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif"];
pub const VECTOR_IMAGE_EXTENSIONS: &[&str] = &["svg"];

/// The dialog error shown for a file that does not decode as a raster image.
pub const UNSUPPORTED_IMAGE_FILE: &str = "The loaded file is not a supported bitmap image format.";

/// The file dialog filter for the raster image formats the editor decodes.
pub fn raster_image_file_filter() -> FileFilter {
	FileFilter {
		name: "Image".into(),
		extensions: RASTER_IMAGE_EXTENSIONS.iter().map(|extension| extension.to_string()).collect(),
	}
}

/// The file dialog filter for every image the editor opens or imports, vector as well as raster.
pub fn image_file_filter() -> FileFilter {
	FileFilter {
		name: "Image".into(),
		extensions: RASTER_IMAGE_EXTENSIONS.iter().chain(VECTOR_IMAGE_EXTENSIONS.iter()).map(|ext| ext.to_string()).collect(),
	}
}

/// The pixel size of a file that fully decodes as a raster image.
pub fn decoded_image_size(data: &[u8]) -> Option<(u32, u32)> {
	image::load_from_memory(data).ok().map(|image| (image.width(), image.height()))
}

/// What a file must decode as before a node input accepts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ResourceFileKind {
	#[default]
	Any,
	RasterImage,
}

impl ResourceFileKind {
	/// The file dialog filters offered for this kind.
	pub fn filters(self) -> Vec<FileFilter> {
		match self {
			Self::Any => Vec::new(),
			Self::RasterImage => vec![raster_image_file_filter()],
		}
	}

	/// The dialog error to show when the file's contents do not decode as this kind.
	pub fn rejection(self, data: &[u8]) -> Option<&'static str> {
		match self {
			Self::Any => None,
			Self::RasterImage => decoded_image_size(data).is_none().then_some(UNSUPPORTED_IMAGE_FILE),
		}
	}
}

/// Where an uploaded file goes once it is stored as a resource.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UploadTarget {
	/// A node input that accepts the given kind of file.
	NodeInput { node_id: NodeId, input_index: usize, kind: ResourceFileKind },
	/// A new image layer in the active document, centered on the mouse or else the viewport.
	Layer {
		mouse: Option<(f64, f64)>,
		parent_and_insert_index: Option<(LayerNodeIdentifier, usize)>,
	},
	/// A new document sized to the image.
	Document,
}

impl UploadTarget {
	/// The file dialog filters for what this target accepts.
	pub fn filters(self) -> Vec<FileFilter> {
		match self {
			Self::NodeInput { kind, .. } => kind.filters(),
			Self::Layer { .. } | Self::Document => vec![raster_image_file_filter()],
		}
	}
}

/// An uploaded image by its stored resource and pixel size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImageResource {
	pub resource_id: ResourceId,
	pub width: u32,
	pub height: u32,
}
