<script lang="ts">
	import { onMount, onDestroy, setContext } from "svelte";

	import { type Editor } from "@graphite/editor";
	import { createClipboardManager } from "@graphite/io-managers/clipboard";
	import { createDragManager } from "@graphite/io-managers/drag";
	import { createHyperlinkManager } from "@graphite/io-managers/hyperlinks";
	import { createInputManager } from "@graphite/io-managers/input";
	import { createLocalizationManager } from "@graphite/io-managers/localization";
	import { createPanicManager } from "@graphite/io-managers/panic";
	import { createPersistenceManager } from "@graphite/io-managers/persistence";
	import { createAppWindowState } from "@graphite/state-providers/app-window";
	import { createDialogState } from "@graphite/state-providers/dialog";
	import { createDocumentState } from "@graphite/state-providers/document";
	import { createFontsState } from "@graphite/state-providers/fonts";
	import { createFullscreenState } from "@graphite/state-providers/fullscreen";
	import { createNodeGraphState } from "@graphite/state-providers/node-graph";
	import { createPortfolioState } from "@graphite/state-providers/portfolio";
	import { operatingSystem } from "@graphite/utility-functions/platform";

	import MainWindow from "@graphite/components/window/MainWindow.svelte";

	// Graphite WASM editor
	export let editor: Editor;
	setContext("editor", editor);

	// State provider systems
	let dialog = createDialogState(editor);
	setContext("dialog", dialog);
	let document = createDocumentState(editor);
	setContext("document", document);
	let fonts = createFontsState(editor);
	setContext("fonts", fonts);
	let fullscreen = createFullscreenState(editor);
	setContext("fullscreen", fullscreen);
	let nodeGraph = createNodeGraphState(editor);
	setContext("nodeGraph", nodeGraph);
	let portfolio = createPortfolioState(editor);
	setContext("portfolio", portfolio);
	let appWindow = createAppWindowState(editor);
	setContext("appWindow", appWindow);

	// Initialize managers, which are isolated systems that subscribe to backend messages to link them to browser API functionality (like JS events, IndexedDB, etc.)
	createClipboardManager(editor);
	createHyperlinkManager(editor);
	createLocalizationManager(editor);
	createPanicManager(editor, dialog);
	createPersistenceManager(editor, portfolio);
	let dragManagerDestructor = createDragManager();
	let inputManagerDestructor = createInputManager(editor, dialog, portfolio, document, fullscreen);

	onMount(() => {
		// Initialize certain setup tasks required by the editor backend to be ready for the user now that the frontend is ready
		editor.handle.initAfterFrontendReady(operatingSystem());
	});

	onDestroy(() => {
		// Call the destructor for each manager
		dragManagerDestructor();
		inputManagerDestructor();
	});
</script>

<MainWindow platform={$appWindow.platform} maximized={$appWindow.maximized} viewportHolePunch={$appWindow.viewportHolePunch} />

