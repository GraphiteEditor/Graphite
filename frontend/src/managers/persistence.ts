import { createStore, del, get, set, update } from "idb-keyval";
import { get as getFromStore } from "svelte/store";

import type { Editor } from "@graphite/editor";
import type { PortfolioStore } from "@graphite/stores/portfolio";
import type { MessageBody } from "@graphite/subscription-router";

const graphiteStore = createStore("graphite", "store");

let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor, PortfolioStore] | undefined;

export function createPersistenceManager(editor: Editor, portfolio: PortfolioStore) {
	currentArgs = [editor, portfolio];
	// DOCUMENTS

	// FRONTEND MESSAGE SUBSCRIPTIONS

	// Subscribe to process backend events
	editor.subscriptions.subscribeFrontendMessage("TriggerSavePreferences", async (data) => {
		await saveEditorPreferences(data.preferences);
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerLoadPreferences", async () => {
		await loadEditorPreferences(editor);
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerPersistenceWriteDocument", async (data) => {
		await storeDocument(data, portfolio);
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerPersistenceRemoveDocument", async (data) => {
		await removeDocument(String(data.documentId), portfolio);
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument", async () => {
		await loadFirstDocument(editor);
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments", async () => {
		await loadRestDocuments(editor);
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerOpenLaunchDocuments", async () => {
		// TODO: Could be used to load documents from URL params or similar on launch
	});
	editor.subscriptions.subscribeFrontendMessage("TriggerSaveActiveDocument", async (data) => {
		const documentId = String(data.documentId);
		const previouslySavedDocuments = await get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", graphiteStore);

		// TODO: Eventually remove this document upgrade code
		// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if needed
		if (previouslySavedDocuments) {
			Object.values(previouslySavedDocuments).forEach((doc) => {
				if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
			});
		}

		if (!previouslySavedDocuments) return;
		if (documentId in previouslySavedDocuments) {
			await storeCurrentDocumentId(documentId);
		}
	});

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("TriggerSavePreferences");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadPreferences");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerPersistenceWriteDocument");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerPersistenceRemoveDocument");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerOpenLaunchDocuments");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerSaveActiveDocument");
	}

	currentCleanup = destroy;
	return { destroy };
}
export type PersistenceManager = ReturnType<typeof createPersistenceManager>;

export async function wipeDocuments() {
	await del("documents_tab_order", graphiteStore);
	await del("current_document_id", graphiteStore);
	await del("documents", graphiteStore);
}

async function storeDocumentOrder(portfolio: PortfolioStore) {
	const documentOrder = getFromStore(portfolio).documents.map((doc) => String(doc.id));
	await set("documents_tab_order", documentOrder, graphiteStore);
}

async function storeCurrentDocumentId(documentId: string) {
	await set("current_document_id", String(documentId), graphiteStore);
}

async function storeDocument(autoSaveDocument: MessageBody<"TriggerPersistenceWriteDocument">, portfolio: PortfolioStore) {
	await update<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>(
		"documents",
		(old) => {
			const documents = old || {};
			documents[String(autoSaveDocument.documentId)] = autoSaveDocument;
			return documents;
		},
		graphiteStore,
	);

	await storeDocumentOrder(portfolio);
	await storeCurrentDocumentId(String(autoSaveDocument.documentId));
}

async function removeDocument(id: string, portfolio: PortfolioStore) {
	await update<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>(
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

async function loadFirstDocument(editor: Editor) {
	const previouslySavedDocuments = await get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", graphiteStore);

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if the browser is storing the old format as strings
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
	const currentDocumentIdString = await get<string>("current_document_id", graphiteStore);
	const currentDocumentId = currentDocumentIdString ? BigInt(currentDocumentIdString) : undefined;
	if (!previouslySavedDocuments || !documentOrder) return;

	const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

	if (currentDocumentId !== undefined && String(currentDocumentId) in previouslySavedDocuments) {
		const doc = previouslySavedDocuments[String(currentDocumentId)];
		editor.handle.openAutoSavedDocument(doc.documentId, doc.details.name, doc.details.isSaved, doc.document, false);
		editor.handle.selectDocument(currentDocumentId);
	} else {
		const len = orderedSavedDocuments.length;
		if (len > 0) {
			const doc = orderedSavedDocuments[len - 1];
			editor.handle.openAutoSavedDocument(doc.documentId, doc.details.name, doc.details.isSaved, doc.document, false);
			editor.handle.selectDocument(doc.documentId);
		}
	}
}

async function loadRestDocuments(editor: Editor) {
	const previouslySavedDocuments = await get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", graphiteStore);

	// TODO: Eventually remove this document upgrade code
	// Migrate TriggerPersistenceWriteDocument.documentId from string to bigint if needed
	if (previouslySavedDocuments) {
		Object.values(previouslySavedDocuments).forEach((doc) => {
			if (typeof doc.documentId === "string") doc.documentId = BigInt(doc.documentId);
		});
	}

	const documentOrder = await get<string[]>("documents_tab_order", graphiteStore);
	const currentDocumentIdString = await get<string>("current_document_id", graphiteStore);
	const currentDocumentId = currentDocumentIdString ? BigInt(currentDocumentIdString) : undefined;
	if (!previouslySavedDocuments || !documentOrder) return;

	const orderedSavedDocuments = documentOrder.flatMap((id) => (previouslySavedDocuments[id] ? [previouslySavedDocuments[id]] : []));

	if (currentDocumentId !== undefined) {
		const currentIndex = orderedSavedDocuments.findIndex((doc) => doc.documentId === currentDocumentId);
		const beforeCurrentIndex = currentIndex - 1;
		const afterCurrentIndex = currentIndex + 1;

		for (let i = beforeCurrentIndex; i >= 0; i--) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.handle.openAutoSavedDocument(documentId, name, isSaved, document, true);
		}
		for (let i = afterCurrentIndex; i < orderedSavedDocuments.length; i++) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.handle.openAutoSavedDocument(documentId, name, isSaved, document, false);
		}

		editor.handle.selectDocument(currentDocumentId);
	} else {
		const length = orderedSavedDocuments.length;

		for (let i = length - 2; i >= 0; i--) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.handle.openAutoSavedDocument(documentId, name, isSaved, document, true);
		}

		if (length > 0) editor.handle.selectDocument(orderedSavedDocuments[length - 1].documentId);
	}
}

// PREFERENCES

async function saveEditorPreferences(preferences: unknown) {
	await set("preferences", preferences, graphiteStore);
}

async function loadEditorPreferences(editor: Editor) {
	const preferences = await get<Record<string, unknown>>("preferences", graphiteStore);
	editor.handle.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createPersistenceManager(...currentArgs);
});
