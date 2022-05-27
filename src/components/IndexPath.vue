<template>
  <a-button
    @click="() => reindex(path)"
    v-bind="$attrs"
    :loading="loading"
  >{{path}}</a-button>
</template>
<script>
import { ref, onMounted } from "vue";
import { message } from "ant-design-vue";
import { invoke } from "@tauri-apps/api/tauri";

export default {
  name: "IndexPath",
  props: {
    path: String,
    loaded: {
      type: Boolean,
      default: true,
    },
  },
  setup(props) {
    const loading = ref(!props.loaded);
    const reindex = (path) => {
      loading.value = true;
      index_doc_file(path)
        .then(() => {
          loading.value = false;
        })
        .catch((e) => {
          message.info(e);
        });
    };

    onMounted(() => {
      if (props.loading) {
        index_doc_file(props.path)
          .then(() => {
            loading.value = false;
          })
          .catch((e) => {
            message.info(e);
          });
      }
    });

    return {
      loading,
      reindex,
    };
  },
};

async function index_doc_file(path) {
  return invoke("index_doc_file", { dirPath: path });
}
</script>