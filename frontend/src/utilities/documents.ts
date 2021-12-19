import { reactive, readonly } from "vue";

import { createDialog, dismissDialog } from "@/utilities/dialog";
import { subscribeJsMessage } from "@/utilities/js-message-dispatcher";
import {
	DisplayConfirmationToCloseAllDocuments,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
	FrontendDocumentState,
} from "@/utilities/js-messages";
import { download, upload } from "@/utilities/files";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

const state = reactive({
	documents: [] as FrontendDocumentState[],
	activeDocumentIndex: 0,
});

export async function selectDocument(documentId: BigInt) {
	(await wasm).select_document(documentId);
}

export async function closeDocumentWithConfirmation(documentId: BigInt) {
	// Assume we receive a correct document_id
	const targetDocument = state.documents.find((doc) => doc.id === documentId) as FrontendDocumentState;
	if (targetDocument.isSaved) {
		(await wasm).close_document(targetDocument.id);
		return;
	}

	// Show the document is being prompted to close
	await selectDocument(targetDocument.id);

	const tabLabel = targetDocument.displayName;

	createDialog("File", "Save changes before closing?", tabLabel, [
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).save_document();
				dismissDialog();
			},
			props: { label: "Save", emphasized: true, minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).close_document(targetDocument.id);
				dismissDialog();
			},
			props: { label: "Discard", minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				dismissDialog();
			},
			props: { label: "Cancel", minWidth: 96 },
		},
	]);
}

export async function closeAllDocumentsWithConfirmation() {
	createDialog("Copy", "Close all documents?", "Unsaved work will be lost!", [
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).close_all_documents();
				dismissDialog();
			},
			props: { label: "Discard All", minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				dismissDialog();
			},
			props: { label: "Cancel", minWidth: 96 },
		},
	]);
}

export default readonly(state);

subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
	state.documents = updateOpenDocumentList.open_documents;
});

subscribeJsMessage(SetActiveDocument, (setActiveDocument) => {
	// Assume we receive a correct document_id
	const activeId = state.documents.findIndex((doc) => doc.id === setActiveDocument.document_id);
	state.activeDocumentIndex = activeId;
});

subscribeJsMessage(DisplayConfirmationToCloseDocument, (displayConfirmationToCloseDocument) => {
	closeDocumentWithConfirmation(displayConfirmationToCloseDocument.document_id);
});

subscribeJsMessage(DisplayConfirmationToCloseAllDocuments, () => {
	closeAllDocumentsWithConfirmation();
});

subscribeJsMessage(OpenDocumentBrowse, async () => {
	const extension = (await wasm).file_save_suffix();
	const data = await upload(extension);
	(await wasm).open_document_file(data.filename, data.content);
});

subscribeJsMessage(ExportDocument, (exportDocument) => {
	download(exportDocument.name, exportDocument.document);
});

subscribeJsMessage(SaveDocument, (saveDocument) => {
	download(saveDocument.name, saveDocument.document);
});

(async () => (await wasm).get_open_documents_list())();
