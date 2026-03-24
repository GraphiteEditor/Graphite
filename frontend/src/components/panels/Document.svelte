<script lang="ts">
	import { getContext, onMount, onDestroy, tick } from "svelte";
	import ColorPicker from "/src/components/floating-menus/ColorPicker.svelte";
	import EyedropperPreview, { ZOOM_WINDOW_DIMENSIONS } from "/src/components/floating-menus/EyedropperPreview.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import Graph from "/src/components/views/Graph.svelte";
	import RulerInput from "/src/components/widgets/inputs/RulerInput.svelte";
	import ScrollbarInput from "/src/components/widgets/inputs/ScrollbarInput.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { AppWindowStore } from "/src/stores/app-window";
	import type { DocumentStore } from "/src/stores/document";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import type { MessageBody } from "/src/subscriptions-router";
	import { fillChoiceColor, createColor } from "/src/utility-functions/colors";
	import { pasteFile } from "/src/utility-functions/files";
	import { textInputCleanup } from "/src/utility-functions/keyboard-entry";
	import { rasterizeSVGCanvas } from "/src/utility-functions/rasterization";
	import { setupViewportResizeObserver } from "/src/utility-functions/viewports";
	import type { Color, EditorWrapper, MenuDirection, MouseCursorIcon } from "/wrapper/pkg/graphite_wasm_wrapper";

	let rulerHorizontal: RulerInput | undefined;
	let rulerVertical: RulerInput | undefined;
	let viewport: HTMLDivElement | undefined;
	let gradientStopPicker: ColorPicker | undefined;

	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");
	const editor = getContext<EditorWrapper>("editor");
	const appWindow = getContext<AppWindowStore>("appWindow");
	const document = getContext<DocumentStore>("document");

	// Interactive text editing
	let textInput: undefined | HTMLDivElement = undefined;
	let showTextInput: boolean;
	let textInputMatrix: [number, number, number, number, number, number];

	// Scrollbars
	let scrollbarPos = { x: 0.5, y: 0.5 };
	let scrollbarSize = { x: 0.5, y: 0.5 };
	let scrollbarMultiplier = { x: 0, y: 0 };

	// Rulers
	let rulerOrigin = { x: 0, y: 0 };
	let rulerSpacing = 100;
	let rulerInterval = 100;
	let rulersVisible = true;

	// Rendered SVG viewport data
	let artworkSvg = "";

	// Rasterized SVG viewport data, or none if it's not up-to-date
	let rasterizedCanvas: HTMLCanvasElement | undefined = undefined;
	let rasterizedContext: CanvasRenderingContext2D | undefined = undefined;

	// Cursor icon to display while hovering over the canvas
	let canvasCursor = "default";

	// Cursor position for cursor floating menus like the Eyedropper tool zoom
	let cursorLeft = 0;
	let cursorTop = 0;
	let cursorEyedropper = false;
	let cursorEyedropperPreviewImageData: ImageData | undefined = undefined;
	let cursorEyedropperPreviewColorChoice = "";
	let cursorEyedropperPreviewColorPrimary = "";
	let cursorEyedropperPreviewColorSecondary = "";

	// Gradient stop color picker
	let gradientStopPickerColor: Color | undefined = undefined;
	let gradientStopPickerPosition: { x: number; y: number } | undefined = undefined;

	// Canvas dimensions
	let canvasWidth: number | undefined = undefined;
	let canvasHeight: number | undefined = undefined;

	let devicePixelRatio: number | undefined;
	let removeUpdatePixelRatio: (() => void) | undefined;
	let viewportResizeObserver: ResizeObserver | undefined;
	let cleanupViewportResizeObserver: (() => void) | undefined;
	let addedFontFaces: FontFace[] = [];

	// Dimension is rounded up to the nearest even number because resizing is centered, and dividing an odd number by 2 for centering causes antialiasing
	$: canvasWidthRoundedToEven = canvasWidth && (canvasWidth % 2 === 1 ? canvasWidth + 1 : canvasWidth);
	$: canvasHeightRoundedToEven = canvasHeight && (canvasHeight % 2 === 1 ? canvasHeight + 1 : canvasHeight);
	// Used to set the canvas element size on the page.
	// The value above in pixels, or if undefined, we fall back to 100% as a non-pixel-perfect backup that's hopefully short-lived
	$: canvasWidthCSS = canvasWidthRoundedToEven ? `${canvasWidthRoundedToEven}px` : "100%";
	$: canvasHeightCSS = canvasHeightRoundedToEven ? `${canvasHeightRoundedToEven}px` : "100%";

	$: canvasWidthScaled = canvasWidth && devicePixelRatio && Math.floor(canvasWidth * devicePixelRatio);
	$: canvasHeightScaled = canvasHeight && devicePixelRatio && Math.floor(canvasHeight * devicePixelRatio);

	// Used to set the canvas rendering dimensions.
	$: canvasWidthScaledRoundedToEven = canvasWidthScaled && (canvasWidthScaled % 2 === 1 ? canvasWidthScaled + 1 : canvasWidthScaled);
	$: canvasHeightScaledRoundedToEven = canvasHeightScaled && (canvasHeightScaled % 2 === 1 ? canvasHeightScaled + 1 : canvasHeightScaled);

	$: toolShelfTotalToolsAndSeparators = ((layoutGroup) => {
		if (!layoutGroup || !("Row" in layoutGroup)) return undefined;

		let totalSeparators = 0;
		let totalToolRowsFor1Columns = 0;
		let totalToolRowsFor2Columns = 0;
		let totalToolRowsFor3Columns = 0;

		const tally = () => {
			totalToolRowsFor1Columns += toolsInCurrentGroup;
			totalToolRowsFor2Columns += Math.ceil(toolsInCurrentGroup / 2);
			totalToolRowsFor3Columns += Math.ceil(toolsInCurrentGroup / 3);
			toolsInCurrentGroup = 0;
		};

		let toolsInCurrentGroup = 0;
		layoutGroup.Row.rowWidgets.forEach((widget) => {
			if ("Separator" in widget.widget) {
				totalSeparators += 1;
				tally();
			} else {
				toolsInCurrentGroup += 1;
			}
		});
		tally();

		return {
			totalSeparators,
			totalToolRowsFor1Columns,
			totalToolRowsFor2Columns,
			totalToolRowsFor3Columns,
		};
	})($document.toolShelfLayout[0]);

	function dropFile(e: DragEvent) {
		if (!e.dataTransfer) return;

		let mouse: [number, number] | undefined = undefined;
		if (e.target instanceof Element && e.target.closest("[data-viewport]")) mouse = [e.clientX, e.clientY];

		e.preventDefault();

		Array.from(e.dataTransfer.items).forEach(async (item) => await pasteFile(item, editor, mouse));
	}

	function panCanvasX(newValue: number) {
		const delta = newValue - scrollbarPos.x;
		scrollbarPos.x = newValue;
		editor.panCanvas(-delta * scrollbarMultiplier.x, 0);
	}

	function panCanvasY(newValue: number) {
		const delta = newValue - scrollbarPos.y;
		scrollbarPos.y = newValue;
		editor.panCanvas(0, -delta * scrollbarMultiplier.y);
	}

	function canvasPointerDown(e: PointerEvent) {
		const onEditbox = e.target instanceof HTMLDivElement && e.target.contentEditable;

		if (!onEditbox) viewport?.setPointerCapture(e.pointerId);
		if (window.document.activeElement instanceof HTMLElement) {
			window.document.activeElement.blur();
		}
	}

	// Update rendered SVGs
	export async function updateDocumentArtwork(svg: string) {
		// TODO: Sort this out so we're either sending only the SVG inner contents from the backend or not setting the width/height attributes here
		// TODO: (but preserving the rounding-up-to-the-next-even-number to prevent antialiasing).
		artworkSvg = svg
			.trim()
			.replace(/<svg[^>]*>/, "")
			.slice(0, -"</svg>".length);
		rasterizedCanvas = undefined;

		await tick();

		const placeholders = window.document.querySelectorAll("[data-viewport] [data-canvas-placeholder]");
		// Replace the placeholders with the actual canvas elements
		Array.from(placeholders).forEach((placeholder) => {
			const canvasName = placeholder.getAttribute("data-canvas-placeholder");
			if (!canvasName) return;
			// Get the canvas element from the global storage
			let canvas = window.imageCanvases[canvasName];

			// Get logical dimensions from foreignObject parent (set by backend)
			const foreignObject = placeholder.parentElement;
			if (!foreignObject) return;
			const logicalWidth = parseFloat(foreignObject.getAttribute("width") || "0");
			const logicalHeight = parseFloat(foreignObject.getAttribute("height") || "0");

			// Viewport canvas is marked with data-is-viewport and should never be cloned.
			// If it's already mounted in the viewport, skip the DOM replacement since it's already showing the rendered content.
			// We check `canvas.isConnected` to ensure it's in the live DOM, not a detached tree from a destroyed component.
			const isViewport = placeholder.hasAttribute("data-is-viewport");
			if (isViewport && canvas.isConnected && canvas.parentElement?.closest("[data-viewport]")) return;

			// Clone canvas for repeated instances (layers that appear multiple times)
			if (!isViewport && canvas.parentElement) {
				const newCanvas = window.document.createElement("canvas");
				const context = newCanvas.getContext("2d");

				newCanvas.width = canvas.width;
				newCanvas.height = canvas.height;

				context?.drawImage(canvas, 0, 0);

				canvas = newCanvas;
			}

			// Set CSS size to logical resolution (for correct display size)
			canvas.style.width = `${logicalWidth}px`;
			canvas.style.height = `${logicalHeight}px`;

			placeholder.replaceWith(canvas);
		});
	}

	export async function updateEyedropperSamplingState(
		// `image` is currently only used for Vello renders
		image: ImageData | undefined,
		mousePosition: [number, number] | undefined,
		colorPrimary: string,
		colorSecondary: string,
	): Promise<[number, number, number] | undefined> {
		if (mousePosition === undefined) {
			cursorEyedropper = false;
			return undefined;
		}
		cursorEyedropper = true;

		if (canvasWidth === undefined || canvasHeight === undefined) return undefined;

		cursorLeft = mousePosition[0];
		cursorTop = mousePosition[1];

		let preview = image;
		if (!preview) {
			// This works nearly perfectly, but sometimes at odd DPI scale factors like 1.25, the anti-aliasing color can yield slightly incorrect colors (potential room for future improvement)
			const dpiFactor = window.devicePixelRatio;
			const [width, height] = [canvasWidth, canvasHeight];

			const outsideArtboardsColor = getComputedStyle(window.document.documentElement).getPropertyValue("--color-2-mildblack");
			const outsideArtboards = `<rect x="0" y="0" width="100%" height="100%" fill="${outsideArtboardsColor}" />`;

			const svg = `
				<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" width="${width}" height="${height}">${outsideArtboards}${artworkSvg}</svg>
				`.trim();

			if (!rasterizedCanvas) {
				rasterizedCanvas = await rasterizeSVGCanvas(svg, width * dpiFactor, height * dpiFactor);
				rasterizedContext = rasterizedCanvas.getContext("2d", { willReadFrequently: true }) || undefined;
			}
			if (!rasterizedContext) return undefined;

			preview = rasterizedContext.getImageData(
				mousePosition[0] * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
				mousePosition[1] * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
				ZOOM_WINDOW_DIMENSIONS,
				ZOOM_WINDOW_DIMENSIONS,
			);
			if (!preview) return undefined;
		}

		const centerPixel = (() => {
			const { width, height, data } = preview;
			const x = Math.floor(width / 2);
			const y = Math.floor(height / 2);
			const index = (y * width + x) * 4;
			return {
				r: data[index],
				g: data[index + 1],
				b: data[index + 2],
			};
		})();
		const hex = [centerPixel.r, centerPixel.g, centerPixel.b].map((x) => x.toString(16).padStart(2, "0")).join("");
		const rgb: [number, number, number] = [centerPixel.r / 255, centerPixel.g / 255, centerPixel.b / 255];

		cursorEyedropperPreviewColorChoice = "#" + hex;
		cursorEyedropperPreviewColorPrimary = colorPrimary;
		cursorEyedropperPreviewColorSecondary = colorSecondary;
		cursorEyedropperPreviewImageData = preview;

		return rgb;
	}

	// Update scrollbars and rulers
	export function updateDocumentScrollbars(position: [number, number], size: [number, number], multiplier: [number, number]) {
		scrollbarPos = { x: position[0], y: position[1] };
		scrollbarSize = { x: size[0], y: size[1] };
		scrollbarMultiplier = { x: multiplier[0], y: multiplier[1] };
	}

	export function updateDocumentRulers(origin: [number, number], spacing: number, interval: number, visible: boolean) {
		rulerOrigin = { x: origin[0], y: origin[1] };
		rulerSpacing = spacing;
		rulerInterval = interval;
		rulersVisible = visible;
	}

	// Update mouse cursor icon
	export function updateMouseCursor(cursor: MouseCursorIcon) {
		const mouseCursorIconCSSNames: Record<MouseCursorIcon, string> = {
			Default: "default",
			None: "none",
			ZoomIn: "zoom-in",
			ZoomOut: "zoom-out",
			Grabbing: "grabbing",
			Crosshair: "crosshair",
			Text: "text",
			Move: "move",
			NSResize: "ns-resize",
			EWResize: "ew-resize",
			NESWResize: "nesw-resize",
			NWSEResize: "nwse-resize",
			Rotate: "custom-rotate",
		};
		let cursorString = mouseCursorIconCSSNames[cursor] || "alias";

		// This isn't very clean but it's good enough for now until we need more icons, then we can build something more robust (consider blob URLs)
		if (cursor === "Rotate") {
			const svg = `
				<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" width="20" height="20">
					<path fill="none" stroke="black" stroke-width="2" d="M10,15.8c-3.2,0-5.8-2.6-5.8-5.8S6.8,4.2,10,4.2c0.999,0,1.999,0.273,2.877,0.771L11.7,7h5.8l-2.9-5l-1.013,1.746C12.5,3.125,11.271,2.8,10,2.8C6,2.8,2.8,6,2.8,10S6,17.2,10,17.2s7.2-3.2,7.2-7.2h-1.4C15.8,13.2,13.2,15.8,10,15.8z" />
					<path fill="white" d="M10,15.8c-3.2,0-5.8-2.6-5.8-5.8S6.8,4.2,10,4.2c0.999,0,1.999,0.273,2.877,0.771L11.7,7h5.8l-2.9-5l-1.013,1.746C12.5,3.125,11.271,2.8,10,2.8C6,2.8,2.8,6,2.8,10S6,17.2,10,17.2s7.2-3.2,7.2-7.2h-1.4C15.8,13.2,13.2,15.8,10,15.8z" />
				</svg>
				`
				.split("\n")
				.map((line) => line.trim())
				.join("");

			cursorString = `url('data:image/svg+xml;utf8,${svg}') 8 8, alias`;
		}

		canvasCursor = cursorString;
	}

	function preventTextEditingScroll(e: Event) {
		if (!(e.target instanceof HTMLElement)) return;
		e.target.scrollTop = 0;
		e.target.scrollLeft = 0;
	}

	// Text entry
	export function triggerTextCommit() {
		if (!textInput) return;
		const textCleaned = textInputCleanup(textInput.innerText);
		editor.onChangeText(textCleaned, false);
	}

	export async function displayEditableTextbox(data: MessageBody<"DisplayEditableTextbox">) {
		showTextInput = true;

		await tick();

		if (!textInput) return;

		// eslint-disable-next-line svelte/no-dom-manipulating
		if (data.text === "") textInput.textContent = "";
		// eslint-disable-next-line svelte/no-dom-manipulating
		else textInput.textContent = `${data.text}\n`;

		// Make it so `maxHeight` is a multiple of `lineHeight`
		const lineHeight = data.lineHeightRatio * data.fontSize;
		let height = data.maxHeight === undefined ? "auto" : `${Math.floor(data.maxHeight / lineHeight) * lineHeight}px`;

		textInput.contentEditable = "true";
		textInput.style.transformOrigin = "0 0";
		textInput.style.width = data.maxWidth ? `${data.maxWidth}px` : "max-content";
		textInput.style.height = height;
		textInput.style.lineHeight = `${data.lineHeightRatio}`;
		textInput.style.fontSize = `${data.fontSize}px`;
		textInput.style.color = data.color;
		textInput.style.textAlign = data.align;

		textInput.oninput = () => {
			if (!textInput) return;
			editor.updateBounds(textInputCleanup(textInput.innerText));
		};

		textInputMatrix = data.transform;

		if (data.fontData.length > 0 && data.fontData.buffer instanceof ArrayBuffer) {
			const fontView = new Uint8Array(data.fontData.buffer, data.fontData.byteOffset, data.fontData.byteLength);
			const face = new FontFace("text-font", fontView);
			window.document.fonts.add(face);
			addedFontFaces.push(face);
			textInput.style.fontFamily = "text-font";
		}

		// Necessary to select contenteditable: https://stackoverflow.com/questions/6139107/programmatically-select-text-in-a-contenteditable-html-element/6150060#6150060

		const range = window.document.createRange();
		range.selectNodeContents(textInput);

		const selection = window.getSelection();
		if (selection) {
			selection.removeAllRanges();
			selection.addRange(range);
		}

		textInput.focus();
		textInput.click();

		// Sends the text input element used for interactively editing with the text tool in a custom event
		window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: textInput }));
	}

	export function displayRemoveEditableTextbox() {
		window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: undefined }));
		showTextInput = false;
	}

	function updateViewportInfo() {
		if (!viewport) return;
		// Resize the canvas
		canvasWidth = Math.ceil(parseFloat(getComputedStyle(viewport).width));
		canvasHeight = Math.ceil(parseFloat(getComputedStyle(viewport).height));

		devicePixelRatio = window.devicePixelRatio || 1;

		// Resize the rulers
		rulerHorizontal?.resize();
		rulerVertical?.resize();

		// Note: Viewport bounds are now sent to the backend by the ResizeObserver in viewports.ts
		// which provides pixel-perfect physical dimensions via devicePixelContentBoxSize
	}

	function gradientStopPickerDirection(position: { x: number; y: number } | undefined, viewport: HTMLDivElement | undefined): MenuDirection {
		const element = gradientStopPicker?.div()?.querySelector("[data-floating-menu-content]");
		const picker = element instanceof HTMLElement ? element : undefined;
		if (!picker || !position || !viewport) return "Bottom";

		const roomRight = position.x + picker.offsetWidth - viewport.clientWidth;
		const roomBelow = position.y + picker.offsetHeight - viewport.clientHeight;

		// Prefer bottom if there's room
		if (roomBelow <= 0) return "Bottom";
		// Otherwise choose the direction with more room
		return roomRight > roomBelow ? "Bottom" : "Right";
	}

	onMount(() => {
		// Not compatible with Safari:
		// <https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio#browser_compatibility>
		// <https://bugs.webkit.org/show_bug.cgi?id=124862>
		const updatePixelRatio = () => {
			removeUpdatePixelRatio?.();
			const mediaQueryList = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
			// The event is one-time use, so we have to set up a new listener and remove the old one every time
			mediaQueryList.addEventListener("change", updatePixelRatio);
			removeUpdatePixelRatio = () => mediaQueryList.removeEventListener("change", updatePixelRatio);

			updateViewportInfo();
		};
		updatePixelRatio();

		// Update rendered SVGs
		subscriptions.subscribeFrontendMessage("UpdateDocumentArtwork", async (data) => {
			await tick();

			updateDocumentArtwork(data.svg);
		});
		subscriptions.subscribeFrontendMessage("UpdateEyedropperSamplingState", async (data) => {
			await tick();

			const { image, mousePosition, primaryColor, secondaryColor, setColorChoice } = data;
			const imageData = image !== undefined ? new ImageData(new Uint8ClampedArray(image.data), image.width, image.height) : undefined;
			const rgb = await updateEyedropperSamplingState(imageData, mousePosition, primaryColor, secondaryColor);

			if (setColorChoice && rgb) {
				if (setColorChoice === "Primary") editor.updatePrimaryColor(...rgb, 1);
				if (setColorChoice === "Secondary") editor.updateSecondaryColor(...rgb, 1);
			}
		});

		// Gradient stop color picker
		subscriptions.subscribeFrontendMessage("UpdateGradientStopColorPickerPosition", (data) => {
			gradientStopPickerColor = data.color;
			gradientStopPickerPosition = { x: data.position[0], y: data.position[1] };
		});

		// Update scrollbars and rulers
		subscriptions.subscribeFrontendMessage("UpdateDocumentScrollbars", async (data) => {
			await tick();

			const { position, size, multiplier } = data;
			updateDocumentScrollbars(position, size, multiplier);
		});
		subscriptions.subscribeFrontendMessage("UpdateDocumentRulers", async (data) => {
			await tick();

			const { origin, spacing, interval, visible } = data;
			updateDocumentRulers(origin, spacing, interval, visible);
		});

		// Update mouse cursor icon
		subscriptions.subscribeFrontendMessage("UpdateMouseCursor", async (data) => {
			await tick();

			updateMouseCursor(data.cursor);
		});

		// Text entry
		subscriptions.subscribeFrontendMessage("TriggerTextCommit", async () => {
			await tick();

			triggerTextCommit();
		});
		subscriptions.subscribeFrontendMessage("DisplayEditableTextbox", async (data) => {
			await tick();

			displayEditableTextbox(data);
		});
		subscriptions.subscribeFrontendMessage("DisplayEditableTextboxUpdateFontData", async (data) => {
			await tick();

			if (textInput && data.fontData.length > 0 && data.fontData.buffer instanceof ArrayBuffer) {
				const fontView = new Uint8Array(data.fontData.buffer, data.fontData.byteOffset, data.fontData.byteLength);
				const face = new FontFace("text-font", fontView);
				window.document.fonts.add(face);
				addedFontFaces.push(face);
				textInput.style.fontFamily = "text-font";
			}
		});
		subscriptions.subscribeFrontendMessage("DisplayEditableTextboxTransform", async (data) => {
			textInputMatrix = data.transform;
		});
		subscriptions.subscribeFrontendMessage("DisplayRemoveEditableTextbox", async () => {
			await tick();

			displayRemoveEditableTextbox();
		});

		// Setup ResizeObserver for pixel-perfect viewport tracking with physical dimensions
		// This must happen in onMount to ensure the viewport container element exists
		cleanupViewportResizeObserver = setupViewportResizeObserver(editor);

		// Also observe the inner viewport for canvas sizing and ruler updates
		viewportResizeObserver = new ResizeObserver(() => {
			updateViewportInfo();
		});
		if (viewport) viewportResizeObserver.observe(viewport);
	});

	onDestroy(() => {
		cleanupViewportResizeObserver?.();
		viewportResizeObserver?.disconnect();
		removeUpdatePixelRatio?.();
		addedFontFaces.forEach((face) => window.document.fonts.delete(face));

		subscriptions.unsubscribeFrontendMessage("UpdateDocumentArtwork");
		subscriptions.unsubscribeFrontendMessage("UpdateEyedropperSamplingState");
		subscriptions.unsubscribeFrontendMessage("UpdateGradientStopColorPickerPosition");
		subscriptions.unsubscribeFrontendMessage("UpdateDocumentScrollbars");
		subscriptions.unsubscribeFrontendMessage("UpdateDocumentRulers");
		subscriptions.unsubscribeFrontendMessage("UpdateMouseCursor");
		subscriptions.unsubscribeFrontendMessage("TriggerTextCommit");
		subscriptions.unsubscribeFrontendMessage("DisplayEditableTextbox");
		subscriptions.unsubscribeFrontendMessage("DisplayEditableTextboxUpdateFontData");
		subscriptions.unsubscribeFrontendMessage("DisplayEditableTextboxTransform");
		subscriptions.unsubscribeFrontendMessage("DisplayRemoveEditableTextbox");
	});
