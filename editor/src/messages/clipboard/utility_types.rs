#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardContentRaw {
	Text(String),
	Svg(String),
	Image { data: Vec<u8>, width: u32, height: u32 },
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardContent {
	Layer(String),
	Nodes(String),
	Vector(String),
	Text(String),
	Svg(String),
	Image { data: Vec<u8>, width: u32, height: u32 },
}