<style lang="scss" global>
	// Disable the spinning loading indicator
	body::before,
	body::after {
		content: none !important;
	}

	:root {
		// Replace usage of `-rgb` variants with CSS color() function to calculate alpha when browsers support it
		// See https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/color() and https://caniuse.com/css-color-function
		// Specifically, support for the relative syntax is needed: `color(from var(--color-0-black) srgb r g b / 0.5)` to convert black to 50% alpha
		--color-0-black: #000;
		--color-0-black-rgb: 0, 0, 0;
		--color-1-nearblack: #111;
		--color-1-nearblack-rgb: 17, 17, 17;
		--color-2-mildblack: #222;
		--color-2-mildblack-rgb: 34, 34, 34;
		--color-3-darkgray: #333;
		--color-3-darkgray-rgb: 51, 51, 51;
		--color-4-dimgray: #444;
		--color-4-dimgray-rgb: 68, 68, 68;
		--color-5-dullgray: #555;
		--color-5-dullgray-rgb: 85, 85, 85;
		--color-6-lowergray: #666;
		--color-6-lowergray-rgb: 102, 102, 102;
		--color-7-middlegray: #777;
		--color-7-middlegray-rgb: 109, 109, 109;
		--color-8-uppergray: #888;
		--color-8-uppergray-rgb: 136, 136, 136;
		--color-9-palegray: #999;
		--color-9-palegray-rgb: 153, 153, 153;
		--color-a-softgray: #aaa;
		--color-a-softgray-rgb: 170, 170, 170;
		--color-b-lightgray: #bbb;
		--color-b-lightgray-rgb: 187, 187, 187;
		--color-c-brightgray: #ccc;
		--color-c-brightgray-rgb: 204, 204, 204;
		--color-d-mildwhite: #ddd;
		--color-d-mildwhite-rgb: 221, 221, 221;
		--color-e-nearwhite: #eee;
		--color-e-nearwhite-rgb: 238, 238, 238;
		--color-f-white: #fff;
		--color-f-white-rgb: 255, 255, 255;
		--color-error-red: #d6536e;
		--color-error-red-rgb: 214, 83, 110;
		--color-warning-yellow: #d5aa43;
		--color-warning-yellow-rgb: 213, 170, 67;

		--color-data-general: #cfcfcf;
		--color-data-general-dim: #8a8a8a;
		--color-data-number: #c9a699;
		--color-data-number-dim: #886b60;
		--color-data-artboard: #fbf9eb;
		--color-data-artboard-dim: #b9b9a9;
		--color-data-graphic: #68c587;
		--color-data-graphic-dim: #37754c;
		--color-data-raster: #e4bb72;
		--color-data-raster-dim: #9a7b43;
		--color-data-vector: #65bbe5;
		--color-data-vector-dim: #417892;
		--color-data-color: #ce6ea7;
		--color-data-color-dim: #924071;
		--color-data-gradient: #af81eb;
		--color-data-gradient-dim: #6c489b;
		--color-data-typography: #eea7a7;
		--color-data-typography-dim: #955252;

		--color-none: white;
		--color-none-repeat: no-repeat;
		--color-none-position: center center;
		// 24px tall, 48px wide
		--color-none-size-24px: 60px 24px;
		// Red diagonal slash (24px tall)
		--color-none-image-24px: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 24"><line stroke="red" stroke-width="4px" x1="0" y1="27" x2="60" y2="-3" /></svg>\
			');
		// 32px tall, 64px wide
		--color-none-size-32px: 80px 32px;
		// Red diagonal slash (32px tall)
		--color-none-image-32px: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 32"><line stroke="red" stroke-width="4px" x1="0" y1="36" x2="80" y2="-4" /></svg>\
			');

		--color-transparent-checkered-background:
			linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%), linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%),
			linear-gradient(#ffffff, #ffffff);
		--color-transparent-checkered-background-size: 16px 16px, 16px 16px, 16px 16px;
		--color-transparent-checkered-background-position: 0 0, 8px 8px, 8px 8px;
		--color-transparent-checkered-background-position-plus-one: 1px 1px, 9px 9px, 9px 9px;
		--color-transparent-checkered-background-size-mini: 8px 8px, 8px 8px, 8px 8px;
		--color-transparent-checkered-background-position-mini: 0 0, 4px 4px, 4px 4px;
		--color-transparent-checkered-background-repeat: repeat, repeat, repeat;

		--inheritance-stripes-background: repeating-linear-gradient(
			-45deg,
			transparent 0px,
			transparent calc((3px * sqrt(2) / 2) - 0.5px),
			var(--color-5-dullgray) calc((3px * sqrt(2) / 2) - 0.5px),
			var(--color-5-dullgray) calc((3px * sqrt(2) / 2) + 0.5px),
			transparent calc((3px * sqrt(2) / 2) + 0.5px),
			transparent calc(6px * sqrt(2) / 2)
		);
		--inheritance-dots-background-4-dimgray: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4" width="4px" height="4px" fill="%23444"><rect width="1" height="1" /><rect x="2" y="2" width="1" height="1" /></svg>\
			');
		--inheritance-dots-background-6-lowergray: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4" width="4px" height="4px" fill="%23666"><rect width="1" height="1" /><rect x="2" y="2" width="1" height="1" /></svg>\
			');

		// Array of 2x3 dots (fill: --color-e-nearwhite)
		--icon-drag-grip: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 24" fill="%23eee">\
				<circle cx="0.5" cy="1.5" r="0.5" /><circle cx="3.5" cy="1.5" r="0.5" />\
				<circle cx="0.5" cy="4.5" r="0.5" /><circle cx="3.5" cy="4.5" r="0.5" />\
				<circle cx="0.5" cy="7.5" r="0.5" /><circle cx="3.5" cy="7.5" r="0.5" />\
			</svg>\
			');
		// Array of 2x3 dots (fill: --color-f-white)
		--icon-drag-grip-hover: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 24" fill="%23fff">\
				<circle cx="0.5" cy="1.5" r="0.5" /><circle cx="3.5" cy="1.5" r="0.5" />\
				<circle cx="0.5" cy="4.5" r="0.5" /><circle cx="3.5" cy="4.5" r="0.5" />\
				<circle cx="0.5" cy="7.5" r="0.5" /><circle cx="3.5" cy="7.5" r="0.5" />\
			</svg>\
			');
		// Array of 2x3 dots (fill: --color-8-uppergray)
		--icon-drag-grip-disabled: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 24" fill="%23888">\
				<circle cx="0.5" cy="1.5" r="0.5" /><circle cx="3.5" cy="1.5" r="0.5" />\
				<circle cx="0.5" cy="4.5" r="0.5" /><circle cx="3.5" cy="4.5" r="0.5" />\
				<circle cx="0.5" cy="7.5" r="0.5" /><circle cx="3.5" cy="7.5" r="0.5" />\
			</svg>\
			');

		// Arrow triangle (fill: --color-e-nearwhite)
		--icon-expand-collapse-arrow: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><polygon fill="%23eee" points="3,0 1,0 5,4 1,8 3,8 7,4" /></svg>\
			');
		// Arrow triangle (fill: --color-f-white)
		--icon-expand-collapse-arrow-hover: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><polygon fill="%23fff" points="3,0 1,0 5,4 1,8 3,8 7,4" /></svg>\
			');
		// Arrow triangle (fill: --color-8-uppergray)
		--icon-expand-collapse-arrow-disabled: url('data:image/svg+xml;utf8,\
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><polygon fill="%23888" points="3,0 1,0 5,4 1,8 3,8 7,4" /></svg>\
			');
	}

	html,
	body {
		margin: 0;
		height: 100%;
		background: var(--color-2-mildblack);
		overscroll-behavior: none;
		-webkit-user-select: none; // Still required by Safari as of 2025
		user-select: none;
	}

	// Needed for the viewport hole punch on desktop
	html:has(body > .viewport-hole-punch),
	body:has(> .viewport-hole-punch) {
		background: none;
	}

	// The default value of `auto` from the CSS spec is a footgun with flexbox layouts:
	// https://stackoverflow.com/questions/36247140/why-dont-flex-items-shrink-past-content-size
	* {
		min-width: 0;
		min-height: 0;
	}

	html,
	body,
	input,
	textarea,
	button {
		font-family: "Source Sans Pro", Arial, sans-serif;
		font-weight: 400;
		font-size: 14px;
		line-height: 1;
		color: var(--color-e-nearwhite);
	}

	svg,
	img {
		display: block;
	}

	.layout-row,
	.layout-col {
		.scrollable-x,
		.scrollable-y {
			overflow: hidden;

			scrollbar-width: thin;
			// Not supported in Safari
			scrollbar-color: var(--color-5-dullgray) transparent;

			// Safari (more capable, removed from recent versions of Chromium, possibly still supported in Safari but not tested)
			&::-webkit-scrollbar {
				width: calc(2px + 6px + 2px);
				height: calc(2px + 6px + 2px);
			}

			&::-webkit-scrollbar-track {
				box-shadow: inset 0 0 0 1px var(--color-5-dullgray);
				border: 2px solid transparent;
				border-radius: 10px;
			}

			&:hover::-webkit-scrollbar-track {
				box-shadow: inset 0 0 0 1px var(--color-6-lowergray);
			}

			&::-webkit-scrollbar-thumb {
				background-clip: padding-box;
				background-color: var(--color-5-dullgray);
				border: 2px solid transparent;
				border-radius: 10px;
				margin: 2px;
			}

			&:hover::-webkit-scrollbar-thumb {
				background-color: var(--color-6-lowergray);
			}

			&::-webkit-scrollbar-corner {
				background: none;
			}
		}

		.scrollable-x.scrollable-y {
			overflow: auto;
		}

		.scrollable-x:not(.scrollable-y) {
			overflow: auto hidden;
		}

		.scrollable-y:not(.scrollable-x) {
			overflow: hidden auto;
		}
	}

	// List of all elements that should show an outline when focused by tabbing or by clicking the element
	.dropdown-input .dropdown-box,
	.font-input .dropdown-box {
		&:focus {
			outline: 1px dashed var(--color-e-nearwhite);
			outline-offset: -1px;
		}
	}

	// List of all elements that should show an outline when focused by tabbing, but not by clicking the element
	.icon-button,
	.text-button,
	.popover-button,
	.color-button > button,
	.color-picker .preset-color,
	.working-colors-input .swatch > button,
	.radio-input button,
	.menu-list,
	.menu-list-button .entry,
	.layer-tree .expand-arrow,
	.widget-section .header {
		&:focus-visible {
			outline: 1px dashed var(--color-e-nearwhite);
			outline-offset: -1px;
		}

		// Variant: dark outline over light colors
		&.preset-color.white,
		&.text-button.emphasized {
			&:focus-visible {
				outline: 1px dashed var(--color-2-mildblack);
			}
		}
	}

	// Checkbox needs to apply the focus outline to its sibling label
	.optional-input input:focus-visible + label,
	.checkbox-input input:focus-visible + label {
		outline: 1px dashed var(--color-e-nearwhite);
		outline-offset: -1px;
	}

	// Variant: dark outline over light colors (when the checkbox is checked)
	:not(.optional-input) > .checkbox-input input:focus-visible + label.checked {
		outline: 1px dashed var(--color-2-mildblack);
	}

	@font-face {
		font-family: "Source Sans Pro";
		font-weight: 400;
		font-style: normal;
		font-stretch: normal;
		src: url("@graphite/../node_modules/source-sans/WOFF2/TTF/SourceSansPro-Regular.ttf.woff2") format("woff2");
	}

	@font-face {
		font-family: "Source Sans Pro";
		font-weight: 400;
		font-style: italic;
		font-stretch: normal;
		src: url("@graphite/../node_modules/source-sans/WOFF2/TTF/SourceSansPro-It.ttf.woff2") format("woff2");
	}

	@font-face {
		font-family: "Source Sans Pro";
		font-weight: 700;
		font-style: normal;
		font-stretch: normal;
		src: url("@graphite/../node_modules/source-sans/WOFF2/TTF/SourceSansPro-Bold.ttf.woff2") format("woff2");
	}

	@font-face {
		font-family: "Source Sans Pro";
		font-weight: 700;
		font-style: italic;
		font-stretch: normal;
		src: url("@graphite/../node_modules/source-sans/WOFF2/TTF/SourceSansPro-BoldIt.ttf.woff2") format("woff2");
	}

	@font-face {
		font-family: "Source Code Pro";
		font-weight: 400;
		font-style: normal;
		font-stretch: normal;
		src: url("@graphite/../node_modules/source-code-pro/WOFF2/TTF/SourceCodePro-Regular.ttf.woff2") format("woff2");
	}
</style>
