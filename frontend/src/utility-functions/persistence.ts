import { get } from "svelte/store";
import type { PortfolioStore } from "/src/stores/portfolio";
import type { MessageBody } from "/src/subscriptions-router";
import type { EditorWrapper, PersistedDocumentInfo, PersistedState } from "/wrapper/pkg/graphite_wasm_wrapper";

const PERSISTENCE_DB = "graphite";
const PERSISTENCE_STORE = "store";

function emptyPersistedState(): PersistedState {
	// eslint-disable-next-line camelcase
	return { documents: [], current_document: undefined };
}

function createDocumentInfo(id: bigint, name: string, isSaved: boolean): PersistedDocumentInfo {
	// eslint-disable-next-line camelcase
	return { id, name, is_saved: isSaved };
}

// ====================================
// State-based persistence (new format)
// ====================================

export async function storeDocumentTabOrder(portfolio: PortfolioStore) {
	const portfolioData = get(portfolio);
	const orderedIds = portfolioData.documents.map((doc) => doc.id);

	await databaseUpdate<PersistedState>("state", (old) => {
		const state = old || emptyPersistedState();

		// Reorder existing document entries to match the portfolio's tab order, preserving metadata
		const byId = new Map(state.documents.map((entry) => [entry.id, entry]));
		const reordered: PersistedDocumentInfo[] = [];
		orderedIds.forEach((id) => {
			const existing = byId.get(id);
			if (existing) {
				reordered.push(existing);
				byId.delete(id);
			}
		});

		// Append any entries not yet present in the portfolio (e.g. documents still loading at startup)
		byId.forEach((entry) => reordered.push(entry));

		return { ...state, documents: reordered };
	});
}

export async function storeDocument(autoSaveDocument: MessageBody<"TriggerPersistenceWriteDocument">, portfolio: PortfolioStore) {
	const { documentId, document, details } = autoSaveDocument;

	// Update content in the documents store
	await databaseUpdate<Record<string, string>>("documents", (old) => {
		const documents = old || {};
		documents[String(documentId)] = document;
		return documents;
	});

	// Update metadata and ordering in the state store
	const portfolioData = get(portfolio);
	const orderedIds = portfolioData.documents.map((doc) => doc.id);

	await databaseUpdate<PersistedState>("state", (old) => {
		const state = old || emptyPersistedState();

		// Update (or add) the document info entry
		const entry = createDocumentInfo(documentId, details.name, details.is_saved);
		const existingIndex = state.documents.findIndex((doc) => doc.id === documentId);
		if (existingIndex !== -1) {
			state.documents[existingIndex] = entry;
		} else {
			state.documents.push(entry);
		}

		// Reorder to match the portfolio's tab order
		const byId = new Map(state.documents.map((doc) => [doc.id, doc]));
		const reordered: PersistedDocumentInfo[] = [];
		orderedIds.forEach((id) => {
			const existing = byId.get(id);
			if (existing) {
				reordered.push(existing);
				byId.delete(id);
			}
		});

		// Append any entries not yet present in the portfolio (e.g. documents still loading at startup)
		byId.forEach((entry) => reordered.push(entry));

		// eslint-disable-next-line camelcase
		state.current_document = documentId;
		state.documents = reordered;
		return state;
	});
}

export async function removeDocument(id: string, portfolio: PortfolioStore) {
	const documentId = BigInt(id);

	// Remove content from the documents store
	await databaseUpdate<Record<string, string>>("documents", (old) => {
		const documents = old || {};
		delete documents[id];
		return documents;
	});

	// Update state: remove the entry and update current_document
	const portfolioData = get(portfolio);
	const documentCount = portfolioData.documents.length;

	await databaseUpdate<PersistedState>("state", (old) => {
		const state: PersistedState = old || emptyPersistedState();
		state.documents = state.documents.filter((doc) => doc.id !== documentId);

		if (state.current_document === documentId) {
			// eslint-disable-next-line camelcase
			state.current_document = documentCount > 0 ? portfolioData.documents[portfolioData.activeDocumentIndex].id : undefined;
		}

		return state;
	});
}

export async function loadDocuments(editor: EditorWrapper) {
	await migrateToNewFormat();
	await garbageCollectDocuments();

	const state = await databaseGet<PersistedState>("state");
	const documentContents = await databaseGet<Record<string, string>>("documents");
	if (!state || !documentContents || state.documents.length === 0) return;

	// Find the current document (or fall back to the last document in the list)
	const currentId = state.current_document;
	const currentEntry = currentId !== undefined ? state.documents.find((doc) => doc.id === currentId) : undefined;
	const current = currentEntry || state.documents[state.documents.length - 1];
	const currentIndex = state.documents.indexOf(current);

	// Open documents in order around the current document, placing earlier ones before it and later ones after
	state.documents.forEach((entry, index) => {
		const content = documentContents[String(entry.id)];
		if (content === undefined) return;

		const toFront = index < currentIndex;
		editor.openAutoSavedDocument(entry.id, entry.name, entry.is_saved, content, toFront);
	});

	editor.selectDocument(current.id);
}

