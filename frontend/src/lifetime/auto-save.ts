import { AutoSaveDocument, RemoveAutoSaveDocument } from "@/dispatcher/js-messages";
import { DocumentsState } from "@/state/documents";
import { EditorState } from "@/state/wasm-loader";

const GRAPHITE_INDEXED_DB_NAME = "graphite-indexed-db";
const GRAPHITE_INDEXED_DB_VERSION = 1;
const GRAPHITE_AUTO_SAVE_STORE = "auto-save-documents";
const GRAPHITE_AUTO_SAVE_ORDER_KEY = "auto-save-documents-order";

const databaseConnection: Promise<IDBDatabase> = new Promise((resolve) => {
	const dbOpenRequest = indexedDB.open(GRAPHITE_INDEXED_DB_NAME, GRAPHITE_INDEXED_DB_VERSION);

	dbOpenRequest.onupgradeneeded = () => {
		const db = dbOpenRequest.result;
		if (!db.objectStoreNames.contains(GRAPHITE_AUTO_SAVE_STORE)) {
			db.createObjectStore(GRAPHITE_AUTO_SAVE_STORE, { keyPath: "details.id" });
		}
	};

	dbOpenRequest.onerror = () => {
		// eslint-disable-next-line no-console
		console.error("Graphite IndexedDb error:", dbOpenRequest.error);
	};

	dbOpenRequest.onsuccess = () => {
		resolve(dbOpenRequest.result);
	};
});

export function createAutoSaveManager(editor: EditorState, documents: DocumentsState) {
	const openAutoSavedDocuments = async (): Promise<void> => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readonly");
		const request = transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).getAll();

		return new Promise((resolve) => {
			request.onsuccess = () => {
				const previouslySavedDocuments: AutoSaveDocument[] = request.result;

				const documentOrder: string[] = JSON.parse(window.localStorage.getItem(GRAPHITE_AUTO_SAVE_ORDER_KEY) || "[]");
				const orderedSavedDocuments = documentOrder.map((id) => previouslySavedDocuments.find((autoSave) => autoSave.details.id === id)).filter((x) => x !== undefined) as AutoSaveDocument[];

				orderedSavedDocuments.forEach((doc: AutoSaveDocument) => {
					editor.instance.open_auto_saved_document(BigInt(doc.details.id), doc.details.name, doc.details.is_saved, doc.document);
				});
				resolve(undefined);
			};
		});
	};

	const storeDocumentOrder = () => {
		// Make sure to store as string since JSON does not play nice with BigInt
		const documentOrder = documents.state.documents.map((doc) => doc.id.toString());
		window.localStorage.setItem(GRAPHITE_AUTO_SAVE_ORDER_KEY, JSON.stringify(documentOrder));
	};

	editor.dispatcher.subscribeJsMessage(AutoSaveDocument, async (autoSaveDocument) => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).put(autoSaveDocument);
		storeDocumentOrder();
	});

	editor.dispatcher.subscribeJsMessage(RemoveAutoSaveDocument, async (removeAutoSaveDocument) => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).delete(removeAutoSaveDocument.document_id);
		storeDocumentOrder();
	});

	// On creation
	openAutoSavedDocuments();

	return {
		openAutoSavedDocuments,
	};
}
