import { createStore, del, get, set, update } from "idb-keyval";
import { get as getFromStore } from "svelte/store";

import { type PortfolioState } from "@graphite/state-providers/portfolio";
import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerIndexedDbWriteDocument, TriggerIndexedDbRemoveDocument, TriggerSavePreferences, TriggerLoadAutoSaveDocuments, TriggerLoadPreferences } from "@graphite/wasm-communication/messages";

const graphiteStore = createStore("graphite", "store");

export function createPersistenceManager(editor: Editor, portfolio: PortfolioState) {
	// DOCUMENTS

	async function storeDocumentOrder() {
		const documentOrder = getFromStore(portfolio).documents.map((doc) => String(doc.id));

		await set("documents_tab_order", documentOrder, graphiteStore);
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

		await storeDocumentOrder();
	}

	async function loadDocuments() {
		const previouslySavedDocuments = await get<Record<string, TriggerIndexedDbWriteDocument>>("documents", graphiteStore);
		const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
		if (!previouslySavedDocuments || !documentOrder) return;

		const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

		orderedSavedDocuments?.forEach(async (doc: TriggerIndexedDbWriteDocument) => {
			editor.instance.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document);
		});
	}

	// PREFERENCES

	async function savePreferences(preferences: TriggerSavePreferences["preferences"]) {
		await set("preferences", preferences, graphiteStore);
	}

	async function loadPreferences() {
		const preferences = await get<Record<string, unknown>>("preferences", graphiteStore);
		if (!preferences) return;

		editor.instance.loadPreferences(JSON.stringify(preferences));
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
	editor.subscriptions.subscribeJsMessage(TriggerLoadAutoSaveDocuments, async () => {
		await loadDocuments();
	});
}

export async function wipeDocuments() {
	await del("documents_tab_order", graphiteStore);
	await del("documents", graphiteStore);
}
