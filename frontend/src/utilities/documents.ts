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
	DisplayFolderTreeStructure,
} from "@/utilities/response-handler";
import { download, upload } from "@/utilities/files";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

interface OpenDocumentInformation {
	name: string;
	unsaved: boolean;
	layerTreeHeight: number;
}
const state = reactive({
	title: "",
	documents: [] as OpenDocumentInformation[],
	activeDocumentIndex: 0,
});

export async function selectDocument(tabIndex: number) {
	(await wasm).select_document(tabIndex);
}

export function markUnsavedActiveDocument() {
	state.documents[state.activeDocumentIndex].unsaved = true;
}

export async function closeDocumentWithConfirmation(tabIndex: number) {
	selectDocument(tabIndex);

	const tabLabel = state.documents[tabIndex].name;

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
	if (documentListData) {
		state.documents = documentListData.open_documents.map((name) => {
			// Can't guarantee an exact match since it is going off of names
			// This is ok though since it is only used to mark a document as unsaved
			const probableMatch = state.documents.find((doc) => doc.name === name);
			if (probableMatch) {
				return { name, unsaved: probableMatch.unsaved, layerTreeHeight: probableMatch.layerTreeHeight };
			}
			return { name, unsaved: false, layerTreeHeight: 0 };
		});
		state.title = state.documents[state.activeDocumentIndex].name;
	}
});

registerResponseHandler(ResponseType.SetActiveDocument, (responseData: Response) => {
	const documentData = responseData as SetActiveDocument;
	if (documentData) {
		state.activeDocumentIndex = documentData.document_index;
		state.title = state.documents[state.activeDocumentIndex].name;
	}
});

// Used to figure out if the document has been modified since the last save
// Pitfalls: This will not detect if a new effect has been applied but just if the hierarchy has been modified
registerResponseHandler(ResponseType.DisplayFolderTreeStructure, (responseData: Response) => {
	const expandData = responseData as DisplayFolderTreeStructure;
	const activeDocument = state.documents[state.activeDocumentIndex];
	if (activeDocument.unsaved) {
		// No need to calculate if it is already unsaved
		return;
	}

	function countLayers(layers: DisplayFolderTreeStructure): number {
		const childrenCount = layers.children.map((child) => countLayers(child)).reduce((acc, curr) => acc + curr, 0);
		return 1 + childrenCount;
	}

	const layerTreeHeight = countLayers(expandData);

	// Clicking increases the tree height by 1 giving false positives
	// Having the first change not trigger the unsaved notification will be unnoticeable by most users
	if (layerTreeHeight > activeDocument.layerTreeHeight + 1 || layerTreeHeight < activeDocument.layerTreeHeight) {
		activeDocument.layerTreeHeight = layerTreeHeight;
		activeDocument.unsaved = true;
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
	state.documents[state.activeDocumentIndex].unsaved = false;
	if (saveData) download(saveData.name, saveData.document);
});

(async () => (await wasm).get_open_documents_list())();