export async function saveActiveDocument(documentId: bigint) {
	await databaseUpdate<PersistedState>("state", (old) => {
		const state: PersistedState = old || emptyPersistedState();

		const exists = state.documents.some((doc) => doc.id === documentId);
		// eslint-disable-next-line camelcase
		if (exists) state.current_document = documentId;

		return state;
	});
}

export async function saveEditorPreferences(preferences: unknown) {
	await databaseSet("preferences", preferences);
}

export async function loadEditorPreferences(editor: EditorWrapper) {
	const preferences = await databaseGet<Record<string, unknown>>("preferences");
	editor.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
}

export async function saveWorkspaceLayout(layout: unknown) {
	await databaseSet("workspace_layout", layout);
}

export async function loadWorkspaceLayout(editor: EditorWrapper) {
	const layout = await databaseGet<Record<string, unknown>>("workspace_layout");
	if (layout) editor.loadWorkspaceLayout(layout);
}

// Remove orphaned entries from the "documents" content store that have no corresponding entry in "state"
async function garbageCollectDocuments() {
	const state = await databaseGet<PersistedState>("state");
	const documentContents = await databaseGet<Record<string, string>>("documents");
	if (!documentContents) return;

	const validIds = new Set(state ? state.documents.map((doc) => String(doc.id)) : []);
	let changed = false;

	Object.keys(documentContents).forEach((key) => {
		if (!validIds.has(key)) {
			delete documentContents[key];
			changed = true;
		}
	});

	if (changed) await databaseSet("documents", documentContents);
}

export async function wipeDocuments() {
	await databaseDelete("state");
	await databaseDelete("documents");

	await wipeOldFormat();
}

// =========================
// Migration from old format
// =========================

// TODO: Eventually remove this document upgrade code
async function wipeOldFormat() {
	await databaseDelete("documents_tab_order");
	await databaseDelete("current_document_id");
}

// TODO: Eventually remove this document upgrade code
async function migrateToNewFormat() {
	// Detect the old format by checking for the existence of the "documents_tab_order" key
	const oldTabOrder = await databaseGet<string[]>("documents_tab_order");
	if (oldTabOrder === undefined) return;

	const oldDocuments = await databaseGet<Record<string, unknown>>("documents");

	// Build the new "state" and "documents" from the old format
	const newDocumentContents: Record<string, string> = {};
	const newDocumentInfos: PersistedDocumentInfo[] = [];

	if (oldDocuments) {
		Object.values(oldDocuments).forEach((value) => {
			const oldEntry: unknown = value;
			if (typeof oldEntry !== "object" || oldEntry === null) return;
			if (!("documentId" in oldEntry) || !("document" in oldEntry) || !("details" in oldEntry)) return;

			// Extract the document ID, handling bigint, number, and string formats
			let id: bigint;
			if (typeof oldEntry.documentId === "bigint") {
				id = oldEntry.documentId;
			} else if (typeof oldEntry.documentId === "number") {
				id = BigInt(oldEntry.documentId);
			} else if (typeof oldEntry.documentId === "string") {
				id = BigInt(oldEntry.documentId);
			} else {
				return;
			}

			// Extract the document content
			if (typeof oldEntry.document !== "string") return;
			newDocumentContents[String(id)] = oldEntry.document;

			// Extract document details, handling camelCase from the old shipped format
			const details: unknown = oldEntry.details;
			if (typeof details !== "object" || details === null) return;

			let name = "";
			if ("name" in details && typeof details.name === "string") name = details.name;

			const status = savedStatusFromUnknown(details);

			newDocumentInfos.push(createDocumentInfo(id, name, status.isSaved));
		});
	}

	const newState = emptyPersistedState();
	newState.documents = newDocumentInfos;

	// Write the new format
	await databaseSet("state", newState);
	await databaseSet("documents", newDocumentContents);

	// Delete old keys
	await databaseDelete("documents_tab_order");
	await databaseDelete("current_document_id");
}

// TODO: Eventually remove this document upgrade code
function savedStatusFromUnknown(details: unknown): { isSaved: boolean; isAutoSaved: boolean } {
	if (typeof details !== "object" || details === null) return { isSaved: false, isAutoSaved: false };

	// Old camelCase format
	if ("isSaved" in details && "isAutoSaved" in details) {
		return { isSaved: Boolean(details.isSaved), isAutoSaved: Boolean(details.isAutoSaved) };
	}

	// New snake_case format
	if ("is_saved" in details && "is_auto_saved" in details) {
		return { isSaved: Boolean(details.is_saved), isAutoSaved: Boolean(details.is_auto_saved) };
	}

	return { isSaved: false, isAutoSaved: false };
}

// =================
// IndexedDB helpers
// =================

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
