<template>
	<div class="user-input-label">
		<template v-for="(keyGroup, keyGroupIndex) in inputKeys" :key="keyGroupIndex">
			<span class="group-gap" v-if="keyGroupIndex > 0"></span>
			<template v-for="inputKey in keyGroup" :key="((keyInfo = keyTextOrIcon(inputKey)), inputKey)">
				<span class="input-key" :class="keyInfo.width">
					<IconLabel v-if="keyInfo.icon" :icon="keyInfo.icon" />
					<template v-else>{{ keyInfo.text }}</template>
				</span>
			</template>
		</template>
		<span class="input-mouse" v-if="inputMouse">
			<IconLabel :icon="mouseMovementIcon(inputMouse)" />
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
		// Firefox renders the text 1px lower than Chrome (tested on Windows) with 16px line-height, so moving it up 1 pixel with 15px makes them agree
		line-height: 15px;

		&.width-16 {
			width: 16px;
		}

		&.width-24 {
			width: 24px;
		}

		&.width-32 {
			width: 32px;
		}

		&.width-40 {
			width: 40px;
		}

		&.width-48 {
			width: 48px;
		}

		.icon-label {
			margin: 1px;
			display: inline-block;
		}
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
		keyTextOrIcon(keyText: string): { text: string | null; icon: string | null; width: string } {
			// Definitions
			const textMap: Record<string, string> = {
				Control: "Ctrl",
				Alt: "Alt",
				Delete: "Del",
				PageUp: "PgUp",
				PageDown: "PgDn",
				Equals: "=",
				Minus: "-",
				Plus: "+",
				Escape: "Esc",
				Comma: ",",
				Period: ".",
				LeftBracket: "[",
				RightBracket: "]",
				LeftCurlyBracket: "{",
				RightCurlyBracket: "}",
			};
			const iconsAndWidths: Record<string, number> = {
				ArrowUp: 1,
				ArrowRight: 1,
				ArrowDown: 1,
				ArrowLeft: 1,
				Backspace: 2,
				Command: 2,
				Enter: 2,
				Option: 2,
				Shift: 2,
				Tab: 2,
				Space: 3,
			};

			// Strip off the "Key" prefix
			const text = keyText.replace(/^(?:Key)?(.*)$/, "$1");

			// If it's an icon, return the icon identifier
			if (text in iconsAndWidths) {
				return {
					text: null,
					icon: `Keyboard${text}`,
					width: `width-${iconsAndWidths[text] * 8 + 8}`,
				};
			}

			// Otherwise, return the text string
			let result;

			// Letters and numbers
			if (/^[A-Z0-9]$/.test(text)) {
				result = text;
			}
			// Abbreviated names
			else if (text in textMap) {
				result = textMap[text];
			}
			// Other
			else {
				result = text;
			}

			return { text: result, icon: null, width: `width-${(result || " ").length * 8 + 8}` };
		},
		mouseMovementIcon(mouseInputInteraction: MouseInputInteraction) {
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
