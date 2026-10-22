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

// =======================
// State-based persistence
// =======================

export async function writePersistedState(state: PersistedState) {
	// Keep state ordered and normalized before writing.
	state.documents = reorderDocuments(
		state.documents,
		state.documents.map((entry) => entry.id),
	);
	await databaseSet("state", state);
}

export async function readPersistedState(editor: EditorWrapper) {
	await migrateToNewFormat();
	await migrateDocumentsToStore();

	const state = await databaseGet<PersistedState>("state");
	editor.loadPersistedState(state ?? emptyPersistedState());
}

export async function saveEditorPreferences(preferences: unknown) {
	await databaseSet("preferences", preferences);
}

export async function loadEditorPreferences(editor: EditorWrapper) {
	const preferences = await databaseGet<Record<string, unknown>>("preferences");
	editor.loadPreferences(preferences ? JSON.stringify(preferences) : undefined);
}

// =========================
// Migration from old format
// =========================

// TODO: Eventually remove this document upgrade code
async function migrateDocumentsToStore() {
	const documents = await databaseGet<Record<string, string>>("documents");
	if (!documents || Object.keys(documents).length === 0) return;

	const root = await navigator.storage.getDirectory();
	const store = await root.getDirectoryHandle("documents", { create: true });
	for (const [id, content] of Object.entries(documents)) {
		const directory = await store.getDirectoryHandle(BigInt(id).toString(16).padStart(16, "0"), { create: true });
		if (!(await fileExists(directory, "legacy.graphite")) && !(await fileExists(directory, "manifest.json"))) {
			const file = await directory.getFileHandle("legacy.graphite", { create: true });
			const writable = await file.createWritable();
			await writable.write(content);
			await writable.close();
		}
		await markDocumentMigrated(id, content);
	}
}

async function fileExists(directory: FileSystemDirectoryHandle, name: string): Promise<boolean> {
	try {
		await directory.getFileHandle(name);
		return true;
	} catch (error) {
		if (error instanceof DOMException && error.name === "NotFoundError") return false;
		throw error;
	}
}

// TODO: Eventually remove this document upgrade code
async function markDocumentMigrated(id: string, content: string) {
	const db = await databaseOpen();
	await new Promise<void>((resolve, reject) => {
		const transaction = db.transaction(PERSISTENCE_STORE, "readwrite");
		const store = transaction.objectStore(PERSISTENCE_STORE);
		const backupsRequest = store.get("documents_migrated");
		backupsRequest.onsuccess = () => {
			const backups: Record<string, string> = backupsRequest.result || {};
			backups[id] = content;
			store.put(backups, "documents_migrated");
			const documentsRequest = store.get("documents");
			documentsRequest.onsuccess = () => {
				const remaining: Record<string, string> = documentsRequest.result || {};
				delete remaining[id];
				store.put(remaining, "documents");
			};
		};
		transaction.oncomplete = () => resolve();
		transaction.onerror = () => reject(transaction.error);
	});
}

async function wipeStoredDocuments() {
	try {
		const root = await navigator.storage.getDirectory();
		await root.removeEntry("documents", { recursive: true });
	} catch {
		// Nothing stored, or OPFS is unavailable
	}
}

export async function wipeDocuments() {
	await databaseDelete("state");
	await databaseDelete("documents");
	await databaseDelete("documents_migrated");
	await wipeStoredDocuments();

	await wipeOldFormat();
}

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
			if (!oldEntry || typeof oldEntry !== "object") return;
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
			if (!details || typeof details !== "object") return;

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
	if (!details || typeof details !== "object") return false;

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
