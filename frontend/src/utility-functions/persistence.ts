import type { MessageBody } from "/src/subscriptions-router";
import type { DocumentInfo, EditorWrapper, PersistedState } from "/wrapper/pkg/graphite_wasm_wrapper";

const PERSISTENCE_DB = "graphite";
const PERSISTENCE_STORE = "store";

function emptyPersistedState(): PersistedState {
	// eslint-disable-next-line camelcase
	return { documents: [], current_document: undefined, workspace_layout: undefined };
}

function createDocumentInfo(id: bigint, name: string, isSaved: boolean): DocumentInfo {
	// eslint-disable-next-line camelcase
	return { id, name, is_saved: isSaved };
}

// Reorder document entries to match the given ID ordering, appending any unmentioned entries at the end
function reorderDocuments(documents: DocumentInfo[], orderedIds: bigint[]): DocumentInfo[] {
	const byId = new Map(documents.map((entry) => [entry.id, entry]));
	const reordered: DocumentInfo[] = [];

	orderedIds.forEach((id) => {
		const existing = byId.get(id);
		if (existing) {
			reordered.push(existing);
			byId.delete(id);
		}
	});

	// Append any entries not yet present in the portfolio (e.g. documents still loading at startup)
	byId.forEach((entry) => reordered.push(entry));

	return reordered;
}

// ====================================
// State-based persistence (new format)
// ====================================

export async function writePersistedDocument(autoSaveDocument: MessageBody<"TriggerPersistenceWriteDocument">) {
	const { documentId, document } = autoSaveDocument;

	// Update content in the documents store
	await databaseUpdate<Record<string, string>>("documents", (old) => {
		const documents = old || {};
		documents[String(documentId)] = document;
		return documents;
	});
}

export async function readPersistedDocument(documentId: bigint, editor: EditorWrapper) {
	const documentContents = await databaseGet<Record<string, string>>("documents");
	if (!documentContents) return;

	const content = documentContents[String(documentId)];
	if (content === undefined) return;

	editor.loadDocumentContent(documentId, content);
}

export async function deletePersistedDocument(id: string) {
	// Remove content from the documents store
	await databaseUpdate<Record<string, string>>("documents", (old) => {
		const documents = old || {};
		delete documents[id];
		return documents;
	});
}

export async function writePersistedState(state: PersistedState) {
	// Keep state ordered and normalized before writing.
	state.documents = reorderDocuments(
		state.documents,
		state.documents.map((entry) => entry.id),
	);
	await databaseSet("state", state);
	await garbageCollectDocuments();
}

export async function readPersistedState(editor: EditorWrapper) {
	await migrateToNewFormat();
	await garbageCollectDocuments();

	const state = await databaseGet<PersistedState>("state");
	if (!state) return;
	editor.loadPersistedState(state);
}

export async function saveEditorPreferences(preferences: unknown) {
	await databaseSet("preferences", preferences);
}

export async function loadEditorPreferences(editor: EditorWrapper) {
	const preferences = await databaseGet<Record<string, unknown>>("preferences");
	editor.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
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
	await databaseDelete("workspace_layout");
}

// TODO: Eventually remove this document upgrade code
async function migrateToNewFormat() {
	// Detect the old format by checking for the existence of the "documents_tab_order" key
	const oldTabOrder = await databaseGet<string[]>("documents_tab_order");
	if (oldTabOrder === undefined) return;

	const oldDocuments = await databaseGet<Record<string, unknown>>("documents");

	// Build the new "state" and "documents" from the old format
	const newDocumentContents: Record<string, string> = {};
	const newDocumentInfos: DocumentInfo[] = [];

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

			const isSaved = extractIsSavedFromUnknown(details);

			newDocumentInfos.push(createDocumentInfo(id, name, isSaved));
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
function extractIsSavedFromUnknown(details: unknown): boolean {
	if (typeof details !== "object" || details === null) return false;

	// Old camelCase format
	if ("isSaved" in details) return Boolean(details.isSaved);

	// New snake_case format
	if ("is_saved" in details) return Boolean(details.is_saved);

	return false;
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
