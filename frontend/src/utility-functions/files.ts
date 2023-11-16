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

export function downloadFileText(filename: string, text: string) {
	const type = filename.endsWith(".svg") ? "image/svg+xml;charset=utf-8" : "text/plain;charset=utf-8";

	const blob = new Blob([text], { type });
	downloadFileBlob(filename, blob);
}

export async function upload<T extends "text" | "data">(acceptedExtensions: string, textOrData: T): Promise<UploadResult<T>> {
	return new Promise<UploadResult<T>>((resolve, _) => {
		const element = document.createElement("input");
		element.type = "file";
		element.accept = acceptedExtensions;

		element.addEventListener(
			"change",
			async () => {
				if (element.files?.length) {
					const file = element.files[0];

					const filename = file.name;
					const type = file.type;
					const content = (textOrData === "text" ? await file.text() : new Uint8Array(await file.arrayBuffer())) as UploadResultType<T>;

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
type UploadResultType<T> = T extends "text" ? string : T extends "data" ? Uint8Array : never;

export function blobToBase64(blob: Blob): Promise<string> {
	return new Promise((resolve) => {
		const reader = new FileReader();
		reader.onloadend = () => resolve(typeof reader.result === "string" ? reader.result : "");
		reader.readAsDataURL(blob);
	});
}

export async function replaceBlobURLsWithBase64(svg: string): Promise<string> {
	const splitByBlobs = svg.split(/("blob:.*?")/);
	const onlyBlobs = splitByBlobs.filter((_, i) => i % 2 === 1);

	const onlyBlobsConverted = onlyBlobs.map(async (blobURL) => {
		const urlWithoutQuotes = blobURL.slice(1, -1);
		const data = await fetch(urlWithoutQuotes);
		const dataBlob = await data.blob();
		return blobToBase64(dataBlob);
	});
	const base64Images = await Promise.all(onlyBlobsConverted);

	const substituted = splitByBlobs.map((segment, i) => {
		if (i % 2 === 0) return segment;

		const blobsIndex = Math.floor(i / 2);
		return `"${base64Images[blobsIndex]}"`;
	});
	return substituted.join("");
}
