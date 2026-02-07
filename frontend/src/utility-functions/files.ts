import { type Editor } from "@graphite/editor";
import { extractPixelData } from "@graphite/utility-functions/rasterization";

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

export function downloadFile(filename: string, content: ArrayBuffer) {
	const type = filename.endsWith(".svg") ? "image/svg+xml;charset=utf-8" : "application/octet-stream";

	const blob = new Blob([new Uint8Array(content)], { type });
	downloadFileBlob(filename, blob);
}

// See https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/file#accept for the `accept` string format
export async function upload<T extends "text" | "data" | "both">(accept: string, textOrData: T): Promise<UploadResult<T>> {
	return new Promise<UploadResult<T>>((resolve, _) => {
		const element = document.createElement("input");
		element.type = "file";
		element.accept = accept;

		element.addEventListener(
			"change",
			async () => {
				if (element.files?.length) {
					const file = element.files[0];

					const filename = file.name;
					const type = file.type;
					const content = (
						textOrData === "text"
							? await file.text()
							: textOrData === "data"
								? new Uint8Array(await file.arrayBuffer())
								: textOrData === "both"
									? { text: await file.text(), data: new Uint8Array(await file.arrayBuffer()) }
									: undefined
					) as UploadResultType<T>;

					resolve({ filename, type, content });
				}
			},
			{ capture: false, once: true },
		);

		element.click();

		// Once `element` goes out of scope, it has no references so it gets garbage collected along with its event listener, so `removeEventListener` is not needed
	});
}
export type UploadResult<T> = { filename: string; type: string; content: UploadResultType<T> };
type UploadResultType<T> = T extends "text" ? string : T extends "data" ? Uint8Array : T extends "both" ? { text: string; data: Uint8Array } : never;

export async function pasteFile(item: DataTransferItem, editor: Editor, mouse?: [number, number], insertParentId?: bigint, insertIndex?: number) {
	const file = item.getAsFile();
	if (!file) return;

	if (file.type.startsWith("image/svg")) {
		const svg = await file.text();
		editor.handle.pasteSvg(file.name, svg, mouse?.[0], mouse?.[1], insertParentId, insertIndex);
	} else if (file.type.startsWith("image/")) {
		const imageData = await extractPixelData(file);
		editor.handle.pasteImage(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height, mouse?.[0], mouse?.[1], insertParentId, insertIndex);
	} else if (file.name.endsWith("." + editor.handle.fileExtension())) {
		// TODO: When we eventually have sub-documents, this should be changed to import the document as a node instead of opening it in a separate tab
		editor.handle.openFile(file.name, await file.bytes());
	}
}
