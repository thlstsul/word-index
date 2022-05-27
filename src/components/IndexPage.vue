<template>
  <a-input-search
    v-model:value="value"
    placeholder="word文件路径"
    enter-button="索引"
    size="large"
    @search="index"
  />

  <a-divider />

  <index-path
    type="primary"
    v-for="path in paths"
    :key="path.value"
    :path="path.value"
    :loaded="path.loaded"
    style="margin-bottom:10px;"
    block
  ></index-path>
</template>
<script>
import { ref, onMounted } from "vue";
import { message } from "ant-design-vue";
import { invoke } from "@tauri-apps/api/tauri";
import IndexPath from "./IndexPath";

export default {
  name: "IndexPage",
  components: {
    IndexPath,
  },
  setup() {
    const value = ref("");
    const paths = ref([]);
    const index = () => {
      save_path(value.value);
      paths.value.push({value: value.value, loaded: false});
    };

    onMounted(() => {
      get_paths()
        .then((res) => {
          const pathsTmp = [];
          for (const path of res) {
            pathsTmp.push({value: path, loaded: true});
          }
          paths.value = pathsTmp;
        })
        .catch((err) => {
          message.info(err);
        });
    });

    return {
      value,
      paths,
      index,
    };
  },
};

function save_path(path) {
  invoke("save_path", { path }).catch((e) => {
    message.info(e);
  });
}

async function get_paths() {
  return invoke("get_paths");
}
</script>
