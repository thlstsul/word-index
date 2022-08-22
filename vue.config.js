const { defineConfig } = require("@vue/cli-service");
module.exports = defineConfig({
  transpileDependencies: true,
  configureWebpack: {
    plugins: [],
  },
  css: {
    loaderOptions: {
      less: {
        lessOptions: {
          modifyVars: {
            "primary-color": "#292a2b",
            "link-color": "#e6e6e6",
            "border-radius-base": "2px",
          },
          javascriptEnabled: true,
        },
      },
    },
  },
});
