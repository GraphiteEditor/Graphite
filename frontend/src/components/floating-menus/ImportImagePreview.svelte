<script lang="ts">
	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	export let pastedImage: ImageData | undefined = undefined;
	export let x: number;
	export let y: number;

	//define fucntions here
</script>

<FloatingMenu open={true} class="import-image-paste-preview" type="Cursor" styles={{ left: x + "px", top: y + "px" }}>
	<!-- <div class="image-preview">
		<img classn/>
	</div> -->
	<img src="" />
</FloatingMenu>

<style lang="scss" global></style>
