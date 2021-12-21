/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

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

export function createDocumentsState(editor: EditorState, dialogState: DialogState) {
	const state = reactive({
		unsaved: false,
		documents: [] as DocumentSaveState[],
		activeDocumentIndex: 0,
	});

	const selectDocument = (tabIndex: number) => {
		editor.instance.select_document(tabIndex);
	};

	const closeDocumentWithConfirmation = (tabIndex: number) => {
		console.log("Hey");
		// Close automatically if it's already saved, no confirmation is needed
		const targetDocument = state.documents[tabIndex];
		if (targetDocument.isSaved) {
			editor.instance.close_document(tabIndex);
			return;
		}

		// Switch to the document that's being prompted to close
		selectDocument(tabIndex);

		// Show the close confirmation prompt
		dialogState.createDialog("File", "Save changes before closing?", targetDocument.displayName, [
			{
				kind: "TextButton",
				callback: () => {
					editor.instance.save_document();
					dialogState.dismissDialog();
				},
				props: { label: "Save", emphasized: true, minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					editor.instance.close_document(tabIndex);
					dialogState.dismissDialog();
				},
				props: { label: "Discard", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	};

	const closeAllDocumentsWithConfirmation = () => {
		dialogState.createDialog("Copy", "Close all documents?", "Unsaved work will be lost!", [
			{
				kind: "TextButton",
				callback: () => {
					editor.instance.close_all_documents();
					dialogState.dismissDialog();
				},
				props: { label: "Discard All", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	};

	// Set up message subscriptions on creation
	editor.dispatcher.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
		state.documents = updateOpenDocumentList.open_documents.map(({ name, isSaved }) => new DocumentSaveState(name, isSaved));
	});

	editor.dispatcher.subscribeJsMessage(SetActiveDocument, (setActiveDocument) => {
		state.activeDocumentIndex = setActiveDocument.document_index;
	});

	editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseDocument, (displayConfirmationToCloseDocument) => {
		closeDocumentWithConfirmation(displayConfirmationToCloseDocument.document_index);
	});

	editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseAllDocuments, () => {
		closeAllDocumentsWithConfirmation();
	});

	editor.dispatcher.subscribeJsMessage(OpenDocumentBrowse, async () => {
		const extension = editor.rawWasm.file_save_suffix();
		const data = await upload(extension);
		editor.instance.open_document_file(data.filename, data.content);
	});

	editor.dispatcher.subscribeJsMessage(ExportDocument, (exportDocument) => {
		download(exportDocument.name, exportDocument.document);
	});

	editor.dispatcher.subscribeJsMessage(SaveDocument, (saveDocument) => {
		download(saveDocument.name, saveDocument.document);
	});

	// Get the initial documents
	editor.instance.get_open_documents_list();

	return {
		state: readonly(state),
		selectDocument,
		closeDocumentWithConfirmation,
		closeAllDocumentsWithConfirmation,
	};
}
export type DocumentsState = ReturnType<typeof createDocumentsState>;
