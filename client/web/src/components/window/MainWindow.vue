<template>
	<LayoutCol class="main-window">
		<LayoutRow :class="'title-bar-row'">
			<TitleBar :platform="platform" :maximized="maximized" />
		</LayoutRow>
		<LayoutRow :class="'workspace-row'">
			<Workspace />
		</LayoutRow>
		<LayoutRow :class="'status-bar-row'">
			<StatusBar />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.main-window {
	height: 100%;
	overflow: auto;
}

.title-bar-row {
	height: 28px;
	flex: 0 0 auto;
}

.workspace-row {
	flex: 1 1 100%;
}

.status-bar-row {
	flex: 0 0 auto;
	// Prevents the creation of a scrollbar due to the child's negative margin
	overflow: hidden;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import Workspace from "../workspace/Workspace.vue";
import TitleBar from "./title-bar/TitleBar.vue";
import StatusBar from "./status-bar/StatusBar.vue";

export enum ApplicationPlatform {
	"Windows" = "Windows",
	"Mac" = "Mac",
	"Linux" = "Linux",
	"Web" = "Web",
}

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		TitleBar,
		Workspace,
		StatusBar,
	},
	data() {
		return {
			platform: ApplicationPlatform.Windows,
			maximized: true,
		};
	},
});
</script>
