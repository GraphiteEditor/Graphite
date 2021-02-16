<template>
	<div class="input-hint">
		<span class="input-key" v-for="inputKey in inputKeys" :key="inputKey">
			{{inputKey}}
		</span>
		<span class="input-mouse" v-if="inputMouse">
			<svg width="16" height="16" viewBox="0 0 16 16" v-html="getMouseIconInnerSVG"></svg>
		</span>
		<span class="hint-text">
			<slot></slot>
		</span>
	</div>
</template>

<style lang="scss">
.input-hint {
	height: 100%;
	margin: 0 8px;
	display: flex;
	align-items: center;
	white-space: nowrap;

	.input-key, .input-mouse {
		margin-right: 4px;
	}

	.input-key {
		font-family: "Consolas", monospace;
		font-weight: bold;
		text-align: center;
		color: #000;
		background: #fff;
		border: 2px;
		border-style: solid;
		border-color: #999;
		border-radius: 4px;
		width: 14px;
		height: 14px;
		line-height: 14px;
	}

	.input-mouse {
		font-size: 0;

		.primary {
			fill: #fff;
		}

		.secondary {
			fill: #888;
		}
	}
}
</style>

<script lang="ts">
import { Options, Vue } from "vue-class-component";

export enum MouseInputInteraction {
	"None" = "None",
	"LMB" = "LMB",
	"RMB" = "RMB",
	"MMB" = "MMB",
	"ScrollUp" = "ScrollUp",
	"ScrollDown" = "ScrollDown",
	"Drag" = "Drag",
	"LMBDrag" = "LMBDrag",
	"RMBDrag" = "RMBDrag",
	"MMBDrag" = "MMBDrag",
}

