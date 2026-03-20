import * as idb from "idb-keyval";
import { get } from "svelte/store";

import type { Editor } from "@graphite/editor";
import type { PortfolioStore } from "@graphite/stores/portfolio";
import type { MessageBody } from "@graphite/subscription-router";

let editorRef: Editor | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;

export function createPersistenceManager(editor: Editor, portfolio: PortfolioStore) {
	editorRef = editor;
	portfolioStore = portfolio;

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
		const indexedDbStorage = idb.createStore("graphite", "store");

		const previouslySavedDocuments = await idb.get<Record<string, MessageBody<"TriggerPersistenceWriteDocument">>>("documents", indexedDbStorage);

		const documentId = String(data.documentId);

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
}

export function destroyPersistenceManager() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("TriggerSavePreferences");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadPreferences");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerPersistenceWriteDocument");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerPersistenceRemoveDocument");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerOpenLaunchDocuments");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerSaveActiveDocument");
}

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

export async function loadFirstDocument(editor: Editor) {
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

export async function loadRestDocuments(editor: Editor) {
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
			editor.handle.openAutoSavedDocument(documentId, name, isSaved, document, true);
		}
		for (let i = currentIndex + 1; i < orderedSavedDocuments.length; i++) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.handle.openAutoSavedDocument(documentId, name, isSaved, document, false);
		}

		editor.handle.selectDocument(currentDocumentId);
	}
	// No valid current document: open all remaining documents and select the last one
	else {
		const length = orderedSavedDocuments.length;

		for (let i = length - 2; i >= 0; i--) {
			const { documentId, document, details } = orderedSavedDocuments[i];
			const { name, isSaved } = details;
			editor.handle.openAutoSavedDocument(documentId, name, isSaved, document, true);
		}

		if (length > 0) editor.handle.selectDocument(orderedSavedDocuments[length - 1].documentId);
	}
}

export async function saveEditorPreferences(preferences: unknown) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.set("preferences", preferences, indexedDbStorage);
}

export async function loadEditorPreferences(editor: Editor) {
	const indexedDbStorage = idb.createStore("graphite", "store");

	const preferences = await idb.get<Record<string, unknown>>("preferences", indexedDbStorage);
	editor.handle.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
}

export async function wipeDocuments() {
	const indexedDbStorage = idb.createStore("graphite", "store");

	await idb.del("documents_tab_order", indexedDbStorage);
	await idb.del("current_document_id", indexedDbStorage);
	await idb.del("documents", indexedDbStorage);
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	destroyPersistenceManager();
	if (editorRef && portfolioStore) newModule?.createPersistenceManager(editorRef, portfolioStore);
});
