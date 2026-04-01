import { get } from "svelte/store";
import type { PortfolioStore } from "/src/stores/portfolio";
import type { MessageBody } from "/src/subscriptions-router";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

const PERSISTENCE_DB = "graphite";
const PERSISTENCE_STORE = "store";

export async function storeCurrentDocumentId(documentId: string) {
	await databaseSet("current_document_id", String(documentId));
}

export async function storeDocument(autoSaveDocument: MessageBody<"TriggerPersistenceWriteDocument">, portfolio: PortfolioStore) {
	await databaseUpdate<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", (old) => {
		const documents = old || {};
		documents[String(autoSaveDocument.documentId)] = autoSaveDocument;
		return documents;
	});

	const documentOrder = get(portfolio).documents.map((doc) => String(doc.id));
	await databaseSet("documents_tab_order", documentOrder);
	await storeCurrentDocumentId(String(autoSaveDocument.documentId));
}

export async function removeDocument(id: string, portfolio: PortfolioStore) {
	await databaseUpdate<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", (old) => {
		const documents = old || {};
		delete documents[id];
		return documents;
	});

	await databaseUpdate<string[]>("documents_tab_order", (old) => {
		const order = old || [];
		return order.filter((docId) => docId !== id);
	});

	const documentCount = get(portfolio).documents.length;
	if (documentCount > 0) {
		const documentIndex = get(portfolio).activeDocumentIndex;
		const documentId = String(get(portfolio).documents[documentIndex].id);

		const tabOrder = (await databaseGet<string[]>("documents_tab_order")) || [];
		if (tabOrder.includes(documentId)) {
			await storeCurrentDocumentId(documentId);
		}
	} else {
		await databaseDelete("current_document_id");
	}
}

export async function loadFirstDocument(editor: EditorWrapper) {
	const previouslySavedDocuments = await databaseGet<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents");

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if the browser is storing the old format as strings
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	const documentOrder = await databaseGet<string[]>("documents_tab_order");
	const currentDocumentIdString = await databaseGet<string>("current_document_id");
	const currentDocumentId = currentDocumentIdString ? BigInt(currentDocumentIdString) : undefined;
	if (!previouslySavedDocuments || !documentOrder) return;

	const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

	if (currentDocumentId !== undefined && String(currentDocumentId) in previouslySavedDocuments) {
		const doc = previouslySavedDocuments[String(currentDocumentId)];
		editor.openAutoSavedDocument(doc.documentId, doc.details.name, doc.details.isSaved, doc.document, false);
		editor.selectDocument(currentDocumentId);
	} else {
		const len = orderedSavedDocuments.length;
		if (len > 0) {
			const doc = orderedSavedDocuments[len - 1];
			editor.openAutoSavedDocument(doc.documentId, doc.details.name, doc.details.isSaved, doc.document, false);
			editor.selectDocument(doc.documentId);
		}
	}
}

export async function loadRestDocuments(editor: EditorWrapper) {
	const previouslySavedDocuments = await databaseGet<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents");

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if needed
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	const documentOrder = await databaseGet<string[]>("documents_tab_order");
	const currentDocumentIdString = await databaseGet<string>("current_document_id");
	const currentDocumentId = currentDocumentIdString ? BigInt(currentDocumentIdString) : undefined;
	if (!previouslySavedDocuments || !documentOrder) return;

	const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

	const currentIndex = currentDocumentId !== undefined ? orderedSavedDocuments.findIndex((doc) => doc.documentId === currentDocumentId) : -1;

	// Open documents in order around the current document, placing earlier ones before it and later ones after
	if (currentIndex !== -1 && currentDocumentId !== undefined) {
		for (let i = currentIndex - 1; i >= 0; i--) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.openAutoSavedDocument(documentId, name, isSaved, document, true);
		}
		for (let i = currentIndex + 1; i < orderedSavedDocuments.length; i++) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.openAutoSavedDocument(documentId, name, isSaved, document, false);
		}

		editor.selectDocument(currentDocumentId);
	}
	// No valid current document: open all remaining documents and select the last one
	else {
		const length = orderedSavedDocuments.length;

		for (let i = length - 2; i >= 0; i--) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.openAutoSavedDocument(documentId, name, isSaved, document, true);
		}

		if (length > 0) editor.selectDocument(orderedSavedDocuments[length - 1].documentId);
	}
}

export async function saveActiveDocument(documentId: bigint) {
	const previouslySavedDocuments = await databaseGet<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents");

	const documentIdString = String(documentId);

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if needed
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	if (!previouslySavedDocuments) return;
	if (documentIdString in previouslySavedDocuments) {
		await storeCurrentDocumentId(documentIdString);
	}
}

export async function saveEditorPreferences(preferences: unknown) {
	await databaseSet("preferences", preferences);
}

export async function loadEditorPreferences(editor: EditorWrapper) {
	const preferences = await databaseGet<Record<string, unknown>>("preferences");
	editor.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
}

export async function wipeDocuments() {
	await databaseDelete("documents_tab_order");
	await databaseDelete("current_document_id");
	await databaseDelete("documents");
}

function databaseOpen(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const request = indexedDB.open(PERSISTENCE_DB, 1);
		request.onupgradeneeded = () => {
			if (!request.result.objectStoreNames.contains(PERSISTENCE_STORE)) {
				request.result.createObjectStore(PERSISTENCE_STORE);
			}
		};
		request.onsuccess = () => resolve(request.result);
		request.onerror = () => reject(request.error);
	});
}

async function databaseGet<T>(key: string): Promise<T | undefined> {
	const db = await databaseOpen();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction(PERSISTENCE_STORE, "readonly");
		const request = transaction.objectStore(PERSISTENCE_STORE).get(key);
		request.onsuccess = () => {
			const result: T | undefined = request.result;
			resolve(result);
		};
		request.onerror = () => reject(request.error);
	});
}

async function databaseSet(key: string, value: unknown): Promise<void> {
	const db = await databaseOpen();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction(PERSISTENCE_STORE, "readwrite");
		transaction.objectStore(PERSISTENCE_STORE).put(value, key);
		transaction.oncomplete = () => resolve();
		transaction.onerror = () => reject(transaction.error);
	});
}

async function databaseDelete(key: string): Promise<void> {
	const db = await databaseOpen();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction(PERSISTENCE_STORE, "readwrite");
		transaction.objectStore(PERSISTENCE_STORE).delete(key);
		transaction.oncomplete = () => resolve();
		transaction.onerror = () => reject(transaction.error);
	});
}

async function databaseUpdate<T>(key: string, updater: (existing: T | undefined) => T): Promise<void> {
	const db = await databaseOpen();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction(PERSISTENCE_STORE, "readwrite");
		const store = transaction.objectStore(PERSISTENCE_STORE);
		const getRequest = store.get(key);
		getRequest.onsuccess = () => {
			const existing: T | undefined = getRequest.result;
			store.put(updater(existing), key);
		};
		transaction.oncomplete = () => resolve();
		transaction.onerror = () => reject(transaction.error);
	});
}
