<template>
  <div id="search_wrapper">
    <a-layout>
      <a-layout-header>
        <a-input-search
          id="query"
          v-model:value="value"
          placeholder="关键字"
          enter-button="搜索"
          size="large"
          @search="search"
        />
      </a-layout-header>
      <a-layout-content>
        <div id="layout_content">
          <a-empty
            :description="null"
            :image-style="{height: '100%', margin: '35px'}"
            v-if="docs.length == 0"
          />
          <a-collapse
            v-model:activeKey="activeDoc"
            accordion
            v-if="docs.length > 0"
          >
            <a-collapse-panel
              v-for="(doc, i) in docs"
              :key="i"
              :header="doc.name"
            >
              <a-back-top>
                <div id="ant-back-top-inner">顶</div>
              </a-back-top>
              <a-button
                type="primary"
                @click="() => open_file(doc.path)"
                block
              >打开原文件</a-button>
              <pre id="doc_content">{{doc.content}}</pre>
            </a-collapse-panel>
          </a-collapse>
        </div>
      </a-layout-content>
      <a-layout-footer>
        <a-pagination
          :current="current"
          :total="total"
          :page-size="pageSize"
          show-less-items=true
          page-size-options=[]
          @change="selectPage"
          style="float:right;margin: 10px 0;"
        />
      </a-layout-footer>
    </a-layout>
  </div>
</template>
<script>
import { ref } from "vue";
import { message } from "ant-design-vue";
import { invoke } from "@tauri-apps/api/tauri";

export default {
  name: "SearchPage",
  setup() {
    const value = ref("");
    const current = ref(1);
    const total = ref(0);
    const docs = ref([]);
    const activeDoc = ref([]);
    const pageSize = ref(5);

    const search = () => {
      search_doc_file(value.value, 1, pageSize.value)
        .then((res) => {
          docs.value = res.results;
          total.value = res.total;
          current.value = 1;
        })
        .catch((err) => {
          message.info(err);
        });
    };

    const selectPage = (page) => {
      search_doc_file(value.value, page, pageSize.value)
        .then((res) => {
          docs.value = res.results;
          total.value = res.total;
          current.value = page;
        })
        .catch((err) => {
          message.info(err);
        });
    };

    const open_file = (path) => {
      invoke("open_file", { path }).catch((e) => {
        message.info(e);
      });
    };

    return {
      value,
      current,
      total,
      pageSize,
      docs,
      activeDoc,
      search,
      selectPage,
      open_file,
    };
  },
};

async function search_doc_file(keyword, pageNum, pageSize) {
  const offset = (pageNum - 1) * pageSize;
  const limit = pageSize;
  return invoke("search_doc_file", { keyword, offset, limit });
}
</script>
<style scoped>
.ant-layout-header {
  padding: 0 0;
  color: #fff;
  background: #fff;
}

.ant-layout-content {
  background: #fff;
}

.ant-layout-footer {
  background: #fff;
}

.ant-input-group-wrapper {
  vertical-align: middle;
}

#search_wrapper {
  padding: 24px;
}

#layout_content {
  min-height: 250px;
}

#doc_content {
  font-family: "Microsoft YaHei";
  white-space: pre-wrap;
  word-wrap: break-word;
}

.ant-back-top {
  bottom: 100px;
}

#ant-back-top-inner {
  height: 40px;
  width: 40px;
  line-height: 40px;
  border-radius: 4px;
  background-color: #292a2b;
  color: #e6e6e6;
  text-align: center;
  font-size: 20px;
}
</style>
