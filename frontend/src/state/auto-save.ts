import { AutoSaveDocument, RemoveAutoSaveDocument } from "@/dispatcher/js-messages";
import { EditorState } from "@/state/wasm-loader";

/* eslint-disable no-console */
const GRAPHITE_INDEXED_DB_NAME = "graphite-indexed-db";
const GRAPHITE_INDEXED_DB_VERSION = 1;
const GRAPHITE_AUTO_SAVE_STORE = "auto-save-documents";

const databaseConnection: Promise<IDBDatabase> = new Promise((resolve) => {
	const dbOpenRequest = indexedDB.open(GRAPHITE_INDEXED_DB_NAME, GRAPHITE_INDEXED_DB_VERSION);

	dbOpenRequest.onupgradeneeded = () => {
		const db = dbOpenRequest.result;
		if (!db.objectStoreNames.contains(GRAPHITE_AUTO_SAVE_STORE)) {
			db.createObjectStore(GRAPHITE_AUTO_SAVE_STORE, { keyPath: "details.id" });
		}
	};

	dbOpenRequest.onerror = () => {
		console.error("Error", dbOpenRequest.error);
	};

	dbOpenRequest.onsuccess = () => {
		resolve(dbOpenRequest.result);
	};
});

export type AutoSaveState = ReturnType<typeof createAutoSaveState>;

export function createAutoSaveState(editor: EditorState) {
	const openAutoSavedDocuments = async (): Promise<void> => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readonly");
		const request = transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).getAll();
		return new Promise((resolve) => {
			request.onsuccess = () => {
				const previouslySavedDocuments = request.result;
				previouslySavedDocuments.forEach((doc: AutoSaveDocument) => {
					editor.instance.open_auto_saved_document(BigInt(doc.details.id), doc.details.name, doc.details.is_saved, doc.document);
				});
				resolve(undefined);
			};
		});
	};

	editor.dispatcher.subscribeJsMessage(AutoSaveDocument, async (autoSaveDocument) => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).put(autoSaveDocument);
	});

	editor.dispatcher.subscribeJsMessage(RemoveAutoSaveDocument, async (removeAutoSaveDocument) => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).delete(removeAutoSaveDocument.document_id);
	});

	return {
		openAutoSavedDocuments,
	};
}
