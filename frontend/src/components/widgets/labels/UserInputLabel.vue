<template>
	<IconLabel class="user-input-label keyboard-lock-notice" v-if="displayKeyboardLockNotice" :icon="'Info'" :title="keyboardLockInfoMessage" />
	<LayoutRow class="user-input-label" v-else>
		<template v-for="(keysWithLabels, i) in keysWithLabelsGroups" :key="i">
			<span class="group-gap" v-if="i > 0"></span>
			<template v-for="(keyInfo, j) in keyTextOrIconList(keysWithLabels)" :key="j">
				<span class="input-key" :class="keyInfo.width">
					<IconLabel v-if="keyInfo.icon" :icon="keyInfo.icon" />
					<template v-else-if="keyInfo.label !== undefined">{{ keyInfo.label }}</template>
				</span>
			</template>
		</template>
		<span class="input-mouse" v-if="mouseMotion">
			<IconLabel :icon="mouseHintIcon(mouseMotion)" />
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

		&.width-1 {
			width: 16px;
		}

		&.width-2 {
			width: 24px;
		}

		&.width-3 {
			width: 32px;
		}

		&.width-4 {
			width: 40px;
		}

		&.width-5 {
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

	.floating-menu-content .row:hover > & {
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
import { defineComponent, type PropType } from "vue";

import { type IconName } from "@/utility-functions/icons";
import { platformIsMac } from "@/utility-functions/platform";
import { type KeyRaw, type KeysGroup, type Key, type MouseMotion } from "@/wasm-communication/messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

type LabelData = { label?: string; icon?: IconName; width: string };

// Keys that become icons if they are listed here with their units of width
const ICON_WIDTHS_MAC = {
	Shift: 2,
	Control: 2,
	Option: 2,
	Command: 2,
};
const ICON_WIDTHS = {
	ArrowUp: 1,
	ArrowRight: 1,
	ArrowDown: 1,
	ArrowLeft: 1,
	Backspace: 2,
	Enter: 2,
	Tab: 2,
	Space: 3,
	...(platformIsMac() ? ICON_WIDTHS_MAC : {}),
};

export default defineComponent({
	inject: ["fullscreen"],
	props: {
		keysWithLabelsGroups: { type: Array as PropType<KeysGroup[]>, default: () => [] },
		mouseMotion: { type: String as PropType<MouseMotion | null>, default: null },
		requiresLock: { type: Boolean as PropType<boolean>, default: false },
	},
	computed: {
		hasSlotContent(): boolean {
			return Boolean(this.$slots.default);
		},
		keyboardLockInfoMessage(): string {
			const RESERVED = "This hotkey is reserved by the browser. ";
			const USE_FULLSCREEN = "It is made available in fullscreen mode.";
			const USE_SECURE_CTX = "It is made available in fullscreen mode when this website is served from a secure context (https or localhost).";
			const SWITCH_BROWSER = "Use a Chromium-based browser (like Chrome or Edge) in fullscreen mode to directly use the shortcut.";

			if (this.fullscreen.keyboardLockApiSupported) return `${RESERVED} ${USE_FULLSCREEN}`;
			if (!("chrome" in window)) return `${RESERVED} ${SWITCH_BROWSER}`;
			if (!window.isSecureContext) return `${RESERVED} ${USE_SECURE_CTX}`;
			return RESERVED;
		},
		displayKeyboardLockNotice(): boolean {
			return this.requiresLock && !this.fullscreen.state.keyboardLocked;
		},
	},
	methods: {
		keyTextOrIconList(keyGroup: KeysGroup): LabelData[] {
			return keyGroup.map((key) => this.keyTextOrIcon(key));
		},
		keyTextOrIcon(keyWithLabel: Key): LabelData {
			// `key` is the name of the `Key` enum in Rust, while `label` is the localized string to display (if it doesn't become an icon)
			let key = keyWithLabel.key;
			const label = keyWithLabel.label;

			// Replace Alt and Accel keys with their Mac-specific equivalents
			if (platformIsMac()) {
				if (key === "Alt") key = "Option";
				if (key === "Accel") key = "Command";
			}

			// Either display an icon...
			// @ts-expect-error We want undefined if it isn't in the object
			const iconWidth: number | undefined = ICON_WIDTHS[key];
			const icon = iconWidth !== undefined && iconWidth > 0 && (this.keyboardHintIcon(key) || false);
			if (icon) return { icon, width: `width-${iconWidth}` };

			// ...or display text
			return { label, width: `width-${label.length}` };
		},
		mouseHintIcon(input: MouseMotion | null): IconName {
			return `MouseHint${input}` as IconName;
		},
		keyboardHintIcon(input: KeyRaw): IconName | undefined {
			switch (input) {
				case "ArrowDown":
					return "KeyboardArrowDown";
				case "ArrowLeft":
					return "KeyboardArrowLeft";
				case "ArrowRight":
					return "KeyboardArrowRight";
				case "ArrowUp":
					return "KeyboardArrowUp";
				case "Backspace":
					return "KeyboardBackspace";
				case "Command":
					return "KeyboardCommand";
				case "Control":
					return "KeyboardControl";
				case "Enter":
					return "KeyboardEnter";
				case "Option":
					return "KeyboardOption";
				case "Shift":
					return "KeyboardShift";
				case "Space":
					return "KeyboardSpace";
				case "Tab":
					return "KeyboardTab";
				default:
					return undefined;
			}
		},
	},
	components: {
		IconLabel,
		LayoutRow,
	},
});
</script>
