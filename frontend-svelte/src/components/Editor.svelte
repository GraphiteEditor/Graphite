<script lang="ts">
	import { onMount, onDestroy, setContext } from "svelte";

	import type { createEditor } from "@/wasm-communication/editor";
	import { operatingSystem } from "@/utility-functions/platform";
	import { createClipboardManager } from "@/io-managers/clipboard";
	import { createDragManager } from "@/io-managers/drag";
	import { createHyperlinkManager } from "@/io-managers/hyperlinks";
	import { createInputManager } from "@/io-managers/input";
	import { createLocalizationManager } from "@/io-managers/localization";
	import { createPanicManager } from "@/io-managers/panic";
	import { createPersistenceManager } from "@/io-managers/persistence";
	import { createDialogState } from "@/state-providers/dialog";
	import { createDocumentState } from "@/state-providers/document";
	import { createFontsState } from "@/state-providers/fonts";
	import { createFullscreenState } from "@/state-providers/fullscreen";
	import { createNodeGraphState } from "@/state-providers/node-graph";
	import { createPortfolioState } from "@/state-providers/portfolio";
	import { createWorkspaceState } from "@/state-providers/workspace";

	import MainWindow from "@/components/window/MainWindow.svelte";

	// Graphite WASM editor instance
	export let editor: ReturnType<typeof createEditor>;
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
	let workspace = createWorkspaceState(editor);
	setContext("workspace", workspace);

	// Initialize managers, which are isolated systems that subscribe to backend messages to link them to browser API functionality (like JS events, IndexedDB, etc.)
	createClipboardManager(editor);
	createHyperlinkManager(editor);
	createLocalizationManager(editor);
	createPanicManager(editor, dialog);
	createPersistenceManager(editor, portfolio);
	let dragManagerDestructor = createDragManager();
	let inputManagerDestructor = createInputManager(editor, dialog, portfolio, fullscreen);

	onMount(() => {
		// Initialize certain setup tasks required by the editor backend to be ready for the user now that the frontend is ready
		editor.instance.initAfterFrontendReady(operatingSystem());
	});

	onDestroy(() => {
		// Call the destructor for each manager
		dragManagerDestructor();
		inputManagerDestructor();
	});
</script>

<MainWindow />

<style lang="scss" global>
	// Disable the spinning loading indicator
	body::after {
		content: none !important;
	}

	:root {
		// Replace usage of `-rgb` variants with CSS color() function to calculate alpha when browsers support it
		// See https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/color() and https://caniuse.com/css-color-function
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

		--color-data-general: #c5c5c5;
		--color-data-general-dim: #767676;
		--color-data-vector: #65bbe5;
		--color-data-vector-dim: #4b778c;
		--color-data-raster: #e4bb72;
		--color-data-raster-dim: #8b7752;
		--color-data-mask: #8d85c7;
		--color-data-number: #d6536e;
		--color-data-number-dim: #803242;
		--color-data-vec2: #cc00ff;
		--color-data-vec2-dim: #71008d;
		--color-data-color: #70a898;
		--color-data-color-dim: #43645b;

		--color-none: white;
		--color-none-repeat: no-repeat;
		--color-none-position: center center;
		// 24px tall, 48px wide
		--color-none-size-24px: 60px 24px;
		--color-none-image-24px: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 24"><line stroke="red" stroke-width="4px" x1="0" y1="27" x2="60" y2="-3" /></svg>');
		// 32px tall, 64px wide
		--color-none-size-32px: 80px 32px;
		--color-none-image-32px: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 32"><line stroke="red" stroke-width="4px" x1="0" y1="36" x2="80" y2="-4" /></svg>');

		--color-transparent-checkered-background: linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%),
			linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%), linear-gradient(#ffffff, #ffffff);
		--color-transparent-checkered-background-size: 16px 16px;
		--color-transparent-checkered-background-position: 0 0, 8px 8px;

		--icon-expand-collapse-arrow: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><polygon fill="%23eee" points="3,0 1,0 5,4 1,8 3,8 7,4" /></svg>');
		--icon-expand-collapse-arrow-hover: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><polygon fill="%23fff" points="3,0 1,0 5,4 1,8 3,8 7,4" /></svg>');
	}

	html,
	body {
		margin: 0;
		height: 100%;
		background: var(--color-2-mildblack);
		overscroll-behavior: none;
		-webkit-user-select: none; // Required as of Safari 15.0 (Graphite's minimum version) through the latest release
		user-select: none;
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

	.sharp-right-corners.sharp-right-corners.sharp-right-corners.sharp-right-corners {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
	}

	.layout-row,
	.layout-col {
		.scrollable-x,
		.scrollable-y {
			// Firefox (standardized in CSS, but less capable)
			scrollbar-width: thin;
			scrollbar-width: 6px;
			scrollbar-gutter: 6px;
			scrollbar-color: var(--color-5-dullgray) transparent;

			&:not(:hover) {
				scrollbar-width: none;
			}

			// WebKit (only in Chromium/Safari but more capable)
			&::-webkit-scrollbar {
				width: calc(2px + 6px + 2px);
				height: calc(2px + 6px + 2px);
			}

			&:not(:hover)::-webkit-scrollbar {
				width: 0;
				height: 0;
			}

			&::-webkit-scrollbar-track {
				box-shadow: inset 0 0 0 1px var(--color-5-dullgray);
				border: 2px solid transparent;
				border-radius: 10px;

				&:hover {
					box-shadow: inset 0 0 0 1px var(--color-6-lowergray);
				}
			}

			&::-webkit-scrollbar-thumb {
				background-clip: padding-box;
				background-color: var(--color-5-dullgray);
				border: 2px solid transparent;
				border-radius: 10px;
				margin: 2px;

				&:hover {
					background-color: var(--color-6-lowergray);
				}
			}

			&::-webkit-scrollbar-corner {
				background: none;
			}
		}

		.scrollable-x.scrollable-y {
			// Standard
			overflow: auto;
			// WebKit
			overflow: overlay;
		}

		.scrollable-x:not(.scrollable-y) {
			// Standard
			overflow: auto hidden;
			// WebKit
			overflow-x: overlay;
		}

		.scrollable-y:not(.scrollable-x) {
			// Standard
			overflow: hidden auto;
			// WebKit
			overflow-y: overlay;
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
	.color-input > button,
	.color-picker .preset-color,
	.swatch-pair .swatch > button,
	.radio-input button,
	.menu-list,
	.menu-bar-input .entry,
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
	.checkbox-input input:focus-visible + label {
		outline: 1px dashed var(--color-e-nearwhite);
		outline-offset: -1px;

		// Variant: dark outline over light colors
		&.checked {
			outline: 1px dashed var(--color-2-mildblack);
		}
	}
</style>