</script>

<LayoutCol class="document" on:dragover={(e) => e.preventDefault()} on:drop={dropFile}>
	<LayoutRow class="control-bar" classes={{ "for-graph": $document.graphViewOverlayOpen }} scrollableX={true}>
		{#if !$document.graphViewOverlayOpen}
			<WidgetLayout layout={$document.toolOptionsLayout} layoutTarget="ToolOptions" />
			<LayoutRow class="spacer" />
			<WidgetLayout layout={$document.documentBarLayout} layoutTarget="DocumentBar" />
		{:else}
			<WidgetLayout layout={$document.nodeGraphControlBarLayout} layoutTarget="NodeGraphControlBar" />
		{/if}
	</LayoutRow>
	<LayoutRow
		class="tool-shelf-and-viewport-area"
		styles={toolShelfTotalToolsAndSeparators && {
			"--total-separators": toolShelfTotalToolsAndSeparators.totalSeparators,
			"--total-tool-rows-for-1-columns": toolShelfTotalToolsAndSeparators.totalToolRowsFor1Columns,
			"--total-tool-rows-for-2-columns": toolShelfTotalToolsAndSeparators.totalToolRowsFor2Columns,
			"--total-tool-rows-for-3-columns": toolShelfTotalToolsAndSeparators.totalToolRowsFor3Columns,
		}}
	>
		<LayoutCol class="tool-shelf">
			{#if !$document.graphViewOverlayOpen}
				<LayoutCol class="tools" scrollableY={true}>
					<WidgetLayout layout={$document.toolShelfLayout} layoutTarget="ToolShelf" />
				</LayoutCol>
			{:else}
				<LayoutRow class="spacer" />
			{/if}
			<LayoutCol class="tool-shelf-bottom-widgets">
				<WidgetLayout class="working-colors-input-area" layout={$document.workingColorsLayout} layoutTarget="WorkingColors" />
			</LayoutCol>
		</LayoutCol>
		<LayoutCol class="viewport-container">
			{#if rulersVisible}
				<LayoutRow class="ruler-or-scrollbar top-ruler">
					<LayoutCol class="ruler-corner"></LayoutCol>
					<RulerInput origin={rulerOrigin.x} majorMarkSpacing={rulerSpacing} numberInterval={rulerInterval} direction="Horizontal" bind:this={rulerHorizontal} />
				</LayoutRow>
			{/if}
			<LayoutRow class="viewport-container-inner-1">
				{#if rulersVisible}
					<LayoutCol class="ruler-or-scrollbar">
						<RulerInput origin={rulerOrigin.y} majorMarkSpacing={rulerSpacing} numberInterval={rulerInterval} direction="Vertical" bind:this={rulerVertical} />
					</LayoutCol>
				{/if}
				<LayoutCol class="viewport-container-inner-2" styles={{ cursor: canvasCursor }} data-viewport-container>
					{#if cursorEyedropper}
						<EyedropperPreview
							colorChoice={cursorEyedropperPreviewColorChoice}
							primaryColor={cursorEyedropperPreviewColorPrimary}
							secondaryColor={cursorEyedropperPreviewColorSecondary}
							imageData={cursorEyedropperPreviewImageData}
							x={cursorLeft}
							y={cursorTop}
						/>
					{/if}
					<div
						style:left={gradientStopPickerPosition ? `${gradientStopPickerPosition?.x}px` : undefined}
						style:top={gradientStopPickerPosition ? `${gradientStopPickerPosition?.y}px` : undefined}
						style:position="absolute"
						data-floating-menu-no-position
					>
						<div data-floating-menu-spawner></div>
						<ColorPicker
							direction={gradientStopPickerDirection(gradientStopPickerPosition, viewport)}
							open={Boolean(gradientStopPickerPosition && gradientStopPickerColor)}
							on:open={({ detail }) => {
								if (!detail) {
									editor.closeGradientStopColorPicker();
									gradientStopPickerPosition = undefined;
									gradientStopPickerColor = undefined;
								}
							}}
							colorOrGradient={{ Solid: gradientStopPickerColor || createColor(0, 0, 0, 1) }}
							on:colorOrGradient={({ detail }) => {
								const color = fillChoiceColor(detail);
								if (color) editor.updateGradientStopColor(color.red, color.green, color.blue, color.alpha);
							}}
							on:startHistoryTransaction={() => editor.startGradientStopColorTransaction()}
							on:commitHistoryTransaction={() => editor.commitGradientStopColorTransaction()}
							bind:this={gradientStopPicker}
						/>
					</div>
					<div
						class:viewport={!$appWindow.viewportHolePunch}
						class:viewport-transparent={$appWindow.viewportHolePunch}
						on:pointerdown={(e) => canvasPointerDown(e)}
						bind:this={viewport}
						data-viewport
					>
						{#if !$appWindow.viewportHolePunch}
							<svg class="artboards" style:width={canvasWidthCSS} style:height={canvasHeightCSS}>
								{@html artworkSvg}
							</svg>
						{/if}
						<div class="text-input" style:width={canvasWidthCSS} style:height={canvasHeightCSS} style:pointer-events={showTextInput ? "auto" : ""}>
							{#if showTextInput}
								<div bind:this={textInput} style:transform="matrix({textInputMatrix})" on:scroll={preventTextEditingScroll}></div>
							{/if}
						</div>
						{#if !$appWindow.viewportHolePunch}
							<canvas
								class="overlays"
								width={canvasWidthScaledRoundedToEven}
								height={canvasHeightScaledRoundedToEven}
								style:width={canvasWidthCSS}
								style:height={canvasHeightCSS}
								data-overlays-canvas
							>
							</canvas>
						{/if}
					</div>

					<div class="graph-view" class:open={$document.graphViewOverlayOpen} style:--fade-artwork={`${$document.fadeArtwork}%`} data-graph>
						<Graph />
					</div>
				</LayoutCol>
				<LayoutCol class="ruler-or-scrollbar right-scrollbar">
					<ScrollbarInput
						direction="Vertical"
						thumbLength={scrollbarSize.y}
						thumbPosition={scrollbarPos.y}
						on:trackShift={({ detail }) => editor.panCanvasByFraction(0, detail)}
						on:thumbPosition={({ detail }) => panCanvasY(detail)}
						on:thumbDragStart={() => editor.panCanvasAbortPrepare(false)}
						on:thumbDragAbort={() => editor.panCanvasAbort(false)}
					/>
				</LayoutCol>
			</LayoutRow>
			<LayoutRow class="ruler-or-scrollbar bottom-scrollbar">
				<ScrollbarInput
					direction="Horizontal"
					thumbLength={scrollbarSize.x}
					thumbPosition={scrollbarPos.x}
					on:trackShift={({ detail }) => editor.panCanvasByFraction(detail, 0)}
					on:thumbPosition={({ detail }) => panCanvasX(detail)}
					on:thumbDragStart={() => editor.panCanvasAbortPrepare(true)}
					on:thumbDragAbort={() => editor.panCanvasAbort(true)}
				/>
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.document {
		height: 100%;

		&.document.document {
			padding-bottom: 0;
		}

		.control-bar {
			height: 32px;
			flex: 0 0 auto;
			padding: 0 4px; // Padding (instead of margin) is needed for the viewport hole punch on desktop
			background: var(--color-3-darkgray); // Needed for the viewport hole punch on desktop

			.spacer {
				min-width: 40px;
			}

			&.for-graph {
				justify-content: space-between;
			}
		}

		.tool-shelf-and-viewport-area {
			// Enables usage of the `100cqh` unit to reference the height of this container element.
			container-type: size;

			// Update this if the tool icons change width in the future.
			--tool-width: 32;
			// Update this if the items below the tools (i.e. the working colors) change height in the future.
			--height-of-elements-below-tools: 72px;
			// Update this if the height changes as set in `Separator.svelte`.
			--height-of-separator: calc(12px + 1px + 12px);

			// Target height for the tools within the container above the lower elements.
			--available-height: calc(100cqh - var(--height-of-elements-below-tools));
			// The least height required to fit all the tools in 1 column and 2 columns, which the available space must exceed in order for the fewest needed columns to be used.
			--1-col-required-height: calc(var(--total-tool-rows-for-1-columns) * calc(var(--tool-width) * 1px) + var(--total-separators) * var(--height-of-separator));
			--2-col-required-height: calc(var(--total-tool-rows-for-2-columns) * calc(var(--tool-width) * 1px) + var(--total-separators) * var(--height-of-separator));

			// These evaluate to 0px (if false) or 1px (if true). (We multiply by 1000000 to force the result to be a discrete integer 0 or 1 and not interpolate values in-between.)
			--needs-at-least-1-column: 1px; // Always true
			--needs-at-least-2-columns: calc(1px - clamp(0px, calc((var(--available-height) - Min(var(--available-height), var(--1-col-required-height))) * 1000000), 1px));
			--needs-at-least-3-columns: calc(1px - clamp(0px, calc((var(--available-height) - Min(var(--available-height), var(--2-col-required-height))) * 1000000), 1px));
			--columns: calc(var(--needs-at-least-1-column) + var(--needs-at-least-2-columns) + var(--needs-at-least-3-columns));
			--columns-width: calc(var(--columns) * var(--tool-width));
			--columns-width-max: calc(3px * var(--tool-width));

			.tool-shelf {
				flex: 0 0 auto;
				justify-content: space-between;
				background: var(--color-3-darkgray); // Needed for the viewport hole punch on desktop

				.tools {
					flex: 0 1 auto;

					// Disabled because Firefox appears to have switched to using overlay scrollbars which float atop the content and don't affect the layout (as of FF 135 on Windows).
					// We'll keep this here in case it's needed in the future.
					//
					// Firefox-specific workaround for this bug causing the scrollbar to cover up the toolbar instead of widening to accommodate the scrollbar:
					// <https://bugzilla.mozilla.org/show_bug.cgi?id=764076>
					// <https://stackoverflow.com/questions/63278303/firefox-does-not-take-vertical-scrollbar-width-into-account-when-calculating-par>
					// Remove this when the Firefox bug is fixed.
					// @-moz-document url-prefix() {
					// 	--available-height-plus-1: calc(var(--available-height) + 1px);
					// 	--3-col-required-height: calc(var(--total-tool-rows-for-3-columns) * calc(var(--tool-width) * 1px) + var(--total-separators) * var(--height-of-separator));
					// 	--overflows-with-3-columns: calc(1px - clamp(0px, calc((var(--available-height-plus-1) - Min(var(--available-height-plus-1), var(--3-col-required-height))) * 1000000), 1px));
					// 	--firefox-scrollbar-width-space-occupied: 2; // Might change someday, or on different platforms, but this is the value in FF 120 on Windows
					// 	padding-right: calc(var(--firefox-scrollbar-width-space-occupied) * var(--overflows-with-3-columns));
					// }

					.widget-span {
						flex-wrap: wrap;
						width: var(--columns-width);

						.icon-button {
							margin: 0;

							// &[data-tooltip-description^="Coming soon."] {
							// 	opacity: 0.25;
							// 	transition: opacity 0.1s;

							// 	&:hover {
							// 		opacity: 1;
							// 	}
							// }

							&:not(.emphasized) {
								.color-general {
									fill: var(--color-data-general);
								}

								.color-vector {
									fill: var(--color-data-vector);
								}

								.color-raster {
									fill: var(--color-data-raster);
								}
							}
						}

						.separator {
							min-height: 0;
						}
					}
				}

				.tool-shelf-bottom-widgets {
					flex: 0 0 auto;
					align-items: center;

					.working-colors-input-area {
						height: auto;
						margin: 0;
						min-height: 0;

						.working-colors-input {
							margin: 0;
						}

						.icon-button {
							--widget-height: 0;
						}
					}
				}
			}

			.viewport-container {
				flex: 1 1 100%;

				.ruler-or-scrollbar {
					flex: 0 0 auto;
					background: var(--color-3-darkgray); // Needed for the viewport hole punch on desktop
				}

				.ruler-corner {
					background: var(--color-2-mildblack);
					width: 16px;
					position: relative;

					&::after {
						content: "";
						background: var(--color-5-dullgray);
						position: absolute;
						width: 1px;
						height: 1px;
						right: 0;
						bottom: 0;
					}
				}

				.top-ruler .ruler-input {
					margin-right: 16px;
				}

				&:has(.top-ruler) .right-scrollbar .scrollbar-input {
					margin-top: -16px;
				}

				.bottom-scrollbar .scrollbar-input {
					margin-right: 16px;
				}

				.viewport-container-inner-1,
				.viewport-container-inner-2 {
					flex: 1 1 100%;
					position: relative;

					.viewport {
						background: var(--color-2-mildblack);
					}

					.viewport,
					.viewport-transparent {
						width: 100%;
						height: 100%;
						// Allows the SVG to be placed at explicit integer values of width and height to prevent non-pixel-perfect SVG scaling
						position: relative;
						overflow: hidden;

						.artwork,
						.text-input,
						.overlays {
							position: absolute;
							top: 0;
							// Fallback values if JS hasn't set these to integers yet
							width: 100%;
							height: 100%;
							// Allows dev tools to select the artwork without being blocked by the SVG containers
							pointer-events: none;

							// Prevent inheritance from reaching the child elements
							> * {
								pointer-events: auto;
							}
						}

						.text-input {
							word-break: break-all;
							unicode-bidi: plaintext;
						}

						.text-input div {
							cursor: text;
							background: none;
							border: none;
							margin: 0;
							padding: 0;
							overflow-x: visible;
							overflow-y: hidden;
							overflow-wrap: anywhere;
							white-space: pre-wrap;
							word-break: normal;
							unicode-bidi: plaintext;
							display: inline-block;
							// Workaround to force Chrome to display the flashing text entry cursor when text is empty
							padding-left: 1px;
							margin-left: -1px;

							&:focus {
								border: none;
								outline: none; // Ok for contenteditable element
								margin: -1px;
							}
						}
					}

					.graph-view {
						pointer-events: none;
						transition: opacity 0.2s;
						opacity: 0;

						&.open {
							cursor: auto;
							pointer-events: auto;
							opacity: 1;
						}

						&::before {
							content: "";
							position: absolute;
							top: 0;
							left: 0;
							width: 100%;
							height: 100%;
							background: var(--color-2-mildblack);
							opacity: var(--fade-artwork);
							pointer-events: none;
						}
					}

					.fade-artwork,
					.graph {
						position: absolute;
						top: 0;
						left: 0;
						width: 100%;
						height: 100%;
					}
				}
			}
		}
	}
</style>
