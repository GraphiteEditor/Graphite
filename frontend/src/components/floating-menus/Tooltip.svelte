<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { LabeledShortcut } from "@graphite/messages";
	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import ShortcutLabel from "@graphite/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const tooltip = getContext<TooltipState>("tooltip");
	const editor = getContext<Editor>("editor");

	let self: FloatingMenu | undefined;

	$: label = filterTodo($tooltip.element?.getAttribute("data-tooltip-label")?.trim());
	$: description = filterTodo($tooltip.element?.getAttribute("data-tooltip-description")?.trim());
	$: shortcutJSON = $tooltip.element?.getAttribute("data-tooltip-shortcut")?.trim();
	$: shortcut = ((shortcutJSON) => {
		if (!shortcutJSON) return undefined;
		try {
			return JSON.parse(shortcutJSON) as LabeledShortcut;
		} catch {
			return undefined;
		}
	})(shortcutJSON);

	// TODO: Once all TODOs are replaced with real text, remove this function
	function filterTodo(text: string | undefined): string | undefined {
		if (text?.trim().toUpperCase() === "TODO" && !editor.handle.inDevelopmentMode()) return "";
		return text;
	}
</script>

{#if label || description}
	<div class="tooltip" style:top={`${$tooltip.position.y}px`} style:left={`${$tooltip.position.x}px`}>
		<FloatingMenu open={true} type="Tooltip" direction="Bottom" bind:this={self}>
			{#if label || shortcut}
				<LayoutRow class="tooltip-header">
					{#if label}
						<TextLabel class="tooltip-label">{label}</TextLabel>
					{/if}
					{#if shortcut}
						<ShortcutLabel shortcut={{ shortcut }} />
					{/if}
				</LayoutRow>
			{/if}
			{#if description}
				<TextLabel class="tooltip-description">{description}</TextLabel>
			{/if}
		</FloatingMenu>
	</div>
{/if}

<style lang="scss" global>
	.tooltip {
		position: absolute;
		pointer-events: none;
		width: 0;
		height: 0;

		.floating-menu-content {
			max-width: Min(400px, 50vw);

			.tooltip-header + .tooltip-description {
				margin-top: 4px;
			}

			.text-label {
				white-space: pre-wrap;
			}

			.text-label + .shortcut-label {
				margin-left: 8px;
			}

			.tooltip-description {
				color: var(--color-b-lightgray);
			}
		}
	}
</style>
