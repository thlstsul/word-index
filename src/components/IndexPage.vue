<template>
  <a-input-search
    v-model:value="value"
    placeholder="word文件路径"
    enter-button="索引"
    size="large"
    @search="index"
  />
  <a-divider />
  <a-button
    type="primary"
    v-for="path in paths"
    :key="path"
    @click="() => reindex(path)"
    style="margin-bottom:10px;"
    block
  >{{path}}</a-button>
</template>
<script>
import { ref, onMounted } from "vue";
import { message } from 'ant-design-vue';
import { invoke } from "@tauri-apps/api/tauri";

export default {
  name: "IndexPage",
  setup() {
    const value = ref("");
    const paths = ref([]);
    const index = () => {
      save_path(value.value);
      paths.value.push(value.value);
      index_doc_file(value.value);
    };
    const reindex = (path) => {
      index_doc_file(path);
    };

    onMounted(() => {
      get_paths()
        .then((res) => {
          paths.value = res;
        })
        .catch((err) => {
          message.info(err);
        });
    });

    return {
      value,
      paths,
      index,
      reindex,
    };
  },
};

function index_doc_file(path) {
  invoke("index_doc_file", { dirPath: path }).catch((e) => {
    message.info(e);
  });
}

function save_path(path) {
  invoke("save_path", { path }).catch((e) => {
    message.info(e);
  });
}

async function get_paths() {
  return invoke("get_paths");
}
</script>
