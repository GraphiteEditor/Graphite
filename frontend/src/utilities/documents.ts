import { reactive, readonly } from "vue";

import { createDialog, dismissDialog } from "@/utilities/dialog";
import {
	ResponseType,
	registerResponseHandler,
	Response,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	SaveDocument,
} from "@/utilities/response-handler";
import { download, upload } from "@/utilities/files";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

class DocumentState {
	readonly displayName: string;

	constructor(readonly name: string, readonly isSaved: boolean) {
		this.displayName = `${name}${isSaved ? "" : "*"}`;
	}
}

const state = reactive({
	documents: [] as DocumentState[],
	activeDocumentIndex: 0,
});

export async function selectDocument(tabIndex: number) {
	(await wasm).select_document(tabIndex);
}

export async function closeDocumentWithConfirmation(tabIndex: number) {
	const targetDocument = state.documents[tabIndex];
	if (targetDocument.isSaved) {
		(await wasm).close_document(tabIndex);
		return;
	}

	// Show the document is being prompted to close
	await selectDocument(tabIndex);

	const tabLabel = targetDocument.name;

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

registerResponseHandler(ResponseType.UpdateOpenDocumentsList, (responseData: Response) => {
	const documentListData = responseData as UpdateOpenDocumentsList;
	state.documents = documentListData.open_documents.map(({ name, isSaved }) => new DocumentState(name, isSaved));
});

registerResponseHandler(ResponseType.SetActiveDocument, (responseData: Response) => {
	const documentData = responseData as SetActiveDocument;
	if (documentData) {
		state.activeDocumentIndex = documentData.document_index;
	}
});

registerResponseHandler(ResponseType.DisplayConfirmationToCloseDocument, (responseData: Response) => {
	const data = responseData as DisplayConfirmationToCloseDocument;
	closeDocumentWithConfirmation(data.document_index);
});

registerResponseHandler(ResponseType.DisplayConfirmationToCloseAllDocuments, (_: Response) => {
	closeAllDocumentsWithConfirmation();
});

registerResponseHandler(ResponseType.OpenDocumentBrowse, async (_: Response) => {
	const extension = (await wasm).file_save_suffix();
	const data = await upload(extension);
	(await wasm).open_document_file(data.filename, data.content);
});

registerResponseHandler(ResponseType.ExportDocument, (responseData: Response) => {
	const updateData = responseData as ExportDocument;
	if (updateData) download(updateData.name, updateData.document);
});

registerResponseHandler(ResponseType.SaveDocument, (responseData: Response) => {
	const saveData = responseData as SaveDocument;
	if (saveData) download(saveData.name, saveData.document);
});

(async () => (await wasm).get_open_documents_list())();
