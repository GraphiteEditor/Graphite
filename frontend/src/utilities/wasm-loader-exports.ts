// These functions are exported to the wasm module.
// See /frontend/wasm/src/lib.rs
export function handleResponse(callback: unknown, responseType: ResponseType, responseData: Response) {
	if (typeof callback === "function") callback(responseType, responseData);
}

/* eslint-disable camelcase */
export function panicHook(panic_info: string, title: string, description: string) {
	// send the panic message to all active editors
	// eslint-disable-next-line @typescript-eslint/no-explicit-any, no-underscore-dangle
	const editorInstances: Set<any> = (window as any)._graphiteActiveEditorInstances;
	editorInstances.forEach((editor) => {
		editor.handleResponse("DisplayPanic", {
			DisplayPanic: {
				panic_info,
				title,
				description,
			},
		});
	});
}
