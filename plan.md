

# 📘 PDF + EPUB 阅读器完整执行计划（Desktop Only）v2.0

---

# 🎯 0. 项目本质定义（必须先统一）

这个项目不是普通阅读器，而是：

> **基于 Mode 状态机 + GPU 渲染 + 自适应功能系统的文档交互引擎**

支持：

* PDF（MuPDF）
* EPUB（自研 reflow）
* TXT（reflow）

核心能力：

* 阅读
* 自动阅读（时间媒体化）
* 标注编辑
* 搜索
* 本地库管理
* 自适应 UI

---

# 🧱 1. 系统总架构

```text id="arch_v2"
app/
 ├── ui/
 │    ├── shell (egui)
 │    ├── mode_ui
 │    └── feature_ui
 │
 ├── core/
 │    ├── app_state
 │    ├── mode_system
 │    ├── feature_system
 │    └── document_manager
 │
 ├── interaction/
 │    ├── input_router
 │    ├── mode_handlers
 │
 ├── engines/
 │    ├── pdf_engine (mupdf)
 │    ├── reflow_engine (epub/txt)
 │
 ├── auto_reading/
 │    ├── controller
 │    ├── page_flow
 │    ├── glyph_reveal
 │    └── sentence_stream
 │
 ├── annotation/
 │    ├── engine
 │    ├── overlay_renderer
 │    └── tool_system
 │
 ├── layout/
 │    ├── line_breaker
 │    ├── paginator
 │    └── glyph_cache
 │
 ├── render/
 │    ├── wgpu_renderer
 │    ├── tile_cache (pdf)
 │    └── overlay_compositor
 │
 ├── services/
 │    ├── search (FTS5)
 │    ├── annotation_service
 │    ├── bookmark
 │    └── usage_tracker
 │
 ├── storage/
 │    ├── sqlite
 │    ├── feature_store
 │    └── library_index
 │
 └── platform/
      ├── fs
      └── font_loader
```

---

# 🎭 2. Mode System（核心）

---

## 2.1 三种模式

```rust id="mode"
enum Mode {
    Reading(ReadingState),
    Auto(AutoState),
    Annotate(AnnotateState),
}
```

---

## 2.2 Mode 本质定义

> Mode = 输入解释器 + UI配置 + 功能作用域

---

## 2.3 Mode Controller

```rust id="mode_controller"
trait ModeController {
    fn switch(&mut self, mode: Mode);
    fn current(&self) -> &Mode;
}
```

---

## 2.4 输入路由（核心）

```text id="input_flow"
Input Event
   ↓
Mode System
   ↓
Mode Handler
   ├── Reading
   ├── Auto
   └── Annotate
```

---

# 📖 3. Document Engine（双模型）

---

## 3.1 固定布局（PDF）

```text id="pdf_model"
MuPDF
  → page render
  → tile cache
  → GPU texture
```

---

## 3.2 可重排（EPUB / TXT）

```text id="reflow_model"
EPUB/TXT
  → parse
  → text spans
  → layout engine
  → pagination
```

---

## 3.3 统一接口

```rust id="doc_trait"
trait Document {
    fn page_count(&self) -> usize;
    fn render(&self, page: usize) -> RenderResult;
    fn text(&self, page: usize) -> Vec<TextSpan>;
}
```

---

# 🎬 4. Auto Reading System（核心差异化功能）

---

## 4.1 三种模式

```rust id="auto_modes"
enum AutoMode {
    PageFlow,
    GlyphReveal,
    SentenceStream,
}
```

---

## 4.2 时间驱动模型

```text id="auto_flow"
clock.tick()
  → auto_controller.update()
  → render_state_update()
```

---

## 4.3 三种表现

### PageFlow

* 类视频翻页
* GPU translate

### GlyphReveal

* 逐字/逐行出现
* shader mask

### SentenceStream

* 类歌词浮窗阅读

---

## 4.4 核心原则

* layout 只做一次
* auto 只控制“显示状态”
* 不重新排版

---

# ✍️ 5. Annotate System（编辑模式）

---

## 5.1 Tool System

```rust id="tools"
enum Tool {
    Highlight,
    Pen,
    Note,
    Eraser,
    Select,
}
```

---

## 5.2 Annotation 模型

```rust id="annotation"
struct Annotation {
    id: String,
    doc_id: String,
    kind: AnnotationType,

    // PDF
    rects: Vec<Rect>,

    // EPUB
    range: TextRange,

    note: Option<String>,
}
```

---

## 5.3 Overlay 架构

```text id="overlay"
Document (immutable)
     +
Annotation Layer
     ↓
Render Composite
```

---

## 5.4 Mode 行为

* 自动阅读暂停
* UI工具栏展开
* 支持 undo/redo

---

# 🧠 6. 自适应功能系统（核心升级）

---

## 6.1 Feature Model

```rust id="feature"
struct Feature {
    id: String,
    usage: u32,
    pinned: bool,
    mode_scope: Mode,
}
```

---

## 6.2 UI 编译模型

```text id="ui_compile"
Feature Pool
 + Usage Tracker
 + Pinned Features
   ↓
Mode UI Layout
```

---

## 6.3 三机制

### Pin（固定）

* 用户显式加入主 UI

### Promote（自动提升）

* 高频功能自动进入快捷栏

### Demote（隐藏）

* 低频进入 command palette

---

## 6.4 UI分层

```text id="ui_layers"
Stable UI (never changes)
   ↓
Adaptive UI (learned)
   ↓
Hidden Features (palette)
```

---

# ⚡ 7. Rendering System（GPU）

---

## pipeline

```text id="render_pipeline"
Document State
 + Mode State
 + Annotation Layer
 + Auto State
   ↓
wgpu renderer
```

---

## PDF优化

* tile cache
* LRU page cache
* async decode

---

## EPUB优化

* layout cache
* chapter lazy load

---

# 🔎 8. Search System

* PDF：MuPDF text extraction
* EPUB/TXT：SQLite FTS5

---

# 💾 9. Storage

SQLite 表：

```sql id="db"
books
progress
annotations
bookmarks
feature_usage
search_index
```

---

# 🧭 10. UI设计原则（关键约束）

---

## 10.1 三模式 UI

| Mode     | UI特征   |
| -------- | ------ |
| Reading  | 极简     |
| Auto     | 时间控制   |
| Annotate | 工具完整展开 |

---

## 10.2 UI核心原则

* UI 永远不直接操作文档
* UI = Mode + Feature Compiler
* 功能可以隐藏，但不能丢失

---

# 🚀 11. 开发路线图

---

## Phase 1（MVP）

* PDF + EPUB
* Reading Mode
* Mode system
* basic rendering

---

## Phase 2

* Annotate Mode
* annotation overlay
* tool system

---

## Phase 3

* Auto Reading System
* glyph reveal
* sentence stream

---

## Phase 4

* Feature adaptive system
* pin/promote/demote
* UI compiler

---

# 🧠 12. 系统核心一句话

---

> 一个基于 Mode 状态机 + GPU 渲染 + 行为学习 UI 编译器的文档阅读引擎

---

# 🔥 如果你下一步要继续（很关键）

我建议下一步直接进入：

👉 **Rust 可实现版本：trait + state machine + event bus + minimal egui UI scaffold**

可以直接变成：

* repo 结构
* 可运行 demo
* PDF 打开 + mode 切换 + annotation overlay

技术栈 rust egui mupdf 自研排版引擎(用于txt，epub，md)
