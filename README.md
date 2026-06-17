# Local Knowledge Search

一个用 Rust 实现的本地知识库搜索系统。项目可以扫描本地文档目录，抽取文本内容，使用 Tantivy 构建全文索引，并提供命令行和 TUI 两种使用方式。

## 项目定位

本项目面向个人本地资料检索场景，适合管理课程笔记、Markdown 文档、文本文件、代码片段、配置文件、PDF 和 DOCX 文档。

它不是简单的 CRUD 项目，而是一个完整的 Rust 命令行工具，包含文件遍历、文档解析、全文索引、中文分词、终端 UI、本地持久化、错误处理和模块化组织。

## 功能特性

- 扫描本地文档目录并建立全文索引
- 支持 `md`、`txt`、`rs`、`toml`、`json`、`csv`、`log`、`yaml`、`yml`、`pdf`、`docx`
- 自动提取 Markdown 一级标题作为文档标题
- 记录文档路径、扩展名、文件大小和修改时间
- 使用 Tantivy 构建高性能倒排索引
- 使用 `jieba-rs` 对中文、日文、韩文内容进行分词
- 支持完整重建索引和增量更新索引
- 支持命令行搜索、结果数量统计、摘要预览和 JSON 输出
- 提供 TUI 交互界面，可搜索、预览、收藏、查看历史和统计信息
- 使用本地 JSON 文件保存搜索历史、收藏夹和索引清单

## 项目结构

```text
.
├── Cargo.toml
├── Cargo.lock
├── README.md
├── knowledge_config.json
├── docs/
├── knowledge_data/
├── knowledge_index/
├── tests/
└── src/
    ├── main.rs         # CLI 入口和子命令定义
    ├── lib.rs          # 模块导出
    ├── app.rs          # TUI 应用状态和交互逻辑
    ├── config.rs       # 配置文件读写和校验
    ├── document.rs     # 文档扫描、读取和标题提取
    ├── errors.rs       # 自定义错误类型
    ├── highlighter.rs  # 搜索摘要和匹配统计
    ├── indexer.rs      # Tantivy 索引构建、更新和搜索
    ├── models.rs       # 核心数据结构
    ├── storage.rs      # 历史、收藏、manifest 持久化
    ├── tokenizer.rs    # 中英文分词处理
    ├── analyzer.rs     # 查询、历史和结果分析工具
    ├── tui.rs          # Ratatui 终端界面
    └── utils.rs        # 通用工具函数
```

## 快速开始

### 1. 初始化配置

```bash
cargo run -- init
```

默认会生成 `knowledge_config.json`。主要配置项包括：

```json
{
  "docs_dir": "./docs",
  "index_dir": "./knowledge_index",
  "data_dir": "./knowledge_data",
  "max_file_bytes": 2097152,
  "supported_extensions": ["md", "txt", "rs", "toml", "json", "csv", "log", "yaml", "yml", "pdf", "docx"],
  "default_limit": 10,
  "snippet_chars": 160
}
```

### 2. 构建索引

```bash
cargo run -- index --docs ./docs
```

如果不传 `--docs`，程序会使用配置文件中的 `docs_dir`。

### 3. 搜索文档

```bash
cargo run -- search Rust
cargo run -- search Tantivy --limit 5
cargo run -- search "所有权"
```

搜索结果会包含标题、得分、匹配词数量、文件类型、大小、路径和摘要。

### 4. 输出 JSON

```bash
cargo run -- search Rust --json
```

### 5. 预览文档

```bash
cargo run -- preview ./docs/rust_ownership.md
cargo run -- preview ./docs/rust_ownership.md --chars 1000
```

### 6. 增量更新索引

```bash
cargo run -- update --docs ./docs
```

增量更新会根据 `knowledge_data/manifest.json` 判断文件是否发生变化，只重新索引变更过的文档。

### 7. 查看索引统计

```bash
cargo run -- stats
```

### 8. 查看和清空搜索历史

```bash
cargo run -- history
cargo run -- clear-history
```

### 9. 管理收藏

