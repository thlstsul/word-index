<template>
  <a-input-search
    :value="value"
    placeholder="word文件路径"
    enter-button="索引"
    size="large"
    @search="index"
  />

  <index-path
    type="primary"
    v-for="path in paths"
    :key="path.value"
    :path="path.value"
    :loaded="path.loaded"
    style="margin-top:10px;"
    block
  ></index-path>
</template>
<script>
import { ref, onMounted } from "vue";
import { message } from "ant-design-vue";
import { invoke } from "@tauri-apps/api/tauri";
import IndexPath from "./IndexPath.vue";

export default {
  name: "IndexPage",
  components: {
    IndexPath,
  },
  setup() {
    const value = ref("");
    const paths = ref([]);
    const index = () => {
      save_path(value.value)
        .then(() => {
          paths.value.push({ value: value.value, loaded: false });
        })
        .catch((err) => {
          message.error(err);
        });
    };

    onMounted(() => {
      get_paths()
        .then((res) => {
          const pathsTmp = [];
          for (const path of res) {
            pathsTmp.push({ value: path, loaded: true });
          }
          paths.value = pathsTmp;
        })
        .catch((err) => {
          message.error(err);
        });
    });

    return {
      value,
      paths,
      index,
    };
  },
};

async function save_path(path) {
  return invoke("save_path", { path });
}

async function get_paths() {
  return invoke("get_paths");
}
</script>
