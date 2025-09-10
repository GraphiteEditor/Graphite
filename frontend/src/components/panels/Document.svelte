<script lang="ts">
	import { getContext, onMount, tick } from "svelte";

	import type { Editor } from "@graphite/editor";
	import {
		type MouseCursorIcon,
		type XY,
		DisplayEditableTextbox,
		DisplayEditableTextboxTransform,
		DisplayRemoveEditableTextbox,
		TriggerTextCommit,
		UpdateDocumentArtwork,
		UpdateDocumentRulers,
		UpdateDocumentScrollbars,
		UpdateEyedropperSamplingState,
		UpdateMouseCursor,
		isWidgetSpanRow,
	} from "@graphite/messages";
	import type { AppWindowState } from "@graphite/state-providers/app-window";
	import type { DocumentState } from "@graphite/state-providers/document";
	import { textInputCleanup } from "@graphite/utility-functions/keyboard-entry";
	import { extractPixelData, rasterizeSVGCanvas } from "@graphite/utility-functions/rasterization";
	import { updateBoundsOfViewports } from "@graphite/utility-functions/viewports";

	import EyedropperPreview, { ZOOM_WINDOW_DIMENSIONS } from "@graphite/components/floating-menus/EyedropperPreview.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import Graph from "@graphite/components/views/Graph.svelte";
	import RulerInput from "@graphite/components/widgets/inputs/RulerInput.svelte";
	import ScrollbarInput from "@graphite/components/widgets/inputs/ScrollbarInput.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	let rulerHorizontal: RulerInput | undefined;
	let rulerVertical: RulerInput | undefined;
	let viewport: HTMLDivElement | undefined;

	const editor = getContext<Editor>("editor");
	const appWindow = getContext<AppWindowState>("appWindow");
	const document = getContext<DocumentState>("document");

	// Interactive text editing
	let textInput: undefined | HTMLDivElement = undefined;
	let showTextInput: boolean;
	let textInputMatrix: number[];

	// Scrollbars
	let scrollbarPos: XY = { x: 0.5, y: 0.5 };
	let scrollbarSize: XY = { x: 0.5, y: 0.5 };
	let scrollbarMultiplier: XY = { x: 0, y: 0 };

	// Rulers
	let rulerOrigin: XY = { x: 0, y: 0 };
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

	// Canvas dimensions
	let canvasSvgWidth: number | undefined = undefined;
	let canvasSvgHeight: number | undefined = undefined;

	let devicePixelRatio: number | undefined;

	// Dimension is rounded up to the nearest even number because resizing is centered, and dividing an odd number by 2 for centering causes antialiasing
	$: canvasWidthRoundedToEven = canvasSvgWidth && (canvasSvgWidth % 2 === 1 ? canvasSvgWidth + 1 : canvasSvgWidth);
	$: canvasHeightRoundedToEven = canvasSvgHeight && (canvasSvgHeight % 2 === 1 ? canvasSvgHeight + 1 : canvasSvgHeight);
	// Used to set the canvas element size on the page.
	// The value above in pixels, or if undefined, we fall back to 100% as a non-pixel-perfect backup that's hopefully short-lived
	$: canvasWidthCSS = canvasWidthRoundedToEven ? `${canvasWidthRoundedToEven}px` : "100%";
	$: canvasHeightCSS = canvasHeightRoundedToEven ? `${canvasHeightRoundedToEven}px` : "100%";

	$: canvasWidthScaled = canvasSvgWidth && devicePixelRatio && Math.floor(canvasSvgWidth * devicePixelRatio);
	$: canvasHeightScaled = canvasSvgHeight && devicePixelRatio && Math.floor(canvasSvgHeight * devicePixelRatio);

	// Used to set the canvas rendering dimensions.
	$: canvasWidthScaledRoundedToEven = canvasWidthScaled && (canvasWidthScaled % 2 === 1 ? canvasWidthScaled + 1 : canvasWidthScaled);
	$: canvasHeightScaledRoundedToEven = canvasHeightScaled && (canvasHeightScaled % 2 === 1 ? canvasHeightScaled + 1 : canvasHeightScaled);

	$: toolShelfTotalToolsAndSeparators = ((layoutGroup) => {
		if (!isWidgetSpanRow(layoutGroup)) return undefined;

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
		layoutGroup.rowWidgets.forEach((widget) => {
			if (widget.props.kind === "Separator") {
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
	})($document.toolShelfLayout.layout[0]);

	function dropFile(e: DragEvent) {
		const { dataTransfer } = e;
		const [x, y] = e.target instanceof Element && e.target.closest("[data-viewport]") ? [e.clientX, e.clientY] : [undefined, undefined];
		if (!dataTransfer) return;

		e.preventDefault();

		Array.from(dataTransfer.items).forEach(async (item) => {
			const file = item.getAsFile();
			if (!file) return;

			if (file.type.includes("svg")) {
				const svgData = await file.text();
				editor.handle.pasteSvg(file.name, svgData, x, y);
				return;
			}

			if (file.type.startsWith("image")) {
				const imageData = await extractPixelData(file);
				editor.handle.pasteImage(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height, x, y);
				return;
			}

			const graphiteFileSuffix = "." + editor.handle.fileExtension();
			if (file.name.endsWith(graphiteFileSuffix)) {
				const content = await file.text();
				const documentName = file.name.slice(0, -graphiteFileSuffix.length);
				editor.handle.openDocumentFile(documentName, content);
				return;
			}
		});
	}

	function panCanvasX(newValue: number) {
		const delta = newValue - scrollbarPos.x;
		scrollbarPos.x = newValue;
		editor.handle.panCanvas(-delta * scrollbarMultiplier.x, 0);
	}

	function panCanvasY(newValue: number) {
		const delta = newValue - scrollbarPos.y;
		scrollbarPos.y = newValue;
		editor.handle.panCanvas(0, -delta * scrollbarMultiplier.y);
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
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			let canvas = (window as any).imageCanvases[canvasName];

			if (canvasName !== "0" && canvas.parentElement) {
				var newCanvas = window.document.createElement("canvas");
				var context = newCanvas.getContext("2d");

				newCanvas.width = canvas.width;
				newCanvas.height = canvas.height;

				context?.drawImage(canvas, 0, 0);

				canvas = newCanvas;
			}

			placeholder.replaceWith(canvas);
		});
	}

	export async function updateEyedropperSamplingState(mousePosition: XY | undefined, colorPrimary: string, colorSecondary: string): Promise<[number, number, number] | undefined> {
		if (mousePosition === undefined) {
			cursorEyedropper = false;
			return undefined;
		}
		cursorEyedropper = true;

		if (canvasSvgWidth === undefined || canvasSvgHeight === undefined) return undefined;

		cursorLeft = mousePosition.x;
		cursorTop = mousePosition.y;

		// This works nearly perfectly, but sometimes at odd DPI scale factors like 1.25, the anti-aliasing color can yield slightly incorrect colors (potential room for future improvement)
		const dpiFactor = window.devicePixelRatio;
		const [width, height] = [canvasSvgWidth, canvasSvgHeight];

		const outsideArtboardsColor = getComputedStyle(window.document.documentElement).getPropertyValue("--color-2-mildblack");
		const outsideArtboards = `<rect x="0" y="0" width="100%" height="100%" fill="${outsideArtboardsColor}" />`;

		const svg = `
			<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">${outsideArtboards}${artworkSvg}</svg>
			`.trim();

		if (!rasterizedCanvas) {
			rasterizedCanvas = await rasterizeSVGCanvas(svg, width * dpiFactor, height * dpiFactor);
			rasterizedContext = rasterizedCanvas.getContext("2d", { willReadFrequently: true }) || undefined;
		}
		if (!rasterizedContext) return undefined;

		const rgbToHex = (r: number, g: number, b: number): string => `#${[r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("")}`;

		const pixel = rasterizedContext.getImageData(mousePosition.x * dpiFactor, mousePosition.y * dpiFactor, 1, 1).data;
		const hex = rgbToHex(pixel[0], pixel[1], pixel[2]);
		const rgb: [number, number, number] = [pixel[0] / 255, pixel[1] / 255, pixel[2] / 255];

		cursorEyedropperPreviewColorChoice = hex;
		cursorEyedropperPreviewColorPrimary = colorPrimary;
		cursorEyedropperPreviewColorSecondary = colorSecondary;

		const previewRegion = rasterizedContext.getImageData(
			mousePosition.x * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
			mousePosition.y * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
			ZOOM_WINDOW_DIMENSIONS,
			ZOOM_WINDOW_DIMENSIONS,
		);
		cursorEyedropperPreviewImageData = previewRegion;

		return rgb;
	}

	// Update scrollbars and rulers
	export function updateDocumentScrollbars(position: XY, size: XY, multiplier: XY) {
		scrollbarPos = position;
		scrollbarSize = size;
		scrollbarMultiplier = multiplier;
	}

	export function updateDocumentRulers(origin: XY, spacing: number, interval: number, visible: boolean) {
		rulerOrigin = origin;
		rulerSpacing = spacing;
		rulerInterval = interval;
		rulersVisible = visible;
	}

	// Update mouse cursor icon
	export function updateMouseCursor(cursor: MouseCursorIcon) {
		let cursorString: string = cursor;

		// This isn't very clean but it's good enough for now until we need more icons, then we can build something more robust (consider blob URLs)
		if (cursor === "custom-rotate") {
			const svg = `
				<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" width="20" height="20">
					<path transform="translate(2 2)" fill="black" stroke="black" stroke-width="2px" d="
					M8,15.2C4,15.2,0.8,12,0.8,8C0.8,4,4,0.8,8,0.8c2,0,3.9,0.8,5.3,2.3l-1,1C11.2,2.9,9.6,2.2,8,2.2C4.8,2.2,2.2,4.8,2.2,8s2.6,5.8,5.8,5.8s5.8-2.6,5.8-5.8h1.4C15.2,12,12,15.2,8,15.2z
					" />
					<polygon transform="translate(2 2)" fill="black" stroke="black" stroke-width="2px" points="12.6,0 15.5,5 9.7,5" />
					<path transform="translate(2 2)" fill="white" d="
					M8,15.2C4,15.2,0.8,12,0.8,8C0.8,4,4,0.8,8,0.8c2,0,3.9,0.8,5.3,2.3l-1,1C11.2,2.9,9.6,2.2,8,2.2C4.8,2.2,2.2,4.8,2.2,8s2.6,5.8,5.8,5.8s5.8-2.6,5.8-5.8h1.4C15.2,12,12,15.2,8,15.2z
					" />
					<polygon transform="translate(2 2)" fill="white" points="12.6,0 15.5,5 9.7,5" />
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
		editor.handle.onChangeText(textCleaned, false);
	}

	export async function displayEditableTextbox(displayEditableTextbox: DisplayEditableTextbox) {
		showTextInput = true;

		await tick();

		if (!textInput) return;

		// eslint-disable-next-line svelte/no-dom-manipulating
		if (displayEditableTextbox.text === "") textInput.textContent = "";
		// eslint-disable-next-line svelte/no-dom-manipulating
		else textInput.textContent = `${displayEditableTextbox.text}\n`;

		// Make it so `maxHeight` is a multiple of `lineHeight`
		const lineHeight = displayEditableTextbox.lineHeightRatio * displayEditableTextbox.fontSize;
		let height = displayEditableTextbox.maxHeight === undefined ? "auto" : `${Math.floor(displayEditableTextbox.maxHeight / lineHeight) * lineHeight}px`;

		textInput.contentEditable = "true";
		textInput.style.transformOrigin = "0 0";
		textInput.style.width = displayEditableTextbox.maxWidth ? `${displayEditableTextbox.maxWidth}px` : "max-content";
		textInput.style.height = height;
		textInput.style.lineHeight = `${displayEditableTextbox.lineHeightRatio}`;
		textInput.style.fontSize = `${displayEditableTextbox.fontSize}px`;
		textInput.style.color = displayEditableTextbox.color.toHexOptionalAlpha() || "transparent";
		textInput.style.textAlign = displayEditableTextbox.align;

		textInput.oninput = () => {
			if (!textInput) return;
			editor.handle.updateBounds(textInputCleanup(textInput.innerText));
		};
		textInputMatrix = displayEditableTextbox.transform;
		const newFont = new FontFace("text-font", `url(${displayEditableTextbox.url})`);
		window.document.fonts.add(newFont);
		textInput.style.fontFamily = "text-font";

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

	onMount(() => {
		// Not compatible with Safari:
		// <https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio#browser_compatibility>
		// <https://bugs.webkit.org/show_bug.cgi?id=124862>
		let removeUpdatePixelRatio: (() => void) | undefined = undefined;
		const updatePixelRatio = () => {
			removeUpdatePixelRatio?.();
			const mediaQueryList = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
			// The event is one-time use, so we have to set up a new listener and remove the old one every time
			mediaQueryList.addEventListener("change", updatePixelRatio);
			removeUpdatePixelRatio = () => mediaQueryList.removeEventListener("change", updatePixelRatio);

			devicePixelRatio = window.devicePixelRatio;
			editor.handle.setDevicePixelRatio(devicePixelRatio);
		};
		updatePixelRatio();

		// Update rendered SVGs
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (data) => {
			await tick();

			updateDocumentArtwork(data.svg);
		});
		editor.subscriptions.subscribeJsMessage(UpdateEyedropperSamplingState, async (data) => {
			await tick();

			const { mousePosition, primaryColor, secondaryColor, setColorChoice } = data;
			const rgb = await updateEyedropperSamplingState(mousePosition, primaryColor, secondaryColor);

			if (setColorChoice && rgb) {
				if (setColorChoice === "Primary") editor.handle.updatePrimaryColor(...rgb, 1);
				if (setColorChoice === "Secondary") editor.handle.updateSecondaryColor(...rgb, 1);
			}
		});

		// Update scrollbars and rulers
		editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, async (data) => {
			await tick();

			const { position, size, multiplier } = data;
			updateDocumentScrollbars(position, size, multiplier);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, async (data) => {
			await tick();

			const { origin, spacing, interval, visible } = data;
			updateDocumentRulers(origin, spacing, interval, visible);
		});

		// Update mouse cursor icon
		editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, async (data) => {
			await tick();

			const { cursor } = data;
			updateMouseCursor(cursor);
		});

		// Text entry
		editor.subscriptions.subscribeJsMessage(TriggerTextCommit, async () => {
			await tick();

			triggerTextCommit();
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, async (data) => {
			await tick();

			displayEditableTextbox(data);
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextboxTransform, async (data) => {
			textInputMatrix = data.transform;
		});
		editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, async () => {
			await tick();

			displayRemoveEditableTextbox();
		});

		// Once this component is mounted, we want to resend the document bounds to the backend via the resize event handler which does that
		window.dispatchEvent(new Event("resize"));

		const viewportResizeObserver = new ResizeObserver(() => {
			if (!viewport) return;

			// Resize the canvas
			canvasSvgWidth = Math.ceil(parseFloat(getComputedStyle(viewport).width));
			canvasSvgHeight = Math.ceil(parseFloat(getComputedStyle(viewport).height));

			// Resize the rulers
			rulerHorizontal?.resize();
			rulerVertical?.resize();

			// Send the new bounds of the viewports to the backend
			if (viewport.parentElement) updateBoundsOfViewports(editor);
		});
		if (viewport) viewportResizeObserver.observe(viewport);
	});
</script>

<LayoutCol class="document" on:dragover={(e) => e.preventDefault()} on:drop={dropFile}>
	<LayoutRow class="control-bar" classes={{ "for-graph": $document.graphViewOverlayOpen }} scrollableX={true}>
		{#if !$document.graphViewOverlayOpen}
			<WidgetLayout layout={$document.documentModeLayout} />
			<WidgetLayout layout={$document.toolOptionsLayout} />
			<LayoutRow class="spacer" />
			<WidgetLayout layout={$document.documentBarLayout} />
		{:else}
			<WidgetLayout layout={$document.nodeGraphControlBarLayout} />
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
					<WidgetLayout layout={$document.toolShelfLayout} />
				</LayoutCol>
			{:else}
				<LayoutRow class="spacer" />
			{/if}
			<LayoutCol class="tool-shelf-bottom-widgets">
				<WidgetLayout class="working-colors-input-area" layout={$document.workingColorsLayout} />
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
								<div bind:this={textInput} style:transform="matrix({textInputMatrix})" on:scroll={preventTextEditingScroll} />
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
						on:trackShift={({ detail }) => editor.handle.panCanvasByFraction(0, detail)}
						on:thumbPosition={({ detail }) => panCanvasY(detail)}
						on:thumbDragStart={() => editor.handle.panCanvasAbortPrepare(false)}
						on:thumbDragAbort={() => editor.handle.panCanvasAbort(false)}
					/>
				</LayoutCol>
			</LayoutRow>
			<LayoutRow class="ruler-or-scrollbar bottom-scrollbar">
				<ScrollbarInput
					direction="Horizontal"
					thumbLength={scrollbarSize.x}
					thumbPosition={scrollbarPos.x}
					on:trackShift={({ detail }) => editor.handle.panCanvasByFraction(detail, 0)}
					on:thumbPosition={({ detail }) => panCanvasX(detail)}
					on:thumbDragEnd={() => editor.handle.setGridAlignedEdges()}
					on:thumbDragStart={() => editor.handle.panCanvasAbortPrepare(true)}
					on:thumbDragAbort={() => editor.handle.panCanvasAbort(true)}
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

							&[title^="Coming Soon"] {
								opacity: 0.25;
								transition: opacity 0.1s;

								&:hover {
									opacity: 1;
								}
							}

							&:not(.active) {
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

				.right-scrollbar .scrollbar-input {
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