```bash
cargo run -- bookmarks
cargo run -- bookmark "Rust Ownership" ./docs/rust_ownership.md
```

### 10. 启动 TUI

```bash
cargo run -- tui
```

## TUI 快捷键

| 场景 | 快捷键 | 功能 |
| --- | --- | --- |
| 搜索页 | 输入关键字 | 编辑查询内容 |
| 搜索页 | `Enter` | 执行搜索 |
| 搜索页 | `Up` / `Down` | 选择搜索结果 |
| 搜索页 | `Tab` | 预览选中文档 |
| 搜索页 | `Ctrl+B` | 收藏选中文档 |
| 搜索页 | `Ctrl+O` | 进入命令模式 |
| 搜索页 | `Esc` | 退出 TUI |
| 命令模式 | `p` | 预览选中文档 |
| 命令模式 | `b` | 收藏选中文档 |
| 命令模式 | `h` | 查看搜索历史 |
| 命令模式 | `m` | 查看收藏夹 |
| 命令模式 | `s` | 查看索引统计 |
| 命令模式 | `u` | 增量更新索引 |
| 命令模式 | `?` | 查看帮助 |
| 命令模式 | `q` | 退出 TUI |
| 预览页 | `Up` / `Down`、`PageUp` / `PageDown` | 滚动文档 |
| 预览页 | `Tab` / `Esc` / `q` | 返回搜索页 |
| 历史页 | `c` | 清空搜索历史 |
| 收藏页 | `Enter` | 预览收藏文档 |
| 收藏页 | `Delete` / `Ctrl+D` | 删除收藏 |

## 本地数据文件

默认情况下，运行时数据保存在 `knowledge_data/`：

- `history.json`：搜索历史，最多保留 200 条
- `bookmarks.json`：收藏文档列表
- `manifest.json`：索引文档清单，用于统计和增量更新

全文索引保存在 `knowledge_index/`。这些目录属于运行时产物，通常不需要提交到 Git。

## 核心实现

### 文档加载

`DocumentLoader` 负责遍历目录、过滤扩展名、读取文件内容并生成 `KnowledgeDocument`。普通文本文件直接读取为 UTF-8 文本，PDF 使用 `pdf-extract` 抽取文本，DOCX 通过 ZIP 和 XML 解析读取正文。

### 索引与搜索

`KnowledgeIndex` 封装 Tantivy 索引。索引字段包括标题、路径、扩展名、原始内容、分词内容、文件大小和修改时间。搜索时会对查询词进行同样的分词处理，并返回匹配总数和 Top N 结果。

### 增量更新

系统会把已索引文件的路径、大小、修改时间和标题保存到 `manifest.json`。执行 `update` 时，如果文件大小和修改时间没有变化，就跳过该文件；否则删除旧索引项并写入新内容。

### TUI

终端界面基于 Ratatui 和 Crossterm 实现。`AppState` 统一管理输入框、搜索结果、预览内容、历史、收藏、统计信息和当前界面模式。

## 测试

运行全部测试：

```bash
cargo test
```

提交前建议执行：

```bash
cargo fmt
cargo clippy
cargo test
```

## 适合作业报告的亮点

1. 使用 Rust 实现完整的本地知识库搜索工具。
2. 使用 Tantivy 倒排索引完成高性能全文检索。
3. 同时支持 CLI 和 TUI 两种交互方式。
4. 支持中文分词，提高中文内容搜索效果。
5. 支持 PDF 和 DOCX 文档解析，覆盖更真实的本地资料场景。
6. 支持搜索历史、收藏夹、索引统计和增量更新。
7. 使用 `serde`、`anyhow`、`thiserror`、`clap`、`ratatui` 等 Rust 生态库完成工程化实现。

## 后续扩展方向

- 支持多目录同时索引
- 支持标签和分类管理
- 支持模糊搜索和拼写纠错
- 支持打开外部编辑器定位到文档
- 支持更细粒度的结果高亮
- 支持更多文档格式，例如 HTML、EPUB、PPTX
