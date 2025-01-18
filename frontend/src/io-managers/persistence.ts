import { createStore, del, get, set, update } from "idb-keyval";
import { get as getFromStore } from "svelte/store";

import { type PortfolioState } from "@graphite/state-providers/portfolio";
import { type Editor } from "@graphite/wasm-communication/editor";
import {
	TriggerIndexedDbWriteDocument,
	TriggerIndexedDbRemoveDocument,
	TriggerSavePreferences,
	TriggerLoadFirstAutoSaveDocument,
	TriggerLoadRestAutoSaveDocuments,
	TriggerLoadPreferences,
} from "@graphite/wasm-communication/messages";

const graphiteStore = createStore("graphite", "store");

export function createPersistenceManager(editor: Editor, portfolio: PortfolioState) {
	// DOCUMENTS

	async function storeDocumentOrder() {
		const documentOrder = getFromStore(portfolio).documents.map((doc) => String(doc.id));

		await set("documents_tab_order", documentOrder, graphiteStore);
	}

	async function storeCurrentDocumentIndex() {
		const documentIndex = getFromStore(portfolio).activeDocumentIndex;
		const documentId = getFromStore(portfolio).documents[documentIndex].id;

		await storeCurrentDocumentByID(String(documentId));
	}

	async function storeCurrentDocumentByID(documentId: string) {
		await set("current_document_id", String(documentId), graphiteStore);
	}

	async function storeDocument(autoSaveDocument: TriggerIndexedDbWriteDocument) {
		await update<Record<string, TriggerIndexedDbWriteDocument>>(
			"documents",
			(old) => {
				const documents = old || {};
				documents[autoSaveDocument.details.id] = autoSaveDocument;
				return documents;
			},
			graphiteStore,
		);

		await storeDocumentOrder();
		await storeCurrentDocumentByID(autoSaveDocument.details.id);
	}

	async function removeDocument(id: string) {
		await update<Record<string, TriggerIndexedDbWriteDocument>>(
			"documents",
			(old) => {
				const documents = old || {};
				delete documents[id];
				return documents;
			},
			graphiteStore,
		);

		const documentCount = getFromStore(portfolio).documents.length;
		if (documentCount > 0) {
			await storeCurrentDocumentIndex();
		} else {
			await del("current_document_id", graphiteStore);
		}

		await storeDocumentOrder();
	}

	async function loadFirstDocument() {
		const previouslySavedDocuments = await get<Record<string, TriggerIndexedDbWriteDocument>>("documents", graphiteStore);
		const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
		const currentDocumentId = await get<string>("current_document_id", graphiteStore);
		if (!previouslySavedDocuments || !documentOrder) return;

		const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

		if (currentDocumentId) {
			const doc = previouslySavedDocuments[currentDocumentId];
			editor.handle.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document, false);
			editor.handle.selectDocument(BigInt(currentDocumentId));
		} else {
			const len = orderedSavedDocuments.length;
			if (len > 0) {
				const doc = orderedSavedDocuments[len - 1];
				editor.handle.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document, false);
				editor.handle.selectDocument(BigInt(doc.details.id));
			}
		}
	}

	async function loadRestDocuments() {
		const previouslySavedDocuments = await get<Record<string, TriggerIndexedDbWriteDocument>>("documents", graphiteStore);
		const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
		const currentDocumentId = await get<string>("current_document_id", graphiteStore);
		if (!previouslySavedDocuments || !documentOrder) return;

		const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

		if (currentDocumentId) {
			const currentIndex = orderedSavedDocuments.findIndex((doc) => doc.details.id === currentDocumentId);
			for (let i = currentIndex - 1; i >= 0; i--) {
				const doc = orderedSavedDocuments[i];
				editor.handle.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document, true);
			}
			for (let i = currentIndex + 1; i < orderedSavedDocuments.length; i++) {
				const doc = orderedSavedDocuments[i];
				editor.handle.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document, false);
			}
			editor.handle.selectDocument(BigInt(currentDocumentId));
		} else {
			const len = orderedSavedDocuments.length;
			for (let i = len - 2; i >= 0; i--) {
				const doc = orderedSavedDocuments[i];
				editor.handle.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document, true);
			}
			if (len > 0) {
				const doc = orderedSavedDocuments[len - 1];
				editor.handle.selectDocument(BigInt(doc.details.id));
			}
		}
	}

	// PREFERENCES

	async function savePreferences(preferences: TriggerSavePreferences["preferences"]) {
		await set("preferences", preferences, graphiteStore);
	}

	async function loadPreferences() {
		const preferences = await get<Record<string, unknown>>("preferences", graphiteStore);
		if (!preferences) return;

		editor.handle.loadPreferences(JSON.stringify(preferences));
	}

	// FRONTEND MESSAGE SUBSCRIPTIONS

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerSavePreferences, async (preferences) => {
		await savePreferences(preferences.preferences);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadPreferences, async () => {
		await loadPreferences();
	});
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbWriteDocument, async (autoSaveDocument) => {
		await storeDocument(autoSaveDocument);
	});
	editor.subscriptions.subscribeJsMessage(TriggerIndexedDbRemoveDocument, async (removeAutoSaveDocument) => {
		await removeDocument(removeAutoSaveDocument.documentId);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadFirstAutoSaveDocument, async () => {
		await loadFirstDocument();
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadRestAutoSaveDocuments, async () => {
		await loadRestDocuments();
	});
}

export async function wipeDocuments() {
	await del("documents_tab_order", graphiteStore);
	await del("current_document_id", graphiteStore);
	await del("documents", graphiteStore);
}
