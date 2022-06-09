# word-index
该程序用于检索word文件，依赖于Pandoc的解析能力和Meilisearch的检索能力，需要在运行前安装Pandoc和Meilisearch，并配置好相关的环境变量

## Prepare
```
scoop install meilisearch
scoop install pandoc
```

## Project setup
```
yarn install
```

### Compiles and hot-reloads for development
```
yarn tauri:serve
```

### Compiles and minifies for production
```
yarn tauri:build
```

### Lints and fixes files
```
yarn lint
```

### Customize configuration
See [Configuration Reference](https://cli.vuejs.org/config/).
