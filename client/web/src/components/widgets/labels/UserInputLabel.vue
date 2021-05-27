<template>
	<div class="user-input-label">
		<span class="input-key" v-for="inputKey in inputKeys" :key="inputKey" :class="keyCapWidth(inputKey)">
			{{ inputKey }}
		</span>
		<span class="input-mouse" v-if="inputMouse">
			<Icon :icon="mouseInputInteractionToIcon(inputMouse)" />
		</span>
		<span class="hint-text">
			<slot></slot>
		</span>
	</div>
</template>

<style lang="scss">
.user-input-label {
	height: 100%;
	margin: 0 8px;
	display: flex;
	align-items: center;
	white-space: nowrap;

	.input-key,
	.input-mouse {
		margin-right: 4px;
	}

	.input-key {
		font-family: "Inconsolata", monospace;
		font-weight: bold;
		text-align: center;
		color: var(--color-2-mildblack);
		background: var(--color-e-nearwhite);
		border: 1px;
		box-sizing: border-box;
		border-style: solid;
		border-color: var(--color-7-middlegray);
		border-radius: 4px;
		height: 16px;
		line-height: 16px;
	}

	.input-key.width-16 {
		width: 16px;
	}

	.input-key.width-24 {
		width: 24px;
	}

	.input-key.width-32 {
		width: 32px;
	}

	.input-key.width-40 {
		width: 40px;
	}

	.input-key.width-48 {
		width: 48px;
	}

	.input-mouse {
		.bright {
			fill: var(--color-e-nearwhite);
		}

		.dim {
			fill: var(--color-7-middlegray);
		}

		svg {
			vertical-align: top;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Icon from "./Icon.vue";

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

export default defineComponent({
	components: { Icon },
	props: {
		inputKeys: { type: Array, default: () => [] },
		inputMouse: { type: String },
	},
	methods: {
		keyCapWidth(keyText: string) {
			return `width-${keyText.length * 8 + 8}`;
		},
		mouseInputInteractionToIcon(mouseInputInteraction: MouseInputInteraction) {
			switch (mouseInputInteraction) {
				case MouseInputInteraction.LMB:
					return "MouseHintLMB";
				case MouseInputInteraction.RMB:
					return "MouseHintRMB";
				case MouseInputInteraction.MMB:
					return "MouseHintMMB";
				case MouseInputInteraction.ScrollUp:
					return "MouseHintScrollUp";
				case MouseInputInteraction.ScrollDown:
					return "MouseHintScrollDown";
				case MouseInputInteraction.Drag:
					return "MouseHintDrag";
				case MouseInputInteraction.LMBDrag:
					return "MouseHintLMBDrag";
				case MouseInputInteraction.RMBDrag:
					return "MouseHintRMBDrag";
				case MouseInputInteraction.MMBDrag:
					return "MouseHintMMBDrag";
				default:
				case MouseInputInteraction.None:
					return "MouseHintNone";
			}
		},
	},
	data() {
		return {
			MouseInputInteraction,
		};
	},
});
</script>
