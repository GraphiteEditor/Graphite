<template>
	<LayoutCol class="main-window">
		<TitleBar :platform="platform" :maximized="maximized" />

		<Workspace />

		<StatusBar />
	</LayoutCol>
</template>

<style lang="scss">
.main-window {
	height: 100%;
	overflow: auto;
	touch-action: none;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import StatusBar from "@/components/window/status-bar/StatusBar.vue";
import TitleBar from "@/components/window/title-bar/TitleBar.vue";
import Workspace from "@/components/window/workspace/Workspace.vue";

export type ApplicationPlatform = "Windows" | "Mac" | "Linux" | "Web";

export default defineComponent({
	data() {
		return {
			platform: "Web" as ApplicationPlatform,
			maximized: true,
		};
	},
	components: {
		LayoutCol,
		StatusBar,
		TitleBar,
		Workspace,
	},
});
</script>
