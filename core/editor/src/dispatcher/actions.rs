use document_core::LayerId;

pub enum Action {
	SelectSelectTool,
	SelectEllipseTool,
	Undo,
	Redo,
	IncreaseSize,
	DecreaseSize,
	Save,
	SelectDocument(usize),
	DeleteLayer(Vec<LayerId>),
	// …
}
