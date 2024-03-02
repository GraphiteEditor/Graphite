<script lang="ts">
	import { getContext } from "svelte";

	import type { FullscreenState } from "@graphite/state-providers/fullscreen";
	import type { IconName } from "@graphite/utility-functions/icons";
	import { platformIsMac } from "@graphite/utility-functions/platform";
	import { type KeyRaw, type LayoutKeysGroup, type Key, type MouseMotion } from "@graphite/wasm-communication/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

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

	const fullscreen = getContext<FullscreenState>("fullscreen");

	export let keysWithLabelsGroups: LayoutKeysGroup[] = [];
	export let mouseMotion: MouseMotion | undefined = undefined;
	export let requiresLock = false;
	export let textOnly = false;

	$: keyboardLockInfoMessage = watchKeyboardLockInfoMessage(fullscreen.keyboardLockApiSupported);

	$: displayKeyboardLockNotice = requiresLock && !$fullscreen.keyboardLocked;

	function watchKeyboardLockInfoMessage(keyboardLockApiSupported: boolean): string {
		const RESERVED = "This hotkey is reserved by the browser. ";
		const USE_FULLSCREEN = "It is made available in fullscreen mode.";
		const USE_SECURE_CTX = "It is made available in fullscreen mode when this website is served from a secure context (https or localhost).";
		const SWITCH_BROWSER = "Use a Chromium-based browser (like Chrome or Edge) in fullscreen mode to directly use the shortcut.";

		if (keyboardLockApiSupported) return `${RESERVED} ${USE_FULLSCREEN}`;
		if (!("chrome" in window)) return `${RESERVED} ${SWITCH_BROWSER}`;
		if (!window.isSecureContext) return `${RESERVED} ${USE_SECURE_CTX}`;
		return RESERVED;
	}

	function keyTextOrIconList(keyGroup: LayoutKeysGroup): LabelData[] {
		return keyGroup.map((key) => keyTextOrIcon(key));
	}

	function keyTextOrIcon(keyWithLabel: Key): LabelData {
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
		const icon = iconWidth !== undefined && iconWidth > 0 && (keyboardHintIcon(key) || false);
		if (icon) return { icon, width: `width-${iconWidth}` };

		// ...or display text
		return { label, width: `width-${label.length}` };
	}

	function mouseHintIcon(input?: MouseMotion): IconName {
		return `MouseHint${input}` as IconName;
	}

	function keyboardHintIcon(input: KeyRaw): IconName | undefined {
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
	}
</script>

{#if displayKeyboardLockNotice}
	<IconLabel class="user-input-label keyboard-lock-notice" icon="Info" tooltip={keyboardLockInfoMessage} />
{:else}
	<LayoutRow class="user-input-label" classes={{ "text-only": textOnly }}>
		{#each keysWithLabelsGroups as keysWithLabels, groupIndex}
			{#if groupIndex > 0}
				<Separator type="Related" />
			{/if}
			{#each keyTextOrIconList(keysWithLabels) as keyInfo}
				<div class={`input-key ${keyInfo.width}`}>
					{#if keyInfo.icon}
						<IconLabel icon={keyInfo.icon} />
					{:else if keyInfo.label !== undefined}
						<TextLabel>{keyInfo.label}</TextLabel>
					{/if}
				</div>
			{/each}
		{/each}
		{#if mouseMotion}
			<div class="input-mouse">
				<IconLabel icon={mouseHintIcon(mouseMotion)} />
			</div>
		{/if}
		{#if $$slots.default}
			<div class="hint-text">
				<slot />
			</div>
		{/if}
	</LayoutRow>
{/if}

<style lang="scss" global>
	.user-input-label {
		flex: 0 0 auto;
		align-items: center;
		white-space: nowrap;

		&.text-only {
			display: flex;

			.input-key {
				display: flex;
				align-items: center;

				.icon-label {
					margin: calc(calc(18px - 12px) / 2) 0;
				}

				& + .input-key::before {
					line-height: 18px;
					content: "+";
				}
			}
		}

		&:not(.text-only) {
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
				box-sizing: border-box;
				border: 1px solid;
				border-radius: 4px;
				border-color: var(--color-5-dullgray);
				color: var(--color-e-nearwhite);

				.text-label {
					// Firefox renders the text 1px lower than Chrome (tested on Windows) with 16px line-height,
					// so moving it up 1 pixel by using 15px makes them agree.
					line-height: 15px;
				}

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
		}

		.input-mouse {
			.bright {
				fill: var(--color-e-nearwhite);
			}

			.dim {
				fill: var(--color-8-uppergray);
			}
		}

		.hint-text:not(:empty) {
			margin-left: 4px;
		}

		.floating-menu-content .row > & {
			.input-key {
				border-color: var(--color-3-darkgray);
				color: var(--color-8-uppergray);
			}

			.input-key .icon-label svg,
			&.keyboard-lock-notice.keyboard-lock-notice svg,
			.input-mouse .bright {
				fill: var(--color-8-uppergray);
			}

			.input-mouse .dim {
				fill: var(--color-3-darkgray);
			}
		}

		.floating-menu-content .row:hover > & {
			.input-key {
				border-color: var(--color-8-uppergray);
			}

			.input-mouse .dim {
				fill: var(--color-8-uppergray);
			}
		}
	}
</style>
