<template>
	<MainWindow />
	<div class="unsupported-modal-backdrop" v-if="showUnsupportedModal">
		<div class="unsupported-modal">
			<h2>Graphite is not supported by your browser</h2>
			<p>
				Unfortunately your browser does not support the BigInt64Array API. To be able to use Graphite please use a supported browser such as Mozilla Firefox, Google Chrome or Microsoft Edge.
			</p>
			<LayoutRow> <button class="unsupported-modal-button" @click="closeModal()">I understand. See the interface anyways</button> </LayoutRow>
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
	--floating-menu-opacity-color-2-mildblack: #222222e6;
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
	width: 100vw;
	height: 100vh;
	align-items: center;
	justify-content: center;
	display: flex;
}
.unsupported-modal {
	background: var(--color-3-darkgray);
	border-radius: 5px;
	box-shadow: 2px 2px 5px 0px var(--floating-menu-shadow);
	padding: 0rem 1rem 1rem 1rem;
	border: 1px solid var(--color-4-dimgray);
	max-width: 500px;
}
.unsupported-modal-button {
	flex: 1;
	background: var(--color-1-nearblack);
	border: 0px none;
	padding: 0.5rem;
	border-radius: 2px;

	&:hover {
		background-color: var(--color-6-lowergray);
	}

	&:active {
		background-color: var(--color-accent-hover);
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
