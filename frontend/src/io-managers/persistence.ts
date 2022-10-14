import { type PortfolioState } from "@/state-providers/portfolio";
import { stripIndents } from "@/utility-functions/strip-indents";
import { type Editor } from "@/wasm-communication/editor";
import { TriggerIndexedDbWriteDocument, TriggerIndexedDbRemoveDocument, TriggerSavePreferences, TriggerLoadAutoSaveDocuments, TriggerLoadPreferences } from "@/wasm-communication/messages";

const GRAPHITE_INDEXED_DB_VERSION = 2;
const GRAPHITE_INDEXED_DB_NAME = "graphite-indexed-db";

const GRAPHITE_AUTO_SAVE_STORE = { name: "auto-save-documents", keyPath: "details.id" };
const GRAPHITE_EDITOR_PREFERENCES_STORE = { name: "editor-preferences", keyPath: "key" };

const GRAPHITE_INDEXEDDB_STORES = [GRAPHITE_AUTO_SAVE_STORE, GRAPHITE_EDITOR_PREFERENCES_STORE];

const GRAPHITE_AUTO_SAVE_ORDER_KEY = "auto-save-documents-order";

export function createPersistenceManager(editor: Editor, portfolio: PortfolioState): () => void {
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

					db.createObjectStore(store.name, { keyPath: store.keyPath });
				});
			};

			// Handle some other error by presenting it to the user
			dbOpenRequest.onerror = (): void => {
				const errorText = stripIndents`
				Documents won't be saved across reloads and later visits.
				This may be caused by Firefox's private browsing mode.
				
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

	function storeDocumentOrder(): void {
		// Make sure to store as string since JSON does not play nice with BigInt
		const documentOrder = portfolio.state.documents.map((doc) => doc.id.toString());
		window.localStorage.setItem(GRAPHITE_AUTO_SAVE_ORDER_KEY, JSON.stringify(documentOrder));
	}

	async function removeDocument(id: string, db: IDBDatabase): Promise<void> {
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE.name, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE.name).delete(id);
		storeDocumentOrder();
	}

	async function loadAutoSaveDocuments(db: IDBDatabase): Promise<void> {
		let promiseResolve: (value: void | PromiseLike<void>) => void;
		const promise = new Promise<void>((resolve): void => {
			promiseResolve = resolve;
		});

		// Open auto-save documents
		const transaction = db.transaction(GRAPHITE_AUTO_SAVE_STORE.name, "readonly");
		const request = transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE.name).getAll();

		request.onsuccess = (): void => {
			const previouslySavedDocuments: TriggerIndexedDbWriteDocument[] = request.result;

			const documentOrder: string[] = JSON.parse(window.localStorage.getItem(GRAPHITE_AUTO_SAVE_ORDER_KEY) || "[]");
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

			promiseResolve();
		};

		await promise;
	}

	async function loadPreferences(db: IDBDatabase): Promise<void> {
		let promiseResolve: (value: void | PromiseLike<void>) => void;
		const promise = new Promise<void>((resolve): void => {
			promiseResolve = resolve;
		});

		// Open auto-save documents
		const transaction = db.transaction(GRAPHITE_EDITOR_PREFERENCES_STORE.name, "readonly");
		const request = transaction.objectStore(GRAPHITE_EDITOR_PREFERENCES_STORE.name).getAll();

		request.onsuccess = (): void => {
			const preferenceEntries: { key: string; value: unknown }[] = request.result;

			const preferences: Record<string, unknown> = {};
			preferenceEntries.forEach(({ key, value }) => {
				preferences[key] = value;
			});

			editor.instance.loadPreferences(JSON.stringify(preferences));

			promiseResolve();
		};

		await promise;
	}

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbWriteDocument, async (autoSaveDocument) => {
		const transaction = (await databaseConnection).transaction(GRAPHITE_AUTO_SAVE_STORE.name, "readwrite");
		transaction.objectStore(GRAPHITE_AUTO_SAVE_STORE.name).put(autoSaveDocument);

		storeDocumentOrder();
	});
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbRemoveDocument, async (removeAutoSaveDocument) => {
		await removeDocument(removeAutoSaveDocument.documentId, await databaseConnection);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadAutoSaveDocuments, async () => {
		await loadAutoSaveDocuments(await databaseConnection);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSavePreferences, async (preferences) => {
		Object.entries(preferences.preferences).forEach(async ([key, value]) => {
			const storedObject = { key, value };

			const transaction = (await databaseConnection).transaction(GRAPHITE_EDITOR_PREFERENCES_STORE.name, "readwrite");
			transaction.objectStore(GRAPHITE_EDITOR_PREFERENCES_STORE.name).put(storedObject);
		});
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
