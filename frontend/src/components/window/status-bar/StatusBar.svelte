<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { type ActionKeys, type HintData, type HintInfo, UpdateInputHints } from "@graphite/messages";
	import { operatingSystem } from "@graphite/utility-functions/platform";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import ShortcutLabel from "@graphite/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const editor = getContext<Editor>("editor");

	let hintData: HintData = [];

	function inputKeysForPlatform(hint: HintInfo): ActionKeys[] {
		return operatingSystem() === "Mac" && hint.keyGroupsMac ? hint.keyGroupsMac.map((keys) => ({ keys })) : hint.keyGroups.map((keys) => ({ keys }));
	}

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateInputHints, (data) => {
			hintData = data.hintData;
		});
	});
</script>

<LayoutRow class="status-bar">
	<LayoutRow class="hint-groups">
		{#each hintData as hintGroup, index}
			{#if index !== 0}
				<Separator type="Section" />
			{/if}
			{#each hintGroup as hint}
				{#if hint.plus}
					<TextLabel bold={true} class="plus">+</TextLabel>
				{/if}
				{#if hint.slash}
					<TextLabel bold={true} class="slash">/</TextLabel>
				{/if}
				<ShortcutLabel mouseMotion={hint.mouse} shortcuts={inputKeysForPlatform(hint)} />
				{#if hint.label}
					<TextLabel class="hint-text">{hint.label}</TextLabel>
				{/if}
			{/each}
		{/each}
	</LayoutRow>
</LayoutRow>

<style lang="scss" global>
	.status-bar {
		height: 24px;
		width: 100%;
		flex: 0 0 auto;

		.hint-groups {
			flex: 0 0 auto;
			max-width: 100%;
			margin: 0 -4px;
			overflow: hidden;

			.separator.section {
				// Width of section separator (12px) minus the margin of the surrounding shortcut labels (8px)
				margin: 0 calc(12px - 8px);
			}

			:is(.plus, .slash, .hint-text, .shortcut-label) {
				white-space: nowrap;
				flex-shrink: 0;
				line-height: 24px;
				margin: 0 8px;

				+ :is(.plus, .slash, .shortcut-label) {
					margin-left: 0;
				}
			}

			.hint-text {
				margin-left: -4px;
			}
		}
	}
</style>
