/* eslint-disable max-classes-per-file */
import { reactive } from "vue";

import { DialogState } from "@/state/dialog";
import { download, upload } from "@/utilities/files";
import { EditorState } from "@/state/wasm-loader";
import {
	DisplayConfirmationToCloseAllDocuments,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	OpenDocumentBrowse,
	SaveDocument,
	SetActiveDocument,
	UpdateOpenDocumentsList,
} from "@/dispatcher/js-messages";

class DocumentSaveState {
	readonly displayName: string;

	constructor(readonly name: string, readonly isSaved: boolean) {
		this.displayName = `${name}${isSaved ? "" : "*"}`;
	}
}

export class DocumentsState {
	state = reactive({
		unsaved: false,
		documents: [] as DocumentSaveState[],
		activeDocumentIndex: 0,
	});

	constructor(private editor: EditorState, private dialogState: DialogState) {
		this.setupJsMessageListeners();
		// Get the initial documents
		editor.instance.get_open_documents_list();
	}

	selectDocument(tabIndex: number) {
		this.editor.instance.select_document(tabIndex);
	}

	closeDocumentWithConfirmation(tabIndex: number) {
		this.selectDocument(tabIndex);

		const targetDocument = this.state.documents[tabIndex];
		if (targetDocument.isSaved) {
			this.editor.instance.close_document(tabIndex);
			return;
		}

		// Show the document is being prompted to close
		this.dialogState.createDialog("File", "Save changes before closing?", targetDocument.displayName, [
			{
				kind: "TextButton",
				callback: () => {
					this.editor.instance.save_document();
					this.dialogState.dismissDialog();
				},
				props: { label: "Save", emphasized: true, minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					this.editor.instance.close_document(tabIndex);
					this.dialogState.dismissDialog();
				},
				props: { label: "Discard", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					this.dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	}

	closeAllDocumentsWithConfirmation() {
		this.dialogState.createDialog("Copy", "Close all documents?", "Unsaved work will be lost!", [
			{
				kind: "TextButton",
				callback: () => {
					this.editor.instance.close_all_documents();
					this.dialogState.dismissDialog();
				},
				props: { label: "Discard All", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					this.dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	}

	private setupJsMessageListeners() {
		this.editor.dispatcher.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
			this.state.documents = updateOpenDocumentList.open_documents.map(({ name, isSaved }) => new DocumentSaveState(name, isSaved));
		});

		this.editor.dispatcher.subscribeJsMessage(SetActiveDocument, (setActiveDocument) => {
			this.state.activeDocumentIndex = setActiveDocument.document_index;
		});

		this.editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseDocument, (displayConfirmationToCloseDocument) => {
			this.closeDocumentWithConfirmation(displayConfirmationToCloseDocument.document_index);
		});

		this.editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseAllDocuments, () => {
			this.closeAllDocumentsWithConfirmation();
		});

		this.editor.dispatcher.subscribeJsMessage(OpenDocumentBrowse, async () => {
			const extension = this.editor.rawWasm.file_save_suffix();
			const data = await upload(extension);
			this.editor.instance.open_document_file(data.filename, data.content);
		});

		this.editor.dispatcher.subscribeJsMessage(ExportDocument, (exportDocument) => {
			download(exportDocument.name, exportDocument.document);
		});

		this.editor.dispatcher.subscribeJsMessage(SaveDocument, (saveDocument) => {
			download(saveDocument.name, saveDocument.document);
		});
	}
}
