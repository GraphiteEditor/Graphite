<template>
	<LayoutRow class="user-input-label">
		<template v-for="(keyGroup, keyGroupIndex) in inputKeys" :key="keyGroupIndex">
			<span class="group-gap" v-if="keyGroupIndex > 0"></span>
			<template v-for="(keyInfo, index) in keyTextOrIconList(keyGroup)" :key="index">
				<span class="input-key" :class="keyInfo.width">
					<IconLabel v-if="keyInfo.icon" :icon="keyInfo.icon" />
					<template v-else>{{ keyInfo.text }}</template>
				</span>
			</template>
		</template>
		<span class="input-mouse" v-if="inputMouse">
			<IconLabel :icon="mouseHintIcon(inputMouse)" />
		</span>
		<span class="hint-text" v-if="hasSlotContent">
			<slot></slot>
		</span>
	</LayoutRow>
</template>

<style lang="scss">
.user-input-label {
	flex: 0 0 auto;
	height: 100%;
	margin: 0 8px;
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
		display: flex;
		justify-content: center;
		align-items: center;
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
		// Firefox renders the text 1px lower than Chrome (tested on Windows) with 16px line-height, so moving it up 1 pixel by using 15px makes them agree
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
import { defineComponent, PropType } from "vue";

import { HintInfo, KeysGroup } from "@/dispatcher/js-messages";

import { IconName } from "@/utilities/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	components: {
		IconLabel,
		LayoutRow,
	},
	props: {
		inputKeys: { type: Array as PropType<HintInfo["key_groups"]>, default: () => [] },
		inputMouse: { type: String as PropType<HintInfo["mouse"]>, default: null },
	},
	computed: {
		hasSlotContent(): boolean {
			return Boolean(this.$slots.default);
		},
	},
	methods: {
		keyTextOrIconList(keyGroup: KeysGroup): { text: string | null; icon: IconName | null; width: string }[] {
			return keyGroup.map((inputKey) => this.keyTextOrIcon(inputKey));
		},
		keyTextOrIcon(keyText: string): { text: string | null; icon: IconName | null; width: string } {
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
					icon: this.keyboardHintIcon(text),
					width: `width-${iconsAndWidths[text] * 8 + 8}`,
				};
			}

			// Otherwise, return the text string
			let result;

			// Letters and numbers
			if (/^[A-Z0-9]$/.test(text)) result = text;
			// Abbreviated names
			else if (text in textMap) result = textMap[text];
			// Other
			else result = text;

			return { text: result, icon: null, width: `width-${(result || " ").length * 8 + 8}` };
		},
		mouseHintIcon(input: HintInfo["mouse"]): IconName {
			return `MouseHint${input}` as IconName;
		},
		keyboardHintIcon(input: HintInfo["key_groups"][0][0]): IconName {
			return `Keyboard${input}` as IconName;
		},
	},
});
</script>
