import { TriggerImport, TriggerImportWithDestination } from "@graphite/messages";
import { extractPixelData } from "@graphite/utility-functions/rasterization";
import type { Editor } from "@graphite/editor";

let pendingFiles: File[] | undefined;

export function setPendingImportFiles(files: File[]) {
	pendingFiles = files;
}

export function createImportManager(editor: Editor) {
	// Subscribe to TriggerImport to open a file picker in the browser
	editor.subscriptions.subscribeJsMessage(TriggerImport, () => {
		// If we already have pending files (e.g., from a desktop drop), just request the dialog now
		if (pendingFiles && pendingFiles.length > 0) {
			const api = editor.handle as unknown as { requestImportDialog?: () => void };
			api.requestImportDialog?.();
			return;
		}

		// Otherwise, open a file input to select files
		const input = document.createElement("input");
		input.type = "file";
		input.multiple = true;
		input.accept = ".graphite,.svg,image/*";
		input.onchange = () => {
			const files = input.files ? Array.from(input.files) : [];
			if (files.length === 0) return;
			pendingFiles = files;
			const api = editor.handle as unknown as { requestImportDialog?: () => void };
			api.requestImportDialog?.();
		};
		input.click();
	});

	// Subscribe to TriggerImportWithDestination to actually read files and import
	editor.subscriptions.subscribeJsMessage(TriggerImportWithDestination, async ({ destination }) => {
		const isNewDocument = destination === "NewDocument";
		let files = pendingFiles ? pendingFiles : [];
		pendingFiles = undefined;

		// If no files pending, prompt to choose files now, then proceed with import
		if (files.length === 0) {
			const input = document.createElement("input");
			input.type = "file";
			input.multiple = true;
			input.accept = ".graphite,.svg,image/*";
			files = await new Promise<File[]>((resolve) => {
				input.onchange = () => resolve(input.files ? Array.from(input.files) : []);
				input.click();
			});
			if (files.length === 0) return;
		}

		for (const file of files) {
			try {
				const graphiteSuffix = "." + editor.handle.fileExtension();
				if (file.name.endsWith(graphiteSuffix)) {
					const content = await file.text();
					const documentName = file.name.slice(0, -graphiteSuffix.length);
					editor.handle.openDocumentFile(documentName, content);
					continue;
				}

				if (file.type.includes("svg") || file.name.toLowerCase().endsWith(".svg")) {
					const svg = await file.text();
					const api = editor.handle as unknown as { importSvgWithDestination?: (name: string | undefined, svg: string, isNewDocument: boolean) => void };
					api.importSvgWithDestination?.(file.name, svg, isNewDocument);
					continue;
				}

				if (file.type.startsWith("image") || /\.(png|jpg|jpeg|webp|bmp|gif|avif)$/i.test(file.name)) {
					const imageData = await extractPixelData(file);
					const api = editor.handle as unknown as {
						importImageWithDestination?: (name: string | undefined, data: Uint8Array, width: number, height: number, isNewDocument: boolean) => void;
					};
					api.importImageWithDestination?.(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height, isNewDocument);
					continue;
				}
			} catch (e) {
				// eslint-disable-next-line no-console
				console.error("Failed to import file:", file.name, e);
			}
		}
	});

	// No destructor required currently
	return () => {};
}
