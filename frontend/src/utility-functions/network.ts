export type RequestResult = { body: string; status: number };

// Special implementation using the legacy XMLHttpRequest API that provides callbacks to get:
// - Calls with the percent progress uploading the request to the server
// - Calls when downloading the result from the server, after the server has begun streaming back the response data
// It returns a tuple of the promise as well as the XHR which can be used to call the `.abort()` method on it.
export function requestWithUploadDownloadProgress(
	url: string,
	method: "GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "CONNECT" | "OPTIONS" | "TRACE" | "PATCH",
	body: string,
	uploadProgress: (progress: number) => void,
	downloadOccurring: () => void,
): [Promise<RequestResult>, XMLHttpRequest | undefined] {
	let xhrValue: XMLHttpRequest | undefined;
	const promise = new Promise<RequestResult>((resolve, reject) => {
		const xhr = new XMLHttpRequest();
		xhr.upload.addEventListener("progress", (e) => uploadProgress(e.loaded / e.total));
		xhr.addEventListener("progress", () => downloadOccurring());
		xhr.addEventListener("load", () => resolve({ status: xhr.status, body: xhr.responseText }));
		xhr.addEventListener("abort", () => resolve({ status: xhr.status, body: xhr.responseText }));
		xhr.addEventListener("error", () => reject(new Error("Request error")));
		xhr.open(method, url, true);
		xhr.setRequestHeader("accept", "*/*");
		xhr.setRequestHeader("accept-language", "en-US,en;q=0.9");
		xhr.setRequestHeader("content-type", "application/json");

		xhrValue = xhr;

		xhr.send(body);
	});

	return [promise, xhrValue];
}
