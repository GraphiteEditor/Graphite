<script lang="ts">
	export let x: number = 0;
	export let y: number = 0;
	export let visible: boolean = false;
</script>

{#if visible}
	<svg class="software-cursor" style:left="{x}px" style:top="{y}px" width="15" height="27" viewBox="0 0 10 18">
		<path d="M0.7 0.7 L0.7 14.5 L3.4 11.9 L5.4 16.6 L7.4 15.7 L5.5 11.1 L9 11.1 Z" fill="black" stroke="white" stroke-width="1.2" stroke-linejoin="round" />
	</svg>
{/if}

<style lang="scss">
	.software-cursor {
		position: absolute;
		pointer-events: none;
		z-index: 100;
	}
</style>
