<script lang="ts">
	import type { IconName } from "@graphite/icons";
	import { type KeyRaw, type LayoutKeysGroup, type MouseMotion } from "@graphite/messages";
	import { operatingSystem } from "@graphite/utility-functions/platform";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	export let keysWithLabelsGroups: LayoutKeysGroup[] = [];
	export let mouseMotion: MouseMotion | undefined = undefined;

	function keyTextOrIconList(keyGroup: LayoutKeysGroup): { label?: string; icon?: IconName }[] {
		const list = keyGroup.map((keyWithLabel) => {
			// `key` is the name of the `Key` enum in Rust, while `label` is the localized string to display (if it doesn't become an icon)
			let key = keyWithLabel.key;
			const label = keyWithLabel.label;

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
	{#each keysWithLabelsGroups as keysWithLabels}
		<div class="input-key">
			{#each keyTextOrIconList(keysWithLabels) as keyInfo}
				{#if keyInfo.icon}
					<IconLabel icon={keyInfo.icon} />
				{:else if keyInfo.label !== undefined}
					<TextLabel>{keyInfo.label}</TextLabel>
				{/if}
			{/each}
		</div>
	{/each}
	{#if mouseMotion}
		<div class="input-mouse">
			<IconLabel icon={mouseHintIcon(mouseMotion)} />
		</div>
	{/if}
</LayoutRow>

<style lang="scss" global>
	.shortcut-label {
		flex: 0 0 auto;
		align-items: center;
		white-space: nowrap;

		.input-key {
			display: flex;
			align-items: center;
			height: 16px;
			border-radius: 4px;
			background: var(--color-3-darkgray);
			color: var(--color-b-lightgray);

			> * {
				margin: 0 4px;

				+ * {
					margin-left: 0;
				}
			}

			.icon-label {
				fill: var(--color-b-lightgray);
			}

			+ .input-key {
				margin-left: 4px;
			}

			+ .input-mouse {
				margin-left: 2px;
			}
		}

		.icon-label svg {
			fill: var(--color-b-lightgray);

			.dim {
				fill: var(--color-8-uppergray);
			}
		}

		.floating-menu-content .row > & {
			.input-key {
				color: var(--color-8-uppergray);
				background: none;

				> :first-child {
					margin-left: 0;
				}

				> :last-child {
					margin-right: 0;
				}
			}

			.icon-label svg {
				fill: var(--color-8-uppergray);

				.dim {
					fill: var(--color-3-darkgray);
				}
			}
		}
	}
</style>
