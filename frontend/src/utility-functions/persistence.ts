import * as idb from "idb-keyval";
import { get } from "svelte/store";
import type { PortfolioStore } from "/src/stores/portfolio";
import type { MessageBody } from "/src/subscriptions-router";
import type { EditorHandle } from "/wasm/pkg/graphite_wasm";

export async function storeCurrentDocumentId(documentId: string) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.set("current_document_id", String(documentId), indexedDbStorage);
}

export async function storeDocument(autoSaveDocument: MessageBody<"TriggerPersistenceWriteDocument">, portfolio: PortfolioStore) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.update<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>(
		"documents",
		(old) => {
			const documents = old || {};
			documents[String(autoSaveDocument.documentId)] = autoSaveDocument;
			return documents;
		},
		indexedDbStorage,
	);

	const documentOrder = get(portfolio).documents.map((doc) => String(doc.id));
	await idb.set("documents_tab_order", documentOrder, indexedDbStorage);
	await storeCurrentDocumentId(String(autoSaveDocument.documentId));
}

export async function removeDocument(id: string, portfolio: PortfolioStore) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.update<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>(
		"documents",
		(old) => {
			const documents = old || {};
			delete documents[id];
			return documents;
		},
		indexedDbStorage,
	);

	await idb.update<string[]>(
		"documents_tab_order",
		(old) => {
			const order = old || [];
			return order.filter((docId) => docId !== id);
		},
		indexedDbStorage,
	);

	const documentCount = get(portfolio).documents.length;
	if (documentCount > 0) {
		const documentIndex = get(portfolio).activeDocumentIndex;
		const documentId = String(get(portfolio).documents[documentIndex].id);

		const tabOrder = (await idb.get<string[]>("documents_tab_order", indexedDbStorage)) || [];
		if (tabOrder.includes(documentId)) {
			await storeCurrentDocumentId(documentId);
		}
	} else {
		await idb.del("current_document_id", indexedDbStorage);
	}
}

export async function loadFirstDocument(editor: EditorHandle) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	const previouslySavedDocuments = await idb.get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", indexedDbStorage);

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if the browser is storing the old format as strings
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	const documentOrder = await idb.get<string[]>("documents_tab_order", indexedDbStorage);
	const currentDocumentIdString = await idb.get<string>("current_document_id", indexedDbStorage);
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

export async function loadRestDocuments(editor: EditorHandle) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	const previouslySavedDocuments = await idb.get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", indexedDbStorage);

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if needed
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	const documentOrder = await idb.get<string[]>("documents_tab_order", indexedDbStorage);
	const currentDocumentIdString = await idb.get<string>("current_document_id", indexedDbStorage);
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
	const indexedDbStorage = idb.createStore("graphite", "store");

	const previouslySavedDocuments = await idb.get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", indexedDbStorage);

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
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.set("preferences", preferences, indexedDbStorage);
}

export async function loadEditorPreferences(editor: EditorHandle) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	const preferences = await idb.get<Record<string, unknown>>("preferences", indexedDbStorage);
	editor.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
}

export async function wipeDocuments() {
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.del("documents_tab_order", indexedDbStorage);
	await idb.del("current_document_id", indexedDbStorage);
	await idb.del("documents", indexedDbStorage);
}
