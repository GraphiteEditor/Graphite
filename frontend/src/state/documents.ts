/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { DialogState } from "@/state/dialog";
import { download, upload } from "@/utilities/files";
import { EditorState } from "@/state/wasm-loader";
import {
	DisplayConfirmationToCloseAllDocuments,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	FrontendDocumentState,
	OpenDocumentBrowse,
	SaveDocument,
	SetActiveDocument,
	UpdateOpenDocumentsList,
} from "@/dispatcher/js-messages";

export function createDocumentsState(editor: EditorState, dialogState: DialogState) {
	const state = reactive({
		unsaved: false,
		documents: [] as FrontendDocumentState[],
		activeDocumentIndex: 0,
	});

	const selectDocument = (documentId: BigInt) => {
		editor.instance.select_document(documentId);
	};

	const closeDocumentWithConfirmation = async (documentId: BigInt) => {
		// Assume we receive a correct document id
		const targetDocument = state.documents.find((doc) => doc.id === documentId) as FrontendDocumentState;

		// Close automatically if it's already saved, no confirmation is needed
		if (targetDocument.is_saved) {
			editor.instance.close_document(targetDocument.id);
			return;
		}

		// Switch to the document that's being prompted to close
		await selectDocument(targetDocument.id);

		const tabLabel = targetDocument.displayName;

		// Show the close confirmation prompt
		dialogState.createDialog("File", "Save changes before closing?", tabLabel, [
			{
				kind: "TextButton",
				callback: async () => {
					editor.instance.save_document();
					dialogState.dismissDialog();
				},
				props: { label: "Save", emphasized: true, minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: async () => {
					editor.instance.close_document(targetDocument.id);
					dialogState.dismissDialog();
				},
				props: { label: "Discard", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: async () => {
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
		state.documents = updateOpenDocumentList.open_documents;
	});

	editor.dispatcher.subscribeJsMessage(SetActiveDocument, (setActiveDocument) => {
		// Assume we receive a correct document id
		const activeId = state.documents.findIndex((doc) => doc.id === setActiveDocument.document_id);
		state.activeDocumentIndex = activeId;
	});

	editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseDocument, (displayConfirmationToCloseDocument) => {
		closeDocumentWithConfirmation(displayConfirmationToCloseDocument.document_id);
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
