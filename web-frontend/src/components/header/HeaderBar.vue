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
import { Options, Vue } from "vue-class-component";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import FileMenu from "./FileMenu.vue";
import WindowTitle from "./WindowTitle.vue";
import WindowButtons from "./WindowButtons.vue";

@Options({
	components: {
		LayoutRow,
		LayoutCol,
		FileMenu,
		WindowTitle,
		WindowButtons,
	},
	props: {},
})
export default class HeaderBar extends Vue {}
</script>
