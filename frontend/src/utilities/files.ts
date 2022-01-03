export function download(filename: string, fileData: string): void {
	const type = filename.endsWith(".svg") ? "image/svg+xml;charset=utf-8" : "text/plain;charset=utf-8";
	const blob = new Blob([fileData], { type });
	const url = URL.createObjectURL(blob);
	const element = document.createElement("a");

	element.href = url;
	element.setAttribute("download", filename);
	element.style.display = "none";

	element.click();
}

export async function upload(acceptedEextensions: string): Promise<{ filename: string; content: string }> {
	return new Promise<{ filename: string; content: string }>((resolve, _) => {
		const element = document.createElement("input");
		element.type = "file";
		element.style.display = "none";
		element.accept = acceptedEextensions;

		element.addEventListener(
			"change",
			async () => {
				if (!element.files || !element.files.length) return;
				const file = element.files[0];
				const filename = file.name;
				const content = await file.text();

				resolve({ filename, content });
			},
			{ capture: false, once: true }
		);

		element.click();
	});
}
