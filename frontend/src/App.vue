<template>
	<MainWindow />
</template>

<style lang="scss">
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

	--color-accent: #3194d6;
	--color-accent-rgb: 49, 148, 214;
	--color-accent-hover: #49a5e2;
	--color-accent-hover-rgb: 73, 165, 226;
	--color-accent-disabled: #416277;
	--color-accent-disabled-rgb: 65, 98, 119;

	--color-node-background: #f1decd;
	--color-node-background-rgb: 241, 222, 205;
	--color-node-icon: #473a3a;
	--color-node-icon-rgb: 71, 58, 58;

	--color-data-general: #c5c5c5;
	--color-data-general-rgb: 197, 197, 197;
	--color-data-vector: #65bbe5;
	--color-data-vector-rgb: 101, 187, 229;
	--color-data-vector-dim: #4b778c;
	--color-data-vector-dim-rgb: 75, 119, 140;
	--color-data-raster: #e4bb72;
	--color-data-raster-rgb: 228, 187, 114;
	--color-data-raster-dim: #8b7752;
	--color-data-raster-dim-rgb: 139, 119, 82;
	--color-data-mask: #8d85c7;
	--color-data-mask-rgb: 141, 133, 199;
	--color-data-unused1: #d6536e;
	--color-data-unused1-rgb: 214, 83, 110;
	--color-data-unused2: #70a898;
	--color-data-unused2-rgb: 112, 168, 152;
}

html,
body,
#app {
	margin: 0;
	height: 100%;
	background: var(--color-2-mildblack);
	user-select: none;
	overscroll-behavior: none;
	outline: none;
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

::selection {
	background: var(--color-accent);
}

svg,
img {
	display: block;
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

.icon-button,
.text-button,
.popover-button,
.checkbox-input label,
.color-input .swatch .swatch-button,
.dropdown-input .dropdown-box,
.font-input .dropdown-box,
.radio-input button,
.menu-list,
.menu-bar-input .entry {
	&:focus-visible,
	&.dropdown-box:focus {
		outline: 1px dashed var(--color-accent);
		outline-offset: -1px;
	}
}

// For placeholder messages (remove eventually)
.floating-menu {
	h1,
	h2,
	h3,
	h4,
	h5,
	h6,
	p {
		margin: 0;
	}

	p {
		margin-top: 8px;
	}

	.floating-menu-content h3 ~ p {
		white-space: pre-wrap;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { createBlobManager } from "@/io-managers/blob";
import { createClipboardManager } from "@/io-managers/clipboard";
import { createHyperlinkManager } from "@/io-managers/hyperlinks";
import { createInputManager } from "@/io-managers/input";
import { createLocalizationManager } from "@/io-managers/localization";
import { createPanicManager } from "@/io-managers/panic";
import { createPersistenceManager } from "@/io-managers/persistence";
import { createDialogState, type DialogState } from "@/state-providers/dialog";
import { createFontsState, type FontsState } from "@/state-providers/fonts";
import { createFullscreenState, type FullscreenState } from "@/state-providers/fullscreen";
import { createPanelsState, type PanelsState } from "@/state-providers/panels";
import { createPortfolioState, type PortfolioState } from "@/state-providers/portfolio";
import { createWorkspaceState, type WorkspaceState } from "@/state-providers/workspace";
import { operatingSystem } from "@/utility-functions/platform";
import { createEditor, type Editor } from "@/wasm-communication/editor";

import MainWindow from "@/components/window/MainWindow.vue";

const managerDestructors: {
	createBlobManager?: () => void;
	createClipboardManager?: () => void;
	createHyperlinkManager?: () => void;
	createInputManager?: () => void;
	createLocalizationManager?: () => void;
	createPanicManager?: () => void;
	createPersistenceManager?: () => void;
} = {};

// Vue injects don't play well with TypeScript (all injects will show up as `any`) but we can define these types as a solution
declare module "@vue/runtime-core" {
	// Systems `provide`d by the root App to be `inject`ed into descendant components and used for reactive bindings
	// eslint-disable-next-line @typescript-eslint/consistent-type-definitions
	interface ComponentCustomProperties {
		// Graphite WASM editor instance
		editor: Editor;

		// State provider systems
		dialog: DialogState;
		fonts: FontsState;
		fullscreen: FullscreenState;
		panels: PanelsState;
		portfolio: PortfolioState;
		workspace: WorkspaceState;
	}
}

export default defineComponent({
	provide() {
		return { ...this.$data };
	},
	data() {
		const editor = createEditor();
		return {
			// Graphite WASM editor instance
			editor,

			// State provider systems
			dialog: createDialogState(editor),
			fonts: createFontsState(editor),
			fullscreen: createFullscreenState(editor),
			panels: createPanelsState(editor),
			portfolio: createPortfolioState(editor),
			workspace: createWorkspaceState(editor),
		};
	},
	async mounted() {
		// Initialize managers, which are isolated systems that subscribe to backend messages to link them to browser API functionality (like JS events, IndexedDB, etc.)
		Object.assign(managerDestructors, {
			createBlobManager: createBlobManager(this.editor),
			createClipboardManager: createClipboardManager(this.editor),
			createHyperlinkManager: createHyperlinkManager(this.editor),
			createInputManager: createInputManager(this.editor, this.$el.parentElement, this.dialog, this.portfolio, this.fullscreen),
			createLocalizationManager: createLocalizationManager(this.editor),
			createPanicManager: createPanicManager(this.editor, this.dialog),
			createPersistenceManager: await createPersistenceManager(this.editor, this.portfolio),
		});

		// Initialize certain setup tasks required by the editor backend to be ready for the user now that the frontend is ready
		const platform = operatingSystem();
		this.editor.instance.initAfterFrontendReady(platform);
	},
	beforeUnmount() {
		// Call the destructor for each manager
		Object.values(managerDestructors).forEach((destructor) => destructor?.());

		// Destroy the WASM editor instance
		this.editor.instance.free();
	},
	components: { MainWindow },
});
</script>
