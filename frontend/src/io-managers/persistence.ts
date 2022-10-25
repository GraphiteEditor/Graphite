import { type PortfolioState } from "@/state-providers/portfolio";
import { stripIndents } from "@/utility-functions/strip-indents";
import { type Editor } from "@/wasm-communication/editor";
import { TriggerIndexedDbWriteDocument, TriggerIndexedDbRemoveDocument, TriggerSavePreferences, TriggerLoadAutoSaveDocuments, TriggerLoadPreferences } from "@/wasm-communication/messages";

const GRAPHITE_INDEXED_DB_NAME = "graphite";
// Increment this whenever the format changes at all
const GRAPHITE_INDEXED_DB_VERSION = 1;

const GRAPHITE_EDITOR_PREFERENCES_STORE = { name: "editor-preferences", keyPath: "key" };
const GRAPHITE_AUTO_SAVE_DOCUMENT_LIST_STORE = { name: "auto-save-document-list", keyPath: "key" };
const GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE = { name: "auto-save-documents", keyPath: "details.id" };

const GRAPHITE_INDEXEDDB_STORES = [GRAPHITE_EDITOR_PREFERENCES_STORE, GRAPHITE_AUTO_SAVE_DOCUMENT_LIST_STORE, GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE];

export function createPersistenceManager(editor: Editor, portfolio: PortfolioState): () => void {
	// INITIALIZE

	async function initialize(): Promise<IDBDatabase> {
		// Open the IndexedDB database connection and save it to this variable, which is a promise that resolves once the connection is open
		return new Promise<IDBDatabase>((resolve) => {
			const dbOpenRequest = indexedDB.open(GRAPHITE_INDEXED_DB_NAME, GRAPHITE_INDEXED_DB_VERSION);

			// Handle a version mismatch if `GRAPHITE_INDEXED_DB_VERSION` is now higher than what was saved in the database
			dbOpenRequest.onupgradeneeded = (): void => {
				const db = dbOpenRequest.result;

				// Wipe out all stores when a request is made to upgrade the database version to a newer one
				GRAPHITE_INDEXEDDB_STORES.forEach((store) => {
					if (db.objectStoreNames.contains(store.name)) db.deleteObjectStore(store.name);

					const options = store.keyPath ? [{ keyPath: store.keyPath }] : [];

					db.createObjectStore(store.name, ...options);
				});
			};

			// Handle some other error by presenting it to the user
			dbOpenRequest.onerror = (): void => {
				const errorText = stripIndents`
				Documents and preferences won't be saved across reloads and later visits.
				This may be caused by the browser's private browsing mode.
				
				Error on opening IndexDB:
				${dbOpenRequest.error}
				`;
				editor.instance.errorDialog("Document auto-save doesn't work in this browser", errorText);
			};

			// Resolve the promise on a successful opening of the database connection
			dbOpenRequest.onsuccess = (): void => {
				resolve(dbOpenRequest.result);
			};
		});
	}

	function storeDocumentOrder(db: IDBDatabase): void {
		const documentOrder = portfolio.state.documents.map((doc) => String(doc.id));

		const storedObject = { key: "key", value: documentOrder };

		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_DOCUMENT_LIST_STORE.name, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_DOCUMENT_LIST_STORE.name).put(storedObject);
	}

	async function loadDocumentOrder(db: IDBDatabase): Promise<string[]> {
		// Open auto-save documents
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_DOCUMENT_LIST_STORE.name, "readonly");
		const request = transaction.objectStore(GRAPHITE_AUTO_SAVE_DOCUMENT_LIST_STORE.name).getAll();

		// Await the database request data
		await new Promise<void>((resolve): void => {
			request.onsuccess = (): void => resolve();
		});

		const results: { key: string; value: string[] }[] = request.result;
		const documentOrder = results[0]?.value || [];

		return documentOrder;
	}

	// AUTO SAVE DOCUMENTS

	async function storeDocument(db: IDBDatabase, autoSaveDocument: TriggerIndexedDbWriteDocument): Promise<void> {
		const transaction = (await databaseConnection).transaction(GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE.name, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE.name).put(autoSaveDocument);

		storeDocumentOrder(await databaseConnection);
	}

	async function removeDocument(id: string, db: IDBDatabase): Promise<void> {
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE.name, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE.name).delete(id);
		storeDocumentOrder(db);
	}

	async function loadDocuments(db: IDBDatabase): Promise<void> {
		// Open auto-save documents
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE.name, "readonly");
		const request = transaction.objectStore(GRAPHITE_AUTO_SAVE_DOCUMENTS_STORE.name).getAll();

		// Await the database request data
		await new Promise<void>((resolve): void => {
			request.onsuccess = (): void => resolve();
		});

		const previouslySavedDocuments: TriggerIndexedDbWriteDocument[] = request.result;

		const documentOrder = await loadDocumentOrder(db);
		const orderedSavedDocuments = documentOrder
			.map((id) => previouslySavedDocuments.find((autoSave) => autoSave.details.id === id))
			.filter((x) => x !== undefined) as TriggerIndexedDbWriteDocument[];

		const currentDocumentVersion = editor.instance.graphiteDocumentVersion();
		orderedSavedDocuments.forEach(async (doc: TriggerIndexedDbWriteDocument) => {
			if (doc.version === currentDocumentVersion) {
				editor.instance.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document);
			} else {
				await removeDocument(doc.details.id, db);
			}
		});
	}

	// PREFERENCES

	async function savePreferences(preferences: TriggerSavePreferences["preferences"], db: IDBDatabase): Promise<void> {
		Object.entries(preferences).forEach(async ([key, value]) => {
			const storedObject = { key, value };

			const transaction = db.transaction(GRAPHITE_EDITOR_PREFERENCES_STORE.name, "readwrite");
			transaction.objectStore(GRAPHITE_EDITOR_PREFERENCES_STORE.name).put(storedObject);
		});
	}

	async function loadPreferences(db: IDBDatabase): Promise<void> {
		// Open auto-save documents
		const transaction = db.transaction(GRAPHITE_EDITOR_PREFERENCES_STORE.name, "readonly");
		const request = transaction.objectStore(GRAPHITE_EDITOR_PREFERENCES_STORE.name).getAll();

		// Await the database request data
		await new Promise<void>((resolve): void => {
			request.onsuccess = (): void => resolve();
		});

		const preferenceEntries: { key: string; value: unknown }[] = request.result;

		const preferences: Record<string, unknown> = {};
		preferenceEntries.forEach(({ key, value }) => {
			preferences[key] = value;
		});

		editor.instance.loadPreferences(JSON.stringify(preferences));
	}

	// FRONTEND MESSAGE SUBSCRIPTIONS

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbWriteDocument, async (autoSaveDocument) => {
		await storeDocument(await databaseConnection, autoSaveDocument);
	});
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbRemoveDocument, async (removeAutoSaveDocument) => {
		await removeDocument(removeAutoSaveDocument.documentId, await databaseConnection);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadAutoSaveDocuments, async () => {
		await loadDocuments(await databaseConnection);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSavePreferences, async (preferences) => {
		await savePreferences(preferences.preferences, await databaseConnection);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadPreferences, async () => {
		await loadPreferences(await databaseConnection);
	});

	const databaseConnection = initialize();

	// Destructor
	return () => {
		databaseConnection.then((connection) => connection.close());
	};
}
