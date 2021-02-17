<template>
	<div class="header-third">
		<FileMenu />
	</div>
	<div class="header-third">
		<WindowTitle />
	</div>
	<div class="header-third">
		<WindowButtons :maximized="true" />
	</div>
</template>

<style lang="scss">
.header-third {
	display: flex;
	flex: 1 1 100%;

	&:nth-child(1) {
		justify-content: flex-start;
	}

	&:nth-child(2) {
		justify-content: center;
	}

	&:nth-child(3) {
		justify-content: flex-end;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import FileMenu from "./FileMenu.vue";
import WindowTitle from "./WindowTitle.vue";
import WindowButtons from "./WindowButtons.vue";

export default defineComponent({
	name: "HeaderBar",
	components: {
		FileMenu,
		WindowTitle,
		WindowButtons,
	},
});
</script>
