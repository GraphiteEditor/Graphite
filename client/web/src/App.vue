<template>
	<MainWindow />
	<div class="unsupported-modal-backdrop" v-if="showUnsupportedModal">
		<div class="unsupported-modal">
			<h2>Your browser currently doesn't support Graphite</h2>
			<p>
				Unfortunately, some features won't work properly in your browser. Please use a modern browser other than Safari, such as Firefox, Chrome, or Edge. Rest assured, Safari compatibility is
				planned.
			</p>
			<p>
				Your browser is missing support for the
				<a href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt64Array#browser_compatibility" target="_blank"><code>BigInt64Array</code></a> JavaScript
				API which is required for using the editor. You can still explore the user interface.
			</p>
			<LayoutRow> <button class="unsupported-modal-button" @click="closeModal()">I understand, let's just see the interface</button> </LayoutRow>
		</div>
	</div>
</template>

<style lang="scss">
:root {
	--color-0-black: #000;
	--color-1-nearblack: #111;
	--color-2-mildblack: #222;
	--color-3-darkgray: #333;
	--color-4-dimgray: #444;
	--color-5-dullgray: #555;
	--color-6-lowergray: #666;
	--color-7-middlegray: #777;
	--color-8-uppergray: #888;
	--color-9-palegray: #999;
	--color-a-softgray: #aaa;
	--color-b-lightgray: #bbb;
	--color-c-brightgray: #ccc;
	--color-d-mildwhite: #ddd;
	--color-e-nearwhite: #eee;
	--color-f-white: #fff;
	--color-accent: #3194d6;
	--color-accent-hover: #49a5e2;

	// TODO: Replace with CSS color() function to calculate alpha when browsers support it
	// See https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/color() and https://caniuse.com/css-color-function
	// E6 = 90% alpha
	--floating-menu-opacity-color-2-mildblack: #222222f2;
	--floating-menu-shadow: rgba(0, 0, 0, 50%);
}

html,
body,
#app {
	margin: 0;
	height: 100%;
	background: var(--color-2-mildblack);
	user-select: none;
}

body,
input,
textarea,
button {
	font-family: "Source Sans Pro", Arial, sans-serif;
	font-size: 14px;
	line-height: 1;
	color: var(--color-e-nearwhite);
}

svg,
img {
	display: block;
}

.scrollable,
.scrollable-x,
.scrollable-y {
	// Standard
	scrollbar-width: thin;
	scrollbar-width: 6px;
	scrollbar-gutter: 6px;
	scrollbar-color: var(--color-5-dullgray) transparent;

	&:not(:hover) {
		scrollbar-width: none;
	}

	// WebKit
	&::-webkit-scrollbar {
		width: calc(2px + 6px + 2px);
	}

	&:not(:hover)::-webkit-scrollbar {
		width: 0;
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
}

.scrollable {
	// Standard
	overflow: auto;
	// WebKit
	overflow: overlay;
}

.scrollable-x {
	// Standard
	overflow-x: auto;
	// WebKit
	overflow-x: overlay;
}

.scrollable-y {
	// Standard
	overflow-y: auto;
	// WebKit
	overflow-y: overlay;
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
	align-items: center;
	justify-content: center;
	display: flex;
}
.unsupported-modal {
	background: var(--color-3-darkgray);
	border-radius: 4px;
	box-shadow: 2px 2px 5px 0 var(--floating-menu-shadow);
	padding: 0 16px 16px 16px;
	border: 1px solid var(--color-4-dimgray);
	max-width: 500px;

	& a {
		color: var(--color-accent-hover);
	}
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
</style>

<script lang="ts">
import { defineComponent } from "vue";
import MainWindow from "./components/window/MainWindow.vue";
import LayoutRow from "./components/layout/LayoutRow.vue";

export default defineComponent({
	data() {
		return {
			showUnsupportedModal: !("BigInt64Array" in window),
		};
	},
	methods: {
		closeModal() {
			this.showUnsupportedModal = false;
		},
	},
	components: { MainWindow, LayoutRow },
});
</script>
