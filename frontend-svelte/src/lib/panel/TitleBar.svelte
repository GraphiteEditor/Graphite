<script lang="ts" context="module">
  export type Platform = "Windows" | "Mac" | "Linux" | "Web";
</script>

<script lang="ts">
  import LayoutRow from "$lib/layout/LayoutRow.svelte";
  // import MenuBarInput from "$lib/widgets/inputs/MenuBarInput.svelte";
  // import WindowButtonsMac from "$lib/window/title-bar/WindowButtonsMac.svelte";
  // import WindowButtonsWeb from "$lib/window/title-bar/WindowButtonsWeb.svelte";
  // import WindowButtonsWindows from "$lib/window/title-bar/WindowButtonsWindows.svelte";
  // import WindowTitle from "$lib/window/title-bar/WindowTitle.svelte";

  export let platform: Platform;
  export let maximized: boolean;

  // inject: ["portfolio"],

  // $: index = portfolio.state.activeDocumentIndex;
  // $: name = portfolio.state.documents[index]?.displayName || "";
  // $: windowTitle = `${name}${name && " - "}Graphite`;
  let windowTitle = "TODO - Graphite";
</script>

<LayoutRow class="title-bar">
  <LayoutRow class="header-part">
    {#if platform === "Mac"}
      <!-- <WindowButtonsMac {maximized} /> -->
    {:else}
      <!-- <MenuBarInput /> -->
    {/if}
  </LayoutRow>
  <LayoutRow class="header-part">
    <!-- <WindowTitle text={windowTitle} /> -->
  </LayoutRow>
  <LayoutRow class="header-part">
    {#if platform === "Windows" || platform === "Linux"}
      <!-- <WindowButtonsWindows {maximized} /> -->
    {:else if platform === "Web"}
      <!-- <WindowButtonsWeb {maximized} /> -->
    {/if}
  </LayoutRow>
</LayoutRow>

<style lang="scss">
  .title-bar {
    height: 28px;
    flex: 0 0 auto;

    .header-part {
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
  }
</style>
