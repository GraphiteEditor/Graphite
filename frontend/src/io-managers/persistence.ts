import { PortfolioState } from "@/state-providers/portfolio";
import { stripIndents } from "@/utility-functions/strip-indents";
import { Editor } from "@/wasm-communication/editor";
import { TriggerIndexedDbWriteDocument, TriggerIndexedDbRemoveDocument } from "@/wasm-communication/messages";

const GRAPHITE_INDEXED_DB_VERSION = 2;
const GRAPHITE_INDEXED_DB_NAME = "graphite-indexed-db";
const GRAPHITE_AUTO_SAVE_STORE = "auto-save-documents";
const GRAPHITE_AUTO_SAVE_ORDER_KEY = "auto-save-documents-order";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export async function createPersistenceManager(editor: Editor, portfolio: PortfolioState): Promise<() => void> {
	function storeDocumentOrder(): void {
		// Make sure to store as string since JSON does not play nice with BigInt
		const documentOrder = portfolio.state.documents.map((doc) => doc.id.toString());
		window.localStorage.setItem(GRAPHITE_AUTO_SAVE_ORDER_KEY, JSON.stringify(documentOrder));
	}

	async function removeDocument(id: string): Promise<void> {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).delete(id);
		storeDocumentOrder();
	}

	async function closeDatabaseConnection(): Promise<void> {
		const db = await databaseConnection;
		db.close();
	}

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbWriteDocument, async (autoSaveDocument) => {
		const db = await databaseConnection;
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).put(autoSaveDocument);
		storeDocumentOrder();
	});
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbRemoveDocument, async (removeAutoSaveDocument) => {
		removeDocument(removeAutoSaveDocument.document_id);
	});

	// Open the IndexedDB database connection and save it to this variable, which is a promise that resolves once the connection is open
	const databaseConnection: Promise<IDBDatabase> = new Promise((resolve) => {
		const dbOpenRequest = indexedDB.open(GRAPHITE_INDEXED_DB_NAME, GRAPHITE_INDEXED_DB_VERSION);

		dbOpenRequest.onupgradeneeded = (): void => {
			const db = dbOpenRequest.result;
			// Wipes out all auto-save data on upgrade
			if (db.objectStoreNames.contains(GRAPHITE_AUTO_SAVE_STORE)) {
				db.deleteObjectStore(GRAPHITE_AUTO_SAVE_STORE);
			}

			db.createObjectStore(GRAPHITE_AUTO_SAVE_STORE, { keyPath: "details.id" });
		};

		dbOpenRequest.onerror = (): void => {
			const errorText = stripIndents`
				Documents won't be saved across reloads and later visits.
				This may be caused by Firefox's private browsing mode.
				
				Error on opening IndexDB:
				${dbOpenRequest.error}
				`;
			editor.instance.error_dialog("Document auto-save doesn't work in this browser", errorText);
		};

		dbOpenRequest.onsuccess = (): void => {
			resolve(dbOpenRequest.result);
		};
	});

	databaseConnection.then(async (db) => {
		// Open auto-save documents
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE, "readonly");
		const request = transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE).getAll();
		await new Promise((resolve): void => {
			request.onsuccess = (): void => {
				const previouslySavedDocuments: TriggerIndexedDbWriteDocument[] = request.result;

				const documentOrder: string[] = JSON.parse(window.localStorage.getItem(GRAPHITE_AUTO_SAVE_ORDER_KEY) || "[]");
				const orderedSavedDocuments = documentOrder
					.map((id) => previouslySavedDocuments.find((autoSave) => autoSave.details.id === id))
					.filter((x) => x !== undefined) as TriggerIndexedDbWriteDocument[];

				const currentDocumentVersion = editor.instance.graphite_document_version();
				orderedSavedDocuments.forEach((doc: TriggerIndexedDbWriteDocument) => {
					if (doc.version === currentDocumentVersion) {
						editor.instance.open_auto_saved_document(BigInt(doc.details.id), doc.details.name, doc.details.is_saved, doc.document);
					} else {
						removeDocument(doc.details.id);
					}
				});
				resolve(undefined);
			};
		});
	});

	return closeDatabaseConnection;
}
