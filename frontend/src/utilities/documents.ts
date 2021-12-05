import { reactive, readonly } from "vue";

import { createDialog, dismissDialog } from "@/utilities/dialog";
import { registerJsMessageHandler } from "@/utilities/js-message-dispatcher";
import {
	DisplayConfirmationToCloseAllDocuments,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
} from "@/utilities/js-messages";
import { download, upload } from "@/utilities/files";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

const state = reactive({
	documents: [] as string[],
	activeDocumentIndex: 0,
	get activeDocument() {
		return this.documents[this.activeDocumentIndex];
	},
});

export async function selectDocument(tabIndex: number) {
	(await wasm).select_document(tabIndex);
}

export async function closeDocumentWithConfirmation(tabIndex: number) {
	selectDocument(tabIndex);

	const tabLabel = state.documents[tabIndex];

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
				(await wasm).close_document(tabIndex);
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

registerJsMessageHandler(UpdateOpenDocumentsList, (documentListData) => {
	state.documents = documentListData.open_documents.map(({ name, isSaved }) => `${name}${isSaved ? "" : "*"}`);
});

registerJsMessageHandler(SetActiveDocument, (documentData) => {
	if (documentData) {
		state.activeDocumentIndex = documentData.document_index;
	}
});

registerJsMessageHandler(DisplayConfirmationToCloseDocument, (data) => {
	closeDocumentWithConfirmation(data.document_index);
});

registerJsMessageHandler(DisplayConfirmationToCloseAllDocuments, (_) => {
	closeAllDocumentsWithConfirmation();
});

registerJsMessageHandler(OpenDocumentBrowse, async (_) => {
	const extension = (await wasm).file_save_suffix();
	const data = await upload(extension);
	(await wasm).open_document_file(data.filename, data.content);
});

registerJsMessageHandler(ExportDocument, (updateData) => {
	if (updateData) download(updateData.name, updateData.document);
});

registerJsMessageHandler(SaveDocument, (saveData) => {
	if (saveData) download(saveData.name, saveData.document);
});

(async () => (await wasm).get_open_documents_list())();
