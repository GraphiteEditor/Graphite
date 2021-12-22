<template>
	<div class="user-input-label">
		<template v-for="(keyGroup, keyGroupIndex) in inputKeys" :key="keyGroupIndex">
			<span class="group-gap" v-if="keyGroupIndex > 0"></span>
			<span class="input-key" v-for="inputKey in keyGroup" :key="inputKey" :class="keyCapWidth(inputKey)">
				{{ keyText(inputKey) }}
			</span>
		</template>
		<span class="input-mouse" v-if="inputMouse">
			<IconLabel :icon="mouseInputInteractionToIcon(inputMouse)" />
		</span>
		<span class="hint-text" v-if="hasSlotContent">
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

	.group-gap {
		width: 4px;
	}

	.input-key,
	.input-mouse {
		& + .input-key,
		& + .input-mouse {
			margin-left: 2px;
		}
	}

	.input-key {
		font-family: "Inconsolata", monospace;
		font-weight: 400;
		text-align: center;
		color: var(--color-e-nearwhite);
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
	}

	.hint-text {
		margin-left: 4px;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export enum MouseInputInteraction {
	"None" = "None",
	"Lmb" = "Lmb",
	"Rmb" = "Rmb",
	"Mmb" = "Mmb",
	"ScrollUp" = "ScrollUp",
	"ScrollDown" = "ScrollDown",
	"Drag" = "Drag",
	"LmbDrag" = "LmbDrag",
	"RmbDrag" = "RmbDrag",
	"MmbDrag" = "MmbDrag",
}

export default defineComponent({
	components: { IconLabel },
	props: {
		inputKeys: { type: Array, default: () => [] },
		inputMouse: { type: String },
	},
	computed: {
		hasSlotContent(): boolean {
			return Boolean(this.$slots.default);
		},
	},
	methods: {
		keyCapWidth(keyText: string): string {
			const text = this.keyText(keyText) || " ";
			return `width-${text.length * 8 + 8}`;
		},
		keyText(keyText: string): string {
			// Strip off the "Key" prefix
			const text = keyText.replace(/^(?:Key)?(.*)$/, "$1");

			if (/^[A-Z0-9]$/.test(text)) return text;

			const textMap: Record<string, string> = {
				Shift: "⇧",
				Control: "Ctrl",
				Alt: "Alt",
				ArrowUp: "↑",
				ArrowRight: "→",
				ArrowDown: "↓",
				ArrowLeft: "←",
				Tab: "↹",
				Backspace: "Bksp",
				Delete: "Del",
				Option: "⌥",
				Command: "⌘",
				Enter: "↵",
				PageUp: "PgUp",
				PageDown: "PgDn",
			};
			if (text in textMap) return textMap[text];

			return text;
		},
		mouseInputInteractionToIcon(mouseInputInteraction: MouseInputInteraction) {
			switch (mouseInputInteraction) {
				case MouseInputInteraction.Lmb:
					return "MouseHintLmb";
				case MouseInputInteraction.Rmb:
					return "MouseHintRmb";
				case MouseInputInteraction.Mmb:
					return "MouseHintMmb";
				case MouseInputInteraction.ScrollUp:
					return "MouseHintScrollUp";
				case MouseInputInteraction.ScrollDown:
					return "MouseHintScrollDown";
				case MouseInputInteraction.Drag:
					return "MouseHintDrag";
				case MouseInputInteraction.LmbDrag:
					return "MouseHintLmbDrag";
				case MouseInputInteraction.RmbDrag:
					return "MouseHintRmbDrag";
				case MouseInputInteraction.MmbDrag:
					return "MouseHintMmbDrag";
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
