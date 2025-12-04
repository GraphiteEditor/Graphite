<script lang="ts">
	import type { IconName } from "@graphite/icons";
	import type { ActionShortcut, KeyRaw, LabeledShortcut, MouseMotion } from "@graphite/messages";
	import { operatingSystem } from "@graphite/utility-functions/platform";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	export let shortcut: ActionShortcut;

	function keyTextOrIconList(keyGroup: LabeledShortcut): { label?: string; icon?: IconName; mouseMotion?: MouseMotion }[] {
		const list = keyGroup.map((labeledKeyOrMouseMotion) => {
			// Use a mouse icon if it's a mouse motion instead of a key
			if (typeof labeledKeyOrMouseMotion === "string") return { mouseMotion: labeledKeyOrMouseMotion };

			// `key` is the name of the `Key` enum in Rust, while `label` is the localized string to display (if it doesn't become an icon)
			let key = labeledKeyOrMouseMotion.key;
			const label = labeledKeyOrMouseMotion.label;

			// Replace Alt and Accel keys with their Mac-specific equivalents
			if (operatingSystem() === "Mac") {
				if (key === "Alt") key = "Option";
				if (key === "Accel") key = "Command";
			}

			// Either display an icon...
			const icon = keyboardHintIcon(key);
			if (icon) return { icon };

			// ...or display text
			return { label };
		});

		// Consolidate consecutive labels into a concatenated single label
		const consolidatedList: typeof list = [];
		list.forEach((item) => {
			const lastItem = consolidatedList[consolidatedList.length - 1];
			if (item.label && lastItem?.label) lastItem.label += " " + item.label;
			else consolidatedList.push(item);
		});
		return consolidatedList;
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
			case "Enter":
				return "KeyboardEnter";
			case "Space":
				return "KeyboardSpace";
			case "Tab":
				return "KeyboardTab";
			case "Command":
				return operatingSystem() === "Mac" ? "KeyboardCommand" : undefined;
			case "Control":
				return operatingSystem() === "Mac" ? "KeyboardControl" : undefined;
			case "Option":
				return operatingSystem() === "Mac" ? "KeyboardOption" : undefined;
			case "Shift":
				return operatingSystem() === "Mac" ? "KeyboardShift" : undefined;
			default:
				return undefined;
		}
	}

	function mouseHintIcon(input?: MouseMotion): IconName {
		return `MouseHint${input}` as IconName;
	}
</script>

<LayoutRow class="shortcut-label">
	{#each keyTextOrIconList(shortcut.shortcut) as { label, icon, mouseMotion }}
		{#if label}
			<div class="key-label">
				<TextLabel>{label}</TextLabel>
			</div>
		{:else if icon}
			<div class="key-icon">
				<IconLabel {icon} />
			</div>
		{:else if mouseMotion}
			<div class="mouse-icon">
				<IconLabel icon={mouseHintIcon(mouseMotion)} />
			</div>
		{/if}
	{/each}
</LayoutRow>

<style lang="scss" global>
	.shortcut-label {
		.key-icon,
		.key-label {
			display: flex;
			align-items: center;
			height: 16px;
			padding: 0 4px;
			border-radius: 4px;
			background: var(--color-3-darkgray);
			color: var(--color-b-lightgray);
			fill: var(--color-b-lightgray);
		}

		svg {
			fill: var(--color-b-lightgray);

			.dim {
				fill: var(--color-8-uppergray);
			}
		}

		.floating-menu-content .row > & {
			.key-label,
			.key-icon,
			.mouse-icon {
				color: var(--color-8-uppergray);
				background: none;

				&:first-child {
					padding-left: 0;
				}

				&:last-child {
					padding-right: 0;
				}
			}

			.key-icon svg {
				fill: var(--color-8-uppergray);
			}

			.mouse-icon svg {
				// 3 shades brighter than the 8-uppergray of key labels/icons
				fill: var(--color-b-lightgray);

				.dim {
					// 3 shades darker than the 8-uppergray of key labels/icons
					fill: var(--color-5-dullgray);
				}
			}
		}
	}
</style>
