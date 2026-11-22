import type { FileFilter } from "/wrapper/pkg/graphite_wasm_wrapper";

export function downloadFileURL(filename: string, url: string) {
	const element = document.createElement("a");

	element.href = url;
	element.setAttribute("download", filename);

	element.click();
}

export function downloadFileBlob(filename: string, blob: Blob) {
	const url = URL.createObjectURL(blob);

	downloadFileURL(filename, url);

	URL.revokeObjectURL(url);
}

export function downloadFile(filename: string, content: Uint8Array) {
	const type = filename.endsWith(".svg") ? "image/svg+xml;charset=utf-8" : "application/octet-stream";

	if (content.length > 0 && content.buffer instanceof ArrayBuffer) {
		const contentView = new Uint8Array(content.buffer, content.byteOffset, content.byteLength);
		const blob = new Blob([contentView], { type });
		downloadFileBlob(filename, blob);
	}
}

// See https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/file#accept for the `accept` string format
export async function upload(accept: string, textOrData: "text"): Promise<UploadResult<string>>;
export async function upload(accept: string, textOrData: "data"): Promise<UploadResult<Uint8Array>>;
export async function upload(accept: string, textOrData: "both"): Promise<UploadResult<{ text: string; data: Uint8Array }>>;
export async function upload(accept: string, textOrData: "data", multiple: true): Promise<UploadResult<Uint8Array>[]>;
export async function upload(
	accept: string,
	textOrData: "text" | "data" | "both",
	multiple = false,
): Promise<UploadResult<string | Uint8Array | { text: string; data: Uint8Array }> | UploadResult<Uint8Array>[]> {
	return new Promise((resolve) => {
		const element = document.createElement("input");
		element.type = "file";
		element.accept = accept;
		element.multiple = multiple;

		element.addEventListener(
			"change",
			async () => {
				if (!element.files?.length) return;

				// The `multiple: true` overload constrains `textOrData` to "data", so we know each file produces a Uint8Array
				if (multiple) {
					const results = await Promise.all(
						Array.from(element.files).map(async (file) => ({
							filename: file.name,
							type: file.type,
							content: new Uint8Array(await file.arrayBuffer()),
						})),
					);
					resolve(results);
					return;
				}

				const file = element.files[0];
				const content =
					textOrData === "text"
						? await file.text()
						: textOrData === "data"
							? new Uint8Array(await file.arrayBuffer())
							: { text: await file.text(), data: new Uint8Array(await file.arrayBuffer()) };
				resolve({ filename: file.name, type: file.type, content });
			},
			{ capture: false, once: true },
		);

		element.click();

		// Once `element` goes out of scope, it has no references so it gets garbage collected along with its event listener, so `removeEventListener` is not needed
	});
}
export type UploadResult<T> = { filename: string; type: string; content: T };

export function acceptStringFromFilters(filters: FileFilter[]): string {
	return filters.flatMap((filter) => [...filter.mimeTypes, ...filter.extensions.map((extension) => `.${extension}`)]).join(",");
}

// Firefox and Safari lack the File System Access API
export function fileHandlesSupported(): boolean {
	return "showOpenFilePicker" in window && "showSaveFilePicker" in window;
}

// Matches the editor's `FILE_EXTENSION` and `GDD_FILE_EXTENSION`
const DOCUMENT_EXTENSIONS = ["graphite", "gdd"];

export function isDocumentFilename(filename: string): boolean {
	const extension = filename.includes(".") ? filename.split(".").pop()?.toLowerCase() : undefined;
	return extension !== undefined && DOCUMENT_EXTENSIONS.includes(extension);
}

function pickerTypes(filters: FileFilter[]): FilePickerAcceptType[] {
	return filters.map((filter) => {
		const extensions = filter.extensions.map((extension) => `.${extension}`);
		const mimeTypes = filter.mimeTypes.length > 0 ? filter.mimeTypes : ["application/octet-stream"];
		return { description: filter.name, accept: Object.fromEntries(mimeTypes.map((mimeType) => [mimeType, extensions])) };
	});
}

// Resolves to undefined if the user cancels the picker
async function picked<T>(picker: Promise<T>): Promise<T | undefined> {
	try {
		return await picker;
	} catch (error) {
		if (error instanceof DOMException && error.name === "AbortError") return undefined;
		throw error;
	}
}

export async function browseFileHandles(filters: FileFilter[], multiple: boolean): Promise<FileSystemFileHandle[]> {
	return (await picked(window.showOpenFilePicker({ multiple, types: pickerTypes(filters) }))) ?? [];
}

export async function saveFileHandle(suggestedName: string, filters: FileFilter[]): Promise<FileSystemFileHandle | undefined> {
	return picked(window.showSaveFilePicker({ suggestedName, types: pickerTypes(filters) }));
}

// Returns false if the user denies permission, which handles from earlier sessions ask for again
export async function writeFileHandle(handle: FileSystemFileHandle, content: Uint8Array): Promise<boolean> {
	if ((await handle.requestPermission({ mode: "readwrite" })) !== "granted" || !(content.buffer instanceof ArrayBuffer)) return false;

	const writable = await handle.createWritable();
	await writable.write(new Uint8Array(content.buffer, content.byteOffset, content.byteLength));
	await writable.close();
	return true;
}
