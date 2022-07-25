<template>
	<IconLabel class="user-input-label keyboard-lock-notice" v-if="displayKeyboardLockNotice" :icon="'Info'" :title="keyboardLockInfoMessage" />
	<LayoutRow class="user-input-label" v-else>
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
		height: 16px;
		// Firefox renders the text 1px lower than Chrome (tested on Windows) with 16px line-height, so moving it up 1 pixel by using 15px makes them agree
		line-height: 15px;
		box-sizing: border-box;
		border: 1px solid;
		border-radius: 4px;
		border-color: var(--color-7-middlegray);
		color: var(--color-e-nearwhite);

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

	.floating-menu-content & {
		.input-key {
			border-color: var(--color-4-dimgray);
			color: var(--color-8-uppergray);
		}

		.input-key .icon-label svg,
		&.keyboard-lock-notice.keyboard-lock-notice svg,
		.input-mouse .bright {
			fill: var(--color-8-uppergray);
		}

		.input-mouse .dim {
			fill: var(--color-4-dimgray);
		}
	}

	.floating-menu-content .row:hover & {
		.input-key {
			border-color: var(--color-7-middlegray);
			color: var(--color-9-palegray);
		}

		.input-key .icon-label svg,
		&.keyboard-lock-notice.keyboard-lock-notice svg,
		.input-mouse .bright {
			fill: var(--color-9-palegray);
		}

		.input-mouse .dim {
			fill: var(--color-7-middlegray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IconName } from "@/utility-functions/icons";
import { HintInfo, KeysGroup } from "@/wasm-communication/messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	inject: ["fullscreen"],
	props: {
		inputKeys: { type: Array as PropType<HintInfo["key_groups"]>, default: () => [] },
		inputMouse: { type: String as PropType<HintInfo["mouse"]>, default: null },
		requiresLock: { type: Boolean as PropType<boolean>, default: false },
	},
	computed: {
		hasSlotContent(): boolean {
			return Boolean(this.$slots.default);
		},
		keyboardLockInfoMessage(): string {
			const USE_FULLSCREEN = "This hotkey is reserved by the browser, but becomes available in fullscreen mode";
			const SWITCH_BROWSER = "This hotkey is reserved by the browser, but becomes available in Chrome, Edge, and Opera which support the Keyboard.lock() API";

			return this.fullscreen.keyboardLockApiSupported ? USE_FULLSCREEN : SWITCH_BROWSER;
		},
		displayKeyboardLockNotice(): boolean {
			return this.requiresLock && !this.fullscreen.state.keyboardLocked;
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
	components: {
		IconLabel,
		LayoutRow,
	},
});
</script>
