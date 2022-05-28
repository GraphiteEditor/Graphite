<template>
  <div class="App">
    <h1> Bezier-rs Interactive Documentation </h1>
    <p>
      This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag
      on the endpoints of the example curves to visualize the various Bezier utilities 
      and functions.
    </p>
    <ExamplePane />
  </div>
</template>

<script>

import ExamplePane from './components/ExamplePane.vue'

export default {
  name: 'App',
  components: {
    ExamplePane
  }
}
</script>

<style>
#app {
  font-family: Avenir, Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-align: center;
  color: #2c3e50;
  margin-top: 60px;
}
h1 {
  margin: 40px 0 0;
}
</style>
