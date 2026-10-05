<script lang="ts">
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import { ICONS } from "/src/icons";
	import type { IconName } from "/src/icons";
	import type { FrontendRemoteCursor } from "/wrapper/pkg/graphite_wasm_wrapper";

	// Another peer's pointer, placed by the parent in its own pixels: the arrow's tip is the point, and the
	// pill beside it carries the tool the peer holds and their name.
	export let cursor: FrontendRemoteCursor;

	// The tool arrives as a plain string from a peer that may run another build; only a name this build knows is drawn.
	function isIconName(name: string): name is IconName {
		return name in ICONS;
	}
	$: toolIcon = cursor.tool && isIconName(cursor.tool) ? cursor.tool : undefined;
</script>

<div class="remote-cursor" style:left={`${cursor.x}px`} style:top={`${cursor.y}px`}>
	<svg width="12" height="18" viewBox="0 0 12 18"><path d="M0 0 L0 15 L4 11.5 L7 17.5 L9.5 16.5 L6.5 10.5 L11 10.5 Z" fill={cursor.color} stroke="#fff" stroke-width="1" /></svg>
	<span class="label" class:anonymous={cursor.anonymous} style:background={cursor.color}>
		{#if toolIcon}
			<IconLabel icon={toolIcon} iconSizeOverride={16} />
		{/if}
		{cursor.name}
	</span>
</div>

<style lang="scss">
	.remote-cursor {
		position: absolute;
		pointer-events: none;
		z-index: 3;
		display: flex;
		flex-direction: column;
		align-items: flex-start;

		.label {
			display: flex;
			align-items: center;
			gap: 4px;
			margin-left: 12px;
			margin-top: -2px;
			padding: 1px 4px;
			border-radius: 2px;
			color: #fff;
			font-size: 12px;
			line-height: 16px;
			white-space: nowrap;

			&.anonymous {
				font-style: italic;
			}

			:global(.icon-label svg) {
				fill: #fff;
			}
		}
	}
</style>
