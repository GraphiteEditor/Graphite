import { extractPixelData } from "/src/utility-functions/rasterization";
import type { EditorWrapper, FileFilter } from "/wrapper/pkg/graphite_wasm_wrapper";

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

export async function pasteFile(item: DataTransferItem, editor: EditorWrapper, mouse?: [number, number], insertParentId?: bigint, insertIndex?: number) {
	const file = item.getAsFile();
	if (!file) return;

	if (file.type.startsWith("image/svg")) {
		const svg = await file.text();
		editor.pasteSvg(file.name, svg, mouse?.[0], mouse?.[1], insertParentId, insertIndex);
	} else if (file.type.startsWith("image/")) {
		const imageData = await extractPixelData(file);
		editor.pasteImage(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height, mouse?.[0], mouse?.[1], insertParentId, insertIndex);
	} else {
		// TODO: When we eventually have sub-documents, this should be changed to import the document as a node instead of opening it in a separate tab
		editor.openFile(file.name, await file.bytes());
	}
}

export function acceptStringFromFilters(filters: FileFilter[]): string {
	const extensions = filters.flatMap((filter) => filter.extensions);
	const imageMime = extensions.some((extension) => ["svg", "png", "jpg", "jpeg", "bmp", "gif", "webp", "avif", "tif", "tiff"].includes(extension)) ? ["image/*"] : [];
	return [...imageMime, ...extensions.map((extension) => `.${extension}`)].join(",");
}