@Options({
	components: {},
	props: {
		inputKeys: { type: Array, default: [] },
		inputMouse: { type: String },
	},
	computed: {
		getMouseIconInnerSVG() {
			switch (this.inputMouse) {
			case MouseInputInteraction.None: return `
				<path style="fill:#888888;" d="M9,7c0,0.55-0.45,1-1,1l0,0C7.45,8,7,7.55,7,7V4.5c0-0.55,0.45-1,1-1l0,0c0.55,0,1,0.45,1,1V7z" />
				<path style="fill:#888888;" d="M10,2c1.1,0,2,0.9,2,2v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V4c0-1.1,0.9-2,2-2H10 M10,1H6
					C4.35,1,3,2.35,3,4v6c0,2.76,2.24,5,5,5s5-2.24,5-5V4C13,2.35,11.65,1,10,1L10,1z" />`;
			case MouseInputInteraction.LMB: return `
				<path style="fill:#FFFFFF;" d="M8,1H6C4.35,1,3,2.35,3,4v4h5V1z" />
				<path style="fill:#888888;" d="M10,1H9v1h1c1.1,0,2,0.9,2,2v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V9H3v1c0,2.76,2.24,5,5,5s5-2.24,5-5V4
					C13,2.35,11.65,1,10,1z" />`;
			case MouseInputInteraction.RMB: return `
				<path class="secondary" d="M8,1h2c1.65,0,3,1.35,3,3v4H8V1z" />
				<path class="primary" d="M6,1h1v1H6C4.9,2,4,2.9,4,4v6c0,2.21,1.79,4,4,4s4-1.79,4-4V9h1v1c0,2.76-2.24,5-5,5s-5-2.24-5-5V4
				C3,2.35,4.35,1,6,1z" />`;
			case MouseInputInteraction.MMB: return `
				<path style="fill:#FFFFFF;" d="M9,7c0,0.55-0.45,1-1,1l0,0C7.45,8,7,7.55,7,7V4.5c0-0.55,0.45-1,1-1l0,0c0.55,0,1,0.45,1,1V7z" />
				<path style="fill:#888888;" d="M10,2c1.1,0,2,0.9,2,2v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V4c0-1.1,0.9-2,2-2H10 M10,1H6
					C4.35,1,3,2.35,3,4v6c0,2.76,2.24,5,5,5s5-2.24,5-5V4C13,2.35,11.65,1,10,1L10,1z" />`;
			case MouseInputInteraction.ScrollUp: return `
				<polygon style="fill:#FFFFFF;" points="10.5,4 8,2 5.5,4 5.5,2 8,0 10.5,2 " />
				<polygon style="fill:#FFFFFF;" points="10.5,8 8,6 5.5,8 5.5,6 8,4 10.5,6 " />
				<path style="fill:#888888;" d="M11.5,1.42v1.28C11.8,3.06,12,3.5,12,4v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V4c0-0.5,0.2-0.94,0.5-1.29
					V1.42C3.61,1.94,3,2.9,3,4v6c0,2.76,2.24,5,5,5s5-2.24,5-5V4C13,2.9,12.39,1.94,11.5,1.42z" />`;
			case MouseInputInteraction.ScrollDown: return `
				<polygon style="fill:#FFFFFF;" points="5.5,4 8,6 10.5,4 10.5,6 8,8 5.5,6 " />
				<polygon style="fill:#FFFFFF;" points="5.5,0 8,2 10.5,0 10.5,2 8,4 5.5,2 " />
				<path style="fill:#888888;" d="M11.5,1.42v1.28C11.8,3.06,12,3.5,12,4v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V4c0-0.5,0.2-0.94,0.5-1.29
					V1.42C3.61,1.94,3,2.9,3,4v6c0,2.76,2.24,5,5,5s5-2.24,5-5V4C13,2.9,12.39,1.94,11.5,1.42z" />`;
			case MouseInputInteraction.Drag: return `
				<path style="fill:#888888;" d="M8,7c0,0.55-0.45,1-1,1l0,0C6.45,8,6,7.55,6,7V4.5c0-0.55,0.45-1,1-1l0,0c0.55,0,1,0.45,1,1V7z" />
				<path style="fill:#FFFFFF;" d="M11,16c-0.18,0-0.36-0.1-0.45-0.28c-0.12-0.25-0.02-0.55,0.22-0.67C10.87,15.01,13,13.88,13,11V6
					c0-0.28,0.22-0.5,0.5-0.5S14,5.72,14,6v5c0,3.52-2.66,4.89-2.78,4.95C11.15,15.98,11.08,16,11,16z" />
				<path style="fill:#FFFFFF;" d="M14.5,15c-0.13,0-0.26-0.05-0.35-0.15c-0.19-0.19-0.2-0.51,0-0.7C14.17,14.12,15,13.2,15,11V8
					c0-0.28,0.22-0.5,0.5-0.5S16,7.72,16,8v3c0,2.68-1.1,3.81-1.15,3.85C14.76,14.95,14.63,15,14.5,15z" />
				<path style="fill:#888888;" d="M9,2c1.1,0,2,0.9,2,2v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V4c0-1.1,0.9-2,2-2H9 M9,1H5C3.35,1,2,2.35,2,4
					v6c0,2.76,2.24,5,5,5s5-2.24,5-5V4C12,2.35,10.65,1,9,1L9,1z" />`;
			case MouseInputInteraction.LMBDrag: return `
				<path style="fill:#FFFFFF;" d="M11,16c-0.18,0-0.36-0.1-0.45-0.28c-0.12-0.25-0.02-0.55,0.22-0.67C10.87,15.01,13,13.88,13,11V6
					c0-0.28,0.22-0.5,0.5-0.5S14,5.72,14,6v5c0,3.52-2.66,4.89-2.78,4.95C11.15,15.98,11.08,16,11,16z" />
				<path style="fill:#FFFFFF;" d="M14.5,15c-0.13,0-0.26-0.05-0.35-0.15c-0.19-0.19-0.2-0.51,0-0.7C14.17,14.12,15,13.2,15,11V8
					c0-0.28,0.22-0.5,0.5-0.5S16,7.72,16,8v3c0,2.68-1.1,3.81-1.15,3.85C14.76,14.95,14.63,15,14.5,15z" />
				<path style="fill:#FFFFFF;" d="M7,1H5C3.35,1,2,2.35,2,4v4h5V1z" />
				<path style="fill:#888888;" d="M9,1H8v1h1c1.1,0,2,0.9,2,2v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V9H2v1c0,2.76,2.24,5,5,5s5-2.24,5-5V4
					C12,2.35,10.65,1,9,1z" />`;
			case MouseInputInteraction.RMBDrag: return `
				<path style="fill:#FFFFFF;" d="M11,16c-0.18,0-0.36-0.1-0.45-0.28c-0.12-0.25-0.02-0.55,0.22-0.67C10.87,15.01,13,13.88,13,11V6
					c0-0.28,0.22-0.5,0.5-0.5S14,5.72,14,6v5c0,3.52-2.66,4.89-2.78,4.95C11.15,15.98,11.08,16,11,16z" />
				<path style="fill:#FFFFFF;" d="M14.5,15c-0.13,0-0.26-0.05-0.35-0.15c-0.19-0.19-0.2-0.51,0-0.7C14.17,14.12,15,13.2,15,11V8
					c0-0.28,0.22-0.5,0.5-0.5S16,7.72,16,8v3c0,2.68-1.1,3.81-1.15,3.85C14.76,14.95,14.63,15,14.5,15z" />
				<path style="fill:#FFFFFF;" d="M7,1h2c1.65,0,3,1.35,3,3v4H7V1z" />
				<path style="fill:#888888;" d="M5,1h1v1H5C3.9,2,3,2.9,3,4v6c0,2.21,1.79,4,4,4s4-1.79,4-4V9h1v1c0,2.76-2.24,5-5,5s-5-2.24-5-5V4
					C2,2.35,3.35,1,5,1z" />`;
			case MouseInputInteraction.MMBDrag: return `
				<path style="fill:#FFFFFF;" d="M8,7c0,0.55-0.45,1-1,1l0,0C6.45,8,6,7.55,6,7V4.5c0-0.55,0.45-1,1-1l0,0c0.55,0,1,0.45,1,1V7z" />
				<path style="fill:#FFFFFF;" d="M11,16c-0.18,0-0.36-0.1-0.45-0.28c-0.12-0.25-0.02-0.55,0.22-0.67C10.87,15.01,13,13.88,13,11V6
					c0-0.28,0.22-0.5,0.5-0.5S14,5.72,14,6v5c0,3.52-2.66,4.89-2.78,4.95C11.15,15.98,11.08,16,11,16z" />
				<path style="fill:#FFFFFF;" d="M14.5,15c-0.13,0-0.26-0.05-0.35-0.15c-0.19-0.19-0.2-0.51,0-0.7C14.17,14.12,15,13.2,15,11V8
					c0-0.28,0.22-0.5,0.5-0.5S16,7.72,16,8v3c0,2.68-1.1,3.81-1.15,3.85C14.76,14.95,14.63,15,14.5,15z" />
				<path style="fill:#888888;" d="M9,2c1.1,0,2,0.9,2,2v6c0,2.21-1.79,4-4,4s-4-1.79-4-4V4c0-1.1,0.9-2,2-2H9 M9,1H5C3.35,1,2,2.35,2,4
					v6c0,2.76,2.24,5,5,5s5-2.24,5-5V4C12,2.35,10.65,1,9,1L9,1z" />`;
			default: return "";
			}
		},
	},
})
export default class InputHint extends Vue {}
</script>
