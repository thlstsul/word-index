<template>
  <a-drawer
    v-model:visible="visible"
    class="custom-class"
    placement="right"
    :closable="false"
  >
    <index-page></index-page>
  </a-drawer>
  <search-page></search-page>
  <div
    class="shadow"
    @click="visible = true"
  >
    <left-outlined />
  </div>
</template>

<script>
import { ref } from "vue";
import IndexPage from "./components/IndexPage.vue";
import SearchPage from "./components/SearchPage.vue";
import { LeftOutlined } from "@ant-design/icons-vue";

export default {
  name: "App",
  components: {
    IndexPage,
    SearchPage,
    LeftOutlined,
  },
  setup() {
    const visible = ref(false);

    return {
      visible,
    };
  },
};
</script>

<style>
.shadow {
  position: absolute;
  right: 0;
  top: 50%;
  z-index: 998;
  opacity: 100%;
}
</style>
