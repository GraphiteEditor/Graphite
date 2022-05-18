<template>
	<MainWindow />

	<div class="unsupported-modal-backdrop" v-if="apiUnsupported" ref="unsupported">
		<LayoutCol class="unsupported-modal">
			<h2>Your browser currently doesn't support Graphite</h2>
			<p>Unfortunately, some features won't work properly. Please upgrade to a modern browser such as Firefox, Chrome, Edge, or Safari version 15 or later.</p>
			<p>
				Your browser is missing support for the
				<a href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt64Array#browser_compatibility" target="_blank"><code>BigInt64Array</code></a> JavaScript
				API which is required for using the editor. However, you can still explore the user interface.
			</p>
			<LayoutRow>
				<button class="unsupported-modal-button" @click="() => closeUnsupportedWarning()">I understand, let's just see the interface</button>
			</LayoutRow>
		</LayoutCol>
	</div>
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
}

.unsupported-modal-backdrop {
	background: rgba(255, 255, 255, 0.6);
	position: absolute;
	top: 0;
	left: 0;
	bottom: 0;
	right: 0;
	display: flex;
	align-items: center;
	justify-content: center;

	.unsupported-modal {
		background: var(--color-3-darkgray);
		border-radius: 4px;
		box-shadow: 2px 2px 5px 0 rgba(var(--color-0-black-rgb), 50%);
		padding: 0 16px 16px 16px;
		border: 1px solid var(--color-4-dimgray);
		max-width: 500px;

		p {
			margin-top: 0;
		}

		a {
			color: var(--color-accent-hover);
		}

		.unsupported-modal-button {
			flex: 1;
			background: var(--color-1-nearblack);
			border: 0 none;
			padding: 12px;
			border-radius: 2px;

			&:hover {
				background: var(--color-6-lowergray);
				color: var(--color-f-white);
			}

			&:active {
				background: var(--color-accent-hover);
				color: var(--color-f-white);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { createEditor, Editor } from "@/interop/editor";
import { createAutoSaveManager } from "@/managers/auto-save";
import { createInputManager } from "@/managers/input";
import { createPanicManager } from "@/managers/panic";
import { createDialogState, DialogState } from "@/providers/dialog";
import { createFontsState, FontsState } from "@/providers/fonts";
import { createFullscreenState, FullscreenState } from "@/providers/fullscreen";
import { createPortfolioState, PortfolioState } from "@/providers/portfolio";
import { createWorkspaceState, WorkspaceState } from "@/providers/workspace";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import MainWindow from "@/components/window/MainWindow.vue";

// Vue injects don't play well with TypeScript (all injects will show up as `any`) but we can define these types as a solution
declare module "@vue/runtime-core" {
	interface ComponentCustomProperties {
		// Graphite WASM editor instance
		editor: Editor;

		// Stateful systems which are `provide`d by this Vue component to be `inject`ed by descendant components and used for reactive bindings
		dialog: DialogState;
		fonts: FontsState;
		fullscreen: FullscreenState;
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

			// Stateful systems which are `provide`d by this Vue component to be `inject`ed by descendant components and used for reactive bindings
			dialog: createDialogState(editor),
			fonts: createFontsState(editor),
			fullscreen: createFullscreenState(),
			portfolio: createPortfolioState(editor),
			workspace: createWorkspaceState(editor),
		};
	},
	computed: {
		apiUnsupported() {
			return !("BigInt64Array" in window);
		},
	},
	methods: {
		closeUnsupportedWarning() {
			const element = this.$refs.unsupported as HTMLElement;
			element.parentElement?.removeChild(element);
		},
	},
	mounted() {
		// Initialize managers, which are isolated systems that subscribe to backend messages to link them to browser API functionality (like JS events, IndexedDB, etc.)
		createAutoSaveManager(this.editor, this.portfolio);
		createInputManager(this.editor, this.$el.parentElement, this.dialog, this.portfolio, this.fullscreen);
		createPanicManager(this.editor, this.dialog);

		// Initialize certain setup tasks required by the editor backend to be ready for the user now that the frontend is ready
		this.editor.instance.init_app();
	},
	beforeUnmount() {
		this.editor.instance.free();
	},
	components: {
		MainWindow,
		LayoutRow,
		LayoutCol,
	},
});
</script>
