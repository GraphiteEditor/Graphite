import { createStore, del, get, set, update } from "idb-keyval";
import { get as getFromStore } from "svelte/store";

import { type PortfolioState } from "~/src/state-providers/portfolio";
import { type Editor } from "~/src/wasm-communication/editor";
import { TriggerIndexedDbWriteDocument, TriggerIndexedDbRemoveDocument, TriggerSavePreferences, TriggerLoadAutoSaveDocuments, TriggerLoadPreferences } from "~/src/wasm-communication/messages";

const graphiteStore = createStore("graphite", "store");

export function createPersistenceManager(editor: Editor, portfolio: PortfolioState): void {
	// DOCUMENTS

	async function storeDocumentOrder(): Promise<void> {
		const documentOrder = getFromStore(portfolio).documents.map((doc) => String(doc.id));

		await set("documents_tab_order", documentOrder, graphiteStore);
	}

	async function storeDocument(autoSaveDocument: TriggerIndexedDbWriteDocument): Promise<void> {
		await update<Record<string, TriggerIndexedDbWriteDocument>>(
			"documents",
			(old) => {
				const documents = old || {};
				documents[autoSaveDocument.details.id] = autoSaveDocument;
				return documents;
			},
			graphiteStore
		);

		await storeDocumentOrder();
	}

	async function removeDocument(id: string): Promise<void> {
		await update<Record<string, TriggerIndexedDbWriteDocument>>(
			"documents",
			(old) => {
				const documents = old || {};
				delete documents[id];
				return documents;
			},
			graphiteStore
		);

		await storeDocumentOrder();
	}

	async function loadDocuments(): Promise<void> {
		const previouslySavedDocuments = await get<Record<string, TriggerIndexedDbWriteDocument>>("documents", graphiteStore);
		const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
		if (!previouslySavedDocuments || !documentOrder) return;

		const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

		const currentDocumentVersion = editor.instance.graphiteDocumentVersion();
		orderedSavedDocuments?.forEach(async (doc: TriggerIndexedDbWriteDocument) => {
			if (doc.version !== currentDocumentVersion) {
				await removeDocument(doc.details.id);
				return;
			}

			editor.instance.openAutoSavedDocument(BigInt(doc.details.id), doc.details.name, doc.details.isSaved, doc.document);
		});
	}

	// PREFERENCES

	async function savePreferences(preferences: TriggerSavePreferences["preferences"]): Promise<void> {
		await set("preferences", preferences, graphiteStore);
	}

	async function loadPreferences(): Promise<void> {
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

export async function wipeDocuments(): Promise<void> {
	await del("documents_tab_order", graphiteStore);
	await del("documents", graphiteStore);
}
