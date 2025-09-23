import { createStore, del, get, set, update } from "idb-keyval";
import { get as getFromStore } from "svelte/store";

import { type Editor } from "@graphite/editor";
import {
	TriggerPersistenceWriteDocument,
	TriggerPersistenceRemoveDocument,
	TriggerSavePreferences,
	TriggerLoadPreferences,
	TriggerLoadFirstAutoSaveDocument,
	TriggerLoadRestAutoSaveDocuments,
	TriggerSaveActiveDocument,
} from "@graphite/messages";
import { type PortfolioState } from "@graphite/state-providers/portfolio";

const graphiteStore = createStore("graphite", "store");

export function createPersistenceManager(editor: Editor, portfolio: PortfolioState) {
	// DOCUMENTS

	async function storeDocumentOrder() {
		const documentOrder = getFromStore(portfolio).documents.map((doc) => String(doc.id));
		await set("documents_tab_order", documentOrder, graphiteStore);
	}

	async function storeCurrentDocumentId(documentId: string) {
		await set("current_document_id", String(documentId), graphiteStore);
	}

	async function storeDocument(autoSaveDocument: TriggerPersistenceWriteDocument) {
		await update<Record<string, TriggerPersistenceWriteDocument>>(
			"documents",
			(old) => {
				const documents = old || {};
				documents[autoSaveDocument.documentId] = autoSaveDocument;
				return documents;
			},
			graphiteStore,
		);

		await storeDocumentOrder();
		await storeCurrentDocumentId(autoSaveDocument.documentId);
	}

	async function removeDocument(id: string) {
		await update<Record<string, TriggerPersistenceWriteDocument>>(
			"documents",
			(old) => {
				const documents = old || {};
				delete documents[id];
				return documents;
			},
			graphiteStore,
		);

		await update<string[]>(
			"documents_tab_order",
			(old) => {
				const order = old || [];
				return order.filter((docId) => docId !== id);
			},
			graphiteStore,
		);

		const documentCount = getFromStore(portfolio).documents.length;
		if (documentCount > 0) {
			const documentIndex = getFromStore(portfolio).activeDocumentIndex;
			const documentId = String(getFromStore(portfolio).documents[documentIndex].id);

			const tabOrder = (await get<string[]>("documents_tab_order", graphiteStore)) || [];
			if (tabOrder.includes(documentId)) {
				await storeCurrentDocumentId(documentId);
			}
		} else {
			await del("current_document_id", graphiteStore);
		}
	}

	async function loadFirstDocument() {
		const previouslySavedDocuments = await get<Record<string, TriggerPersistenceWriteDocument>>("documents", graphiteStore);
		const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
		const currentDocumentId = await get<string>("current_document_id", graphiteStore);
		if (!previouslySavedDocuments || !documentOrder) return;

		const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

		if (currentDocumentId && currentDocumentId in previouslySavedDocuments) {
			const doc = previouslySavedDocuments[currentDocumentId];
			editor.handle.openAutoSavedDocument(BigInt(doc.documentId), doc.details.name, doc.details.isSaved, doc.document, false);
			editor.handle.selectDocument(BigInt(currentDocumentId));
		} else {
			const len = orderedSavedDocuments.length;
			if (len > 0) {
				const doc = orderedSavedDocuments[len - 1];
				editor.handle.openAutoSavedDocument(BigInt(doc.documentId), doc.details.name, doc.details.isSaved, doc.document, false);
				editor.handle.selectDocument(BigInt(doc.documentId));
			}
		}
	}

	async function loadRestDocuments() {
		const previouslySavedDocuments = await get<Record<string, TriggerPersistenceWriteDocument>>("documents", graphiteStore);
		const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
		const currentDocumentId = await get<string>("current_document_id", graphiteStore);
		if (!previouslySavedDocuments || !documentOrder) return;

		const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

		if (currentDocumentId) {
			const currentIndex = orderedSavedDocuments.findIndex((doc) => doc.documentId === currentDocumentId);
			const beforeCurrentIndex = currentIndex - 1;
			const afterCurrentIndex = currentIndex + 1;

			for (let i = beforeCurrentIndex; i >= 0; i--) {
				const { documentId, document, details } = orderedSavedDocuments[i];
				const { name, isSaved } = details;
				editor.handle.openAutoSavedDocument(BigInt(documentId), name, isSaved, document, true);
			}
			for (let i = afterCurrentIndex; i < orderedSavedDocuments.length; i++) {
				const { documentId, document, details } = orderedSavedDocuments[i];
				const { name, isSaved } = details;
				editor.handle.openAutoSavedDocument(BigInt(documentId), name, isSaved, document, false);
			}

			editor.handle.selectDocument(BigInt(currentDocumentId));
		} else {
			const length = orderedSavedDocuments.length;

			for (let i = length - 2; i >= 0; i--) {
				const { documentId, document, details } = orderedSavedDocuments[i];
				const { name, isSaved } = details;
				editor.handle.openAutoSavedDocument(BigInt(documentId), name, isSaved, document, true);
			}

			if (length > 0) {
				const id = orderedSavedDocuments[length - 1].documentId;
				editor.handle.selectDocument(BigInt(id));
			}
		}
	}

	// PREFERENCES

	async function savePreferences(preferences: TriggerSavePreferences["preferences"]) {
		await set("preferences", preferences, graphiteStore);
	}

	async function loadPreferences() {
		const preferences = await get<Record<string, unknown>>("preferences", graphiteStore);
		editor.handle.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
	}

	// FRONTEND MESSAGE SUBSCRIPTIONS

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerSavePreferences, async (preferences) => {
		await savePreferences(preferences.preferences);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadPreferences, async () => {
		await loadPreferences();
	});
	editor.subscriptions.subscribeJsMessage(TriggerPersistenceWriteDocument, async (autoSaveDocument) => {
		await storeDocument(autoSaveDocument);
	});
	editor.subscriptions.subscribeJsMessage(TriggerPersistenceRemoveDocument, async (removeAutoSaveDocument) => {
		await removeDocument(removeAutoSaveDocument.documentId);
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadFirstAutoSaveDocument, async () => {
		await loadFirstDocument();
	});
	editor.subscriptions.subscribeJsMessage(TriggerLoadRestAutoSaveDocuments, async () => {
		await loadRestDocuments();
	});
	editor.subscriptions.subscribeJsMessage(TriggerSaveActiveDocument, async (triggerSaveActiveDocument) => {
		const documentId = String(triggerSaveActiveDocument.documentId);
		const previouslySavedDocuments = await get<Record<string, TriggerPersistenceWriteDocument>>("documents", graphiteStore);
		if (!previouslySavedDocuments) return;
		if (documentId in previouslySavedDocuments) {
			await storeCurrentDocumentId(documentId);
		}
	});
}

export async function wipeDocuments() {
	await del("documents_tab_order", graphiteStore);
	await del("current_document_id", graphiteStore);
	await del("documents", graphiteStore);
}
