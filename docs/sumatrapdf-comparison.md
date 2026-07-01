# SumatraPDF 对比与差距分析报告

> Folix — Rust 实现的 PDF/EPUB 阅读器
> 对比目标：SumatraPDF 3.7 (Windows)

---

## 一、概述

SumatraPDF 是 Windows 上最优秀的轻量级阅读器之一，支持 17+ 文件格式，拥有成熟的引擎架构、LRU 渲染缓存、平坦文字模型、完善的设置系统和扩展能力。

Folix 是一个 Rust + egui 的跨平台阅读器原型，目前支持 PDF/EPUB/TXT 三种格式，具备基本阅读功能。

**本报告的目的：** 系统化梳理 Folix 与 SumatraPDF 之间的功能和技术差距，为后续开发提供 roadmap。

---

## 二、功能对比

### 2.1 文件格式支持

| 格式 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| PDF | ✅ | ✅ | Folix 使用 mupdf 渲染，支持文字提取 |
| EPUB | ✅ | ✅ | 使用 rbook 解析，HTML 标签剥离 |
| MOBI | ✅ | ❌ | 无解析器 |
| FB2 / FB2.zip | ✅ | ❌ | 无解析器 |
| XPS / OXPS | ✅ | ❌ | 无解析器 |
| DjVu | ✅ | ❌ | 无解析器 |
| CHM | ✅ | ❌ | 无解析器 |
| Comic Book (CBZ/CBT/CB7) | ✅ | ❌ | 无图片归档解析器 |
| HTML / HTM | ✅ | ❌ | Sumatra 使用自己的 HTML 引擎 |
| TXT | ✅ | ✅ | Folix 支持 UTF-8/GBK/Big5/Shift_JIS |
| SVG | ✅ | ❌ | 无 SVG 解析器 |
| CBR (RAR comic) | ✅ | ❌ | 无 RAR 支持 |
| TGA / PPM / PGM / PBM | ✅ | ❌ | 无图片格式支持 |
| 虚拟打印驱动 | ✅ | ❌ | Windows only |

**差距摘要：** Folix 支持 3 种格式，Sumatra 支持 17+ 种。差距主要在于缺乏 CBZ/CBR 等漫画格式和 XPS/DjVu 等文档格式的解析器。

### 2.2 阅读导航

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 翻页（上/下页） | ✅ | ✅ | 工具栏按钮 |
| 页码跳转 | ✅ | ❌ | 无"转到页码"对话框，仅能通过滚动定位 |
| 缩放（滑块/百分比） | ✅ | ✅ | 滑块 0.5x–3.0x |
| 适合宽度/适合页面 | ✅ | ❌ | 无自动缩放快捷键 |
| 旋转页面 | ✅ | ✅ | Edit 模式下 CW/CCW |
| 双页显示 | ✅ | ❌ | 仅单页 |
| 书籍模式（封面单独） | ✅ | ❌ | 仅 Paged/Scroll 两种 |
| 全屏模式 | ✅ (F11) | ❌ | 无 |
| 演示模式（幻灯片） | ✅ (Ctrl+L) | ❌ | 无 |
| 逆序阅读（RTL） | ✅ (Manga 模式) | ❌ | 无 |
| 滚动（平滑/逐行） | ✅ | ✅ | Scroll 模式已实现 |
| 鼠标滚轮翻页 | ✅ | ❌ | 滚轮仅用于滚动 |
| 键盘翻页（PgUp/PgDn） | ✅ | ✅ | 通过 egui 默认支持 |
| 鼠标拖拽平移 | ✅ | ❌ | 拖拽被文字选择占用 |
| 触摸屏手势 | ✅ | ❌ | 无触摸支持 |

**差距摘要：** 缺少全屏、演示模式、双页、RTL 等进阶布局功能。

### 2.3 目录与书签

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 目录树（ToC） | ✅ | ✅ | 从 PDF outline 解析，点击跳转 |
| 用户书签 | ✅ | ✅ | 添加/删除/跳转，存在 SQLite 表但未持久化到 UI |
| 书签导入/导出 | ✅ | ❌ | 无 |
| 最近打开文件 | ✅ | ✅ | 内存中最多 10 个，不持久化 |
| 文件历史记录 | ✅ | ❌ | 无会话恢复 |
| 收藏夹 | ✅ | ❌ | 无 |

**差距摘要：** 基础书签已有，但书签数据未真正写入数据库（UI 层的内存操作与 SQLite 表之间无连接）。缺少收藏夹和历史记录持久化。

### 2.4 搜索

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 全文搜索 | ✅ | ✅ | 侧边栏搜索框，匹配计数 |
| 大小写敏感切换 | ✅ | ❌ | 默认不敏感 |
| 正则搜索 | ✅ | ❌ | 无 |
| 匹配高亮（在页面上） | ✅ | ❌ | 仅搜索结果列表，无页面覆盖层 |
| 搜索结果列表 | ✅ | ✅ | 显示匹配数，▲▼ 导航 |
| 增量搜索 | ✅ | ❌ | 无 |
| 搜索索引（FTS5） | ✅ | ⚠️ | SQLite 有 search_index 表，使用 LIKE 而非 FTS5 |

**差距摘要：** 基础搜索有，但缺少页面高亮、正则、增量搜索等进阶功能。全文搜索引擎未使用 FTS5。

### 2.5 文字选择与复制

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 拖选文字 | ✅ | ✅ | PDF 拖选，蓝色高亮 |
| Ctrl+C 复制 | ✅ | ✅ | shell.rs 快捷键 |
| 右键菜单复制 | ✅ | ✅ | context_menu |
| 单词选择高亮 | ✅ | ✅ | 蓝色半透明覆盖层 |
| EPUB/TXT 文字选择 | ✅ | ✅ | 使用 egui 原生 selectable(true) |
| 选择文字搜索（Ctrl+E） | ✅ | ❌ | 无"用选中内容搜索" |
| 多列选择 | ✅ | ❌ | 无 |
| 文本层开关 | ✅ | ❌ | 无 |

**差距摘要：** 基础文字选择功能接近完备。缺少跨文档搜索等进阶特性。

### 2.6 标注系统

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 高亮 | ✅ | ⚠️ | UI 有工具选择，无实际 PDF 标注写入 |
| 下划线 | ✅ | ❌ | 无 |
| 删除线 | ✅ | ❌ | 无 |
| 波浪线 | ✅ | ❌ | 无 |
| 手绘笔（Freehand） | ✅ | ⚠️ | `stroke_points` 已收集，未渲染到 PDF |
| 注释（Note/贴纸） | ✅ | ⚠️ | UI 有笔图标，未实现 |
| 文本框 | ✅ | ❌ | 无 |
| 箭头/形状 | ✅ | ❌ | 无 |
| 标注列表/管理 | ✅ | ❌ | 无标注面板 |
| 标注导出（PDF 标准） | ✅ | ❌ | 无 |
| 标注撤销/清除 | ✅ | ✅ | UI 有撤销/清除按钮 |
| 标注工具选择 | ✅ | ✅ | 工具栏 4 个工具图标 |

**差距摘要：** 标注是最严重的差距之一。Folix 的标注停留在 UI 层面（有工具选择器和按钮），但没有真正的 PDF 标注写入引擎。SumatraPDF 使用 mupdf 的页对象 API 直接在 PDF 上创建标注。

### 2.7 设置与配置

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 设置持久化 | ✅ (sumatra-settings.txt) | ❌ | Folix 设置仅内存，重启丢失 |
| 背景色设置 | ✅ | ✅ | Settings 选项卡有颜色选择器 |
| 工具栏图标大小 | ✅ | ✅ | 滑块 12–32 |
| 工具栏显示/隐藏 | ✅ | ✅ | Settings 选项卡勾选框 |
| 缩放记忆 | ✅ | ❌ | 无 |
| 窗口位置/大小记忆 | ✅ | ❌ | 无 |
| 上次阅读位置恢复 | ✅ | ❌ | progress 表存在但未使用 |
| 默认字体设置 | ✅ | ❌ | 无 |
| 快捷键绑定自定义 | ✅ | ❌ | 无 |
| 语言设置 | ✅ (多国语言) | ❌ | 无 |
| 更新检查 | ✅ | ❌ | 无 |
| 便携模式 | ✅ | ❌ | 无 |
| 外部查看器关联 | ✅ | ❌ | 无 |

**差距摘要：** Folix 设置仅 3 项（图标大小、工具栏可见性、背景色），均不持久化。SumatraPDF 有 60+ 项设置，持久化到文件。

### 2.8 打印

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 打印（Ctrl+P） | ✅ | ❌ | 无打印支持 |
| 打印预览 | ✅ | ❌ | 无 |
| 页面范围选择 | ✅ | ❌ | 无 |
| 缩放/适合页面 | ✅ | ❌ | 无 |

**差距摘要：** 完全缺失。egui 本身不支持打印，需要操作系统级打印 API。

### 2.9 高级功能

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 自动翻页（幻灯片模式） | ✅ | ⚠️ | 有 auto-play（Page Flow/Glyph Reveal/Sentence Stream），但 Glyph Reveal 和 Sentence Stream 暂无实现 |
| 自动翻页速度调节 | ✅ | ✅ | 0.5x–5.0x 滑块 |
| 提取文本 | ✅ | ❌ | 无导出功能 |
| 提取图片 | ✅ | ❌ | 无 |
| 裁剪页面空白 | ✅ | ❌ | 无 |
| 反转颜色（夜间模式） | ✅ | ❌ | 无 |
| 对比度/亮度调节 | ✅ | ❌ | 无 |
| 物理页面编号 vs 逻辑页码 | ✅ | ❌ | 无 |
| PDF 表单填写 | ✅ | ❌ | 无 |
| 电子签名 | ✅ | ❌ | 无 |
| 文件重加载（外部修改） | ✅ | ❌ | 无 |
| 清除页面缓存 | ✅ | ❌ | 无 |
| 命令行参数支持 | ✅ | ❌ | 无 |

**差距摘要：** 自动翻页有 UI 和基础计时框架但子模式未实现。夜间模式、裁剪、图片提取等完全缺失。

### 2.10 UI/UX

| 功能 | SumatraPDF | Folix | 备注 |
|------|-----------|-------|------|
| 多标签页 | ✅ | ✅ | TabBar 已实现 |
| 标签页顺序拖拽 | ✅ | ❌ | 无 |
| 标签页缩略图 | ✅ | ❌ | 无 |
| 侧边栏（目录+搜索+书签） | ✅ | ✅ | 已实现 |
| 侧边栏开关 | ✅ | ✅ | 📑 按钮 |
| 菜单栏 | ✅ | ✅ | 文件/模式/帮助 |
| 右键上下文菜单 | ✅ | ✅ | 页面右键→复制 |
| 状态栏 | ✅ | ⚠️ | 仅底部工具栏，无页码/文件名状态显示 |
| 文件拖放打开 | ✅ | ✅ | 已实现 |
| 快捷键大全 | 50+ 快捷键 | 仅 Ctrl+C/Tab | 严重不足 |
| 多显示器支持 | ✅ | ⚠️ | egui 原生支持但未适配 |
| 高 DPI 支持 | ✅ | ✅ | egui 原生 |
| 触摸友好 | ✅ | ❌ | 无 |
| 页面缩略图预览 | ✅ | ❌ | 无 |
| 字体反走样设置 | ✅ | ❌ | 无 |

**差距摘要：** UI 框架虽然能用，但快捷键严重不足，缺少标签拖拽、缩略图、触摸支持等体验特性。

---

## 三、架构对比

### 3.1 引擎层

| 方面 | SumatraPDF | Folix |
|------|-----------|-------|
| 引擎架构 | `EngineBase` 虚基类 → `EnginePDF`/`EngineEpub`/`EngineMobi`/`EngineDjVu`/`EngineImage`/`EngineChm`/`EngineHtml` | `Document` trait → `PdfDocument`/`ReflowDocument` |
| 引擎注册表 | `EngineManager` 根据扩展名自动匹配 | `DocumentManager::open()` 手动匹配 |
| 页面渲染 | 所有引擎实现 `RenderPage()` → `dib` (GDI bitmap) | `render_page() → RenderedPage { width, height, rgba }` |
| MuPDF 实例管理 | 每个文档打开一次 `fz_context`，引擎整个生命周期复用 | PDF 每帧重新调用 `mupdf::Document::open()` (通过 drop_handle 模式) |
| 引擎数量 | 7 种不同类型的引擎 | 2 种 |
| 文本提取 | 所有引擎实现 `GetTextInRect()` + 平坦 WCHAR 数组 | `page_text_positions()` — Vec of WordPosition { text: String, rect } |

**关键差距：**
1. **MuPDF 上下文复用** — SumatraPDF 在引擎生命周期内保持 `fz_context` 打开。Folix 每次调用 `open()` → `drop()` 创建和销毁，虽然 Send+Sync 安全但浪费。
2. **引擎注册表** — Sumatra 的 `EngineManager` 支持自动扩展，Folix 的 `DocumentManager` 需要手动 case。
3. **文本存储模型** — Sumatra 用平坦 WCHAR 数组（连续内存，可 mmap），Folix 用 `Vec<TextWordPosition>`（每个 word 一个 `String` 堆分配）。

### 3.2 渲染管线

| 方面 | SumatraPDF | Folix |
|------|-----------|-------|
| 渲染目标 | GDI `HDC` / DirectWrite | wgpu 纹理 → egui `ColorImage` |
| 页面缓存 | `RenderCache` — LRU，128 项 | `render_cache: HashMap` — 2 项 + `texture_handles: HashMap` — 10 项 |
| 缓存键 | `(page, zoom, rotation, renderflags)` | `(page, scale_bits)` |
| 缓存值 | 压缩位图 (`dib`) | GPU 纹理句柄 `TextureHandle` |
| 显示列表 | ✅ — 每个页面预编译为紧凑 DisplayList | ❌ — 无显示列表，直接 RGBA 位图 |
| 增量更新 | ✅ — 仅重绘脏区域 | ❌ — 每帧全量重绘 |
| 异步渲染 | ✅ — 后台线程 + 前台合成 | ❌ — 主线程同步 |
| 双缓冲 | ✅ | ❌ (egui 框架层有) |
| 组合层（标注覆盖） | ✅ — 标注在显示列表层上叠加 | ❌ — `overlay_compositor` 是空壳 |
| 平铺缓存 | ✅ — 页内分块渲染 | ❌ — `tile_cache` 是空壳 |

**关键差距：**
1. **显示列表** — SumatraPDF 将 PDF 页面编译为紧凑的显示列表（操作码序列），渲染时快速回放。Folix 每次渲染都走完整的 MuPDF `to_pixmap()` 路径。
2. **缓存大小** — 128 项 vs 2–10 项。我们在小文档上够用，但大文档（1000+ 页）的页面切换会产生大量重渲染。
3. **异步渲染** — Sumatra 在后台线程渲染，UI 线程仅做合成。Folix 所有渲染在主线程帧内完成。
4. **标注合成** — Sumatra 的标注作为独立显示列表叠加在页面之上。Folix 的标注层是空壳。

### 3.3 文字模型

| 方面 | SumatraPDF | Folix |
|------|-----------|-------|
| 存储格式 | 平坦数组：`WCHAR* chars + RectI* rects + int len` | `Vec<TextWordPosition>` — 每个元素含 `String` |
| 内存布局 | 连续内存块，可利用虚拟内存 | 多个堆分配（每个 String 单独分配） |
| 对齐信息 | `RectI`：相对于页面的 bbox | `(x0,y0,x1,y1)`：相对于页面的 f32 rect |
| 字符级 vs 词级 | 字符级 | 词级（由 mupdf `page.words()` 决定） |
| 搜索匹配 | 字符级线性扫描 | 字符串包含匹配 |
| 缓存策略 | `TextCache` + `PageText` 按需加载 | `text_positions_cache: HashMap<usize, Vec<TextWordPosition>>` |
| 复制路径 | `GetTextInRect()` → 平坦缓冲区 → 系统剪贴板 | 遍历 `selected_word_indices` → `String` join → `ctx.copy_text()` |

**关键差距：**
1. **字符级 vs 词级** — SumatraPDF 的字符级模型支持更精确的选择范围（双击选词、三击选行）。Folix 的词级模型只能选择整个词。
2. **内存效率** — 每个词一个 `String`（至少 24 字节 + 堆分配）远不如连续 `WCHAR[]` 数组高效。大文档时内存开销显著。
3. **搜索精度** — 字符级模型支持部分匹配高亮。Folix 词级模型只能匹配完整词。

### 3.4 UI 与命令系统

| 方面 | SumatraPDF | Folix |
|------|-----------|-------|
| 架构 | 基于消息/Win32 窗口过程 | egui 立即模式（immediate mode） |
| 命令系统 | `menu::MenuItem` 树 + `cmd::Command` enum (120+ 命令) | 无中央命令系统，直接调用函数 |
| 快捷键绑定 | 集中式 `accel::AccelTable` | 分散在 `shell.rs:update()` 的按键检查 |
| 工具栏定义 | 声明式 `ToolbarItem` 数组 | 手动布局 `ui.horizontal()` |
| 菜单定义 | 声明式 `MenuDef` 嵌套结构 | 手动 `ui.menu_button()` 调用 |
| 主题/皮肤 | ✅ — 自定义渲染的深色主题 | ❌ — 仅有背景色设置 |
| 工具栏自定义 | ✅ | ❌ — 固定的工具栏布局 |
| 状态栏 | ✅ — 页码/文件名/缩放百分比 | ⚠️ — 仅在 toolbar row 1 右端显示页码 |
| 语言文件 | `.lang` 文件 (48 种语言) | ❌ — 硬编码中文 |
| 插件 API | ✅ — `plugin-api` DLL 接口 | ❌ — 无插件系统 |
| 外部工具集成 | ✅ — 调用外部编辑器 | ❌ — 无 |

**关键差距：**
1. **命令系统** — SumatraPDF 的中央命令系统是架构核心，支持撤销/快捷键绑定/菜单联动。Folix 直接调用函数，难以统一管理。
2. **快捷键** — 120+ 命令对应 50+ 快捷键，Folix 仅 Ctrl+C。
3. **插件系统** — SumatraPDF 的 DLL 插件 API 支持扩展，Folix 无此概念。

### 3.5 设置系统

| 方面 | SumatraPDF | Folix |
|------|-----------|-------|
| 存储格式 | 纯文本 `sumatra-settings.txt` | 内存 `AppSettings` struct |
| 持久化 | 手动读写文本文件 | 无 |
| 值类型 | int/bool/string/color/rect/fontdesc 等 | 仅 f32/bool/[u8;4] |
| 设置项数量 | 60+ | 3 |
| 分层默认值 | 编译期默认值 + 用户覆盖值 | 仅有编译期默认值 |
| 全局/本地设置 | 全局设置 + 每个文档覆盖 | 无 |
| 设置 UI | 自动生成设置页 | 手动 `egui::Slider` / `Checkbox` / `color_edit` |
| 同步设置 | `DmSnap` + `RestoreSnap` — 随文档保存/恢复 | 无 |
| 便携模式 | 设置文件在 exe 所在目录 | 不适用（未打包） |

**关键差距：** 设置系统是最基础的差距之一。没有持久化意味着每次重启丢失阅读位置、缩放、主题等所有偏好。

### 3.6 RenderCache 实现细节（代码审计）

SumatraPDF 的 `RenderCache`（`src/RenderCache.h/.cpp`，~1300 行）是核心渲染调度器：

- **线程池**：最多 32 个渲染线程（`kMaxRenderThreads`），延迟创建
- **请求队列**：最大 8 个待处理请求（`MAX_PAGE_REQUESTS`），用信号量调度
- **Bitmap 缓存**：LRU，最多 128 项（`MAX_BITMAPS_CACHED`）
- **Tile 切分**：`TilePosition { res, row, col }` — 四叉树风格分级，根据缩放级别自动选择分辨率。大页面被渐进加载
- **预测渲染**：当前页完成后自动预渲染后续 4 页（`kMaxPredictiveRequests`），如果来源页不再可见则停止

**渲染流程：**
```
Canvas::OnPaintDocument()
  → DrawDocument()
    → 对每个可见页: RenderCache::Paint() → 查缓存
      → 命中 → PaintTile() → BitBlt/StretchBlt 合成到画布
      → 未命中 → RequestRendering() → 加入队列 → 信号量
        → 工作线程: GetNextRequest() → EngineBase::RenderPage() → RenderedBitmap → Add() 入缓存 → 回调
```

### 3.7 三重锁设计（EngineMupdf）

SumatraPDF 的 MuPDF 引擎用三个 CRITICAL_SECTION 实现线程安全：

```
pagesLock → renderLock → docLock  (锁定顺序，防止死锁)
```

- `pagesLock`：保护每一页的 `fz_page*` 指针数组（页面粒度）
- `renderLock`：保护 MuPDF 的 `fz_display_list` 缓存（跨页面共享）
- `docLock`：保护 `fz_document` 的全局状态（MuPDF 的 image store 等全局资源）

Folix 的 `Arc<Mutex<DocumentHandle>>` 是单个全局锁，无分层。

### 3.8 HtmlFormatter 管线（重排版）

Reflowable 格式（EPUB/MOBI/FB2/TXT）走 `EngineEbook`：

```
EngineEbook::RenderPage()
  → EbookDoc::GetHtmlData()         — 提取/转换为 HTML 字符串
  → HtmlFormatter::Next()           — 格式化为 HtmlPage（DrawInstr 指令列表）
    → HtmlPullParser 分词 HTML
    → CSS 解析器提取样式规则
    → GDI+ Graphics 做字体测量
    → 行对齐 + 断行
    → 生成 DrawInstr { String, Image, SetFont, Line, LinkStart, LinkEnd, ... }
  → DrawHtmlPage()                  — DrawInstr 列表 → GDI+ Bitmap → RenderedBitmap
    → mui::ITextRender（GDI+ 或 GDI 文本渲染）
    → 图片解码: FzImgReader 或 Windows WIC
```

### 3.9 双重控制器架构

SumatraPDF 使用 `DocController` 接口处理两种文档类型：

- **DisplayModel** — 固定布局文档（PDF/DjVu/图片）：管理 `PageInfo[]`、缩放、滚动、页码、`DisplayMode`（Single/Facing/Book/Continuous）
- **ChmModel** — CHM 文档：委托给 MSHTML WebBrowser 控件

两者都实现 `DocController`，UI 层通过统一接口调度。

---

## 四、优先级路线图

### P0 — 基础体验（1–2 周）

这些功能缺失严重影响日常使用，应优先实现。

| # | 功能 | 文件/位置 | 预估工作量 | 说明 |
|---|------|-----------|-----------|------|
| 1 | **设置持久化** | `app_state.rs` + `storage/sqlite.rs` | 2 天 | 将 `AppSettings` 序列化到 SQLite `settings` 表，启动时读取 |
| 2 | **阅读位置恢复** | `shell.rs` + `storage/sqlite.rs` | 1 天 | 打开文件时从 `progress` 表恢复页码/滚动位置 |
| 3 | **全屏模式 (F11)** | `shell.rs` + eframe `WindowBuilder` | 1 天 | 切换无边框全屏，隐藏菜单栏/工具栏 |
| 4 | **键盘快捷键扩展** | `shell.rs:update()` | 2 天 | 实现 Ctrl+O/Ctrl+W/PgUp/PgDn/Home/End/+/-/Esc 等核心快捷键 |
| 5 | **"转到页码"对话框** | `shell.rs` / 新 dialog | 1 天 | 简单的弹出式输入框 + 跳转 |
| 6 | **数据库集成到 UI** | `shell.rs` → `sqlite.rs` | 1 天 | 初始化 Database 实例，在打开/关闭/翻页/书签等操作时写入 |

**总计：** ~8 天

### P1 — 功能补全（2–3 周）

补齐常见阅读器的标配有但 Folix 缺失的功能。

| # | 功能 | 文件/位置 | 预估工作量 | 说明 |
|---|------|-----------|-----------|------|
| 7 | **夜间模式/深色主题** | `app_state.rs` + `mode_ui.rs` | 2 天 | 增加反转颜色或背景色/前景色独立设置，实时切换 |
| 8 | **PDF 标注持久化** | `pdf_engine.rs` + `annotation/*` | 4 天 | 使用 mupdf 的 `PdfDocument` 创建真正的 PDF 标注（高亮/手绘/注释） |
| 9 | **页面搜索高亮** | `mode_ui.rs:render_image_page()` | 2 天 | 在页面位图上画出匹配文字的高亮矩形 |
| 10 | **适合宽度/适合页面** | `mode_ui.rs` | 1 天 | 计算缩放值使页面宽度或高度适应视口 |
| 11 | **双页显示** | `mode_ui.rs` | 3 天 | Paged 模式下同时渲染两页，封面单独处理 |
| 12 | **缩放百分比显示 + 自定义输入** | `shell.rs` toolbar | 1 天 | 在缩放滑块旁显示当前缩放值（如 "125%"） |
| 13 | **MRU 持久化** | `shell.rs` + `sqlite.rs` | 1 天 | 最近文件列表写入 SQLite，启动时恢复 |
| 14 | **会话恢复** | `shell.rs` + `sqlite.rs` | 2 天 | 退出时保存打开的所有标签页，下次启动恢复 |
| 15 | **快捷键绑定声明式系统** | 新 `keybinds.rs` | 3 天 | 参考 SumatraPDF 的 `accel::AccelTable` 集中管理所有快捷键 |

**总计：** ~19 天

### P2 — 架构改进（3–4 周）

重构核心架构以支持更多格式和更好性能。

| # | 功能 | 文件/位置 | 预估工作量 | 说明 |
|---|------|-----------|-----------|------|
| 16 | **引擎注册表** | `engines/mod.rs` | 2 天 | 将 `DocumentManager::open()` 改为注册表模式，按扩展名自动匹配引擎 |
| 17 | **MuPDF 上下文复用** | `pdf_engine.rs` | 3 天 | 保持 `fz_context` 在 PdfDocument 生命周期内打开，减少重复开销 |
| 18 | **异步渲染** | `mode_ui.rs` + 新 render thread | 5 天 | 后台线程渲染，前台线程从缓存读取；需要处理 Send+Sync 约束 |
| 19 | **渲染缓存 LRU** | 替换 `render_cache` | 2 天 | 参考 SumatraPDF 的 `RenderCache` 实现固定大小 LRU（128 项） |
| 20 | **平坦文字模型** | `engines/mod.rs` | 4 天 | 将 `TextWordPosition` 改为字符级连续数组，减少堆分配 |
| 21 | **中央命令系统** | 新 `commands/mod.rs` | 5 天 | `Command` enum + `CommandTarget` trait，统一快捷键/菜单/工具栏调度 |
| 22 | **标注合成层** | `render/overlay_compositor.rs` | 3 天 | 在渲染管线中叠加标注显示列表（而非修改页面位图） |

**总计：** ~24 天

### P3 — 新增格式支持（可选，按需求排序）

| # | 格式 | 引擎策略 | 预估工作量 | 说明 |
|---|------|---------|-----------|------|
| 23 | **CBZ/CBT** | 图片归档引擎（zip 解压 → image crate 解码） | 3 天 | 最简单的格式，适合作为新引擎模板 |
| 24 | **FB2** | XML 解析引擎（serde/xml 反序列化） | 3 天 | FB2 是 XML 格式，类似 EPUB 但更简单 |
| 25 | **MOBI** | 使用第三方 crate 或分叉 C 库 | 5–10 天 | 最复杂的格式，有专有编码 |
| 26 | **DjVu** | DjVuLibre 绑定或子进程 | 10 天+ | C++ 库，绑定工作量大 |
| 27 | **HTML** | 使用 `html2text` 或 `select.rs` 提取 | 2 天 | 仅提取文本（不渲染 CSS） |
| 28 | **图片格式支持** | `image` crate 通用解码器 | 2 天 | 支持 JPEG/PNG/GIF/WebP/BMP/TIFF 直接打开 |

### P4 — 进阶与体验（长期）

| # | 功能 | 说明 |
|---|------|------|
| 29 | 打印（操作系统级 API） | `print` crate 或系统打印对话框 |
| 30 | 文字提取/导出 | 将 `page_text()` 导出为 TXT 或标记格式 |
| 31 | 图片提取 | PDF 中提取内嵌图片 |
| 32 | PDF 表单填写 | mupdf 的 `PdfWidget` API |
| 33 | 触摸手势 | 缩放、翻页、双击适应 |
| 34 | 标签页拖拽排序 | egui 无原生支持，需实现拖放 |
| 35 | 侧边栏缩略图 | 在 ToC 上方加小型页面预览 |
| 36 | 命令行参数 | clap 解析器，支持 `folix file.pdf` |
| 37 | 插件 API | WASM 或 DLL 扩展系统 |
| 38 | 多语言支持 | `fluent-rs` 或纯文本语言文件 |
| 39 | 便携模式 | 设置文件/数据库在可执行文件同目录 |
| 40 | RTL 阅读模式 | 参考 SumatraPDF Manga 模式 |

---

## 五、关键技术债清单

### 5.1 死代码/未使用代码

这些模块已创建但未被实际使用，建议清理或整合：

| 文件 | 行数 | 状态 | 建议 |
|------|------|------|------|
| `auto_reading/*` (3 文件) | 25 | 空壳 | 移除或将逻辑从 `mode_ui.rs` 移入 |
| `annotation/*` (3 文件) | 17 | 空壳 | 实现标注时再填充 |
| `layout/*` (3 文件) | 21 | 空壳 | 移除 |
| `services/*` (4 文件) | 40 | 空壳 | 实现搜索注册时再填充 |
| `interaction/*` (2 文件) | 51 | 未调用 | 移除或将输入处理集中化 |
| `render/wgpu_renderer.rs` | 13 | 空壳 | 移除 |
| `render/tile_cache.rs` | 28 | 未使用 | 移除或集成 |
| `render/overlay_compositor.rs` | 10 | 空壳 | 移除 |
| `storage/library_index.rs` | 13 | 空壳 | 移除 |
| `storage/feature_store.rs` | 13 | 空壳 | 移除 |
| `text_renderer.rs` | 引用于 AGENTS.md 但不存在 | — | 更新文档或删除引用 |
| `platform/fs.rs` | 11 | 换行 | 删除，直接用 std::fs |

**总计死代码：** ~230 行，占代码库约 8%

### 5.2 框架锁定问题

- **egui 限制了多线程渲染** — 所有绘制必须在主线程。异步渲染需要渲染→纹理的"传送"机制。
- **立即模式 UI 限制** — 难以实现标签拖拽、动画过渡等保留模式 UI 特性。
- **跨平台打印** — egui 本身无打印支持。

---

## 六、推荐路线

### 短期（1 个月）
```
P0 项（1–6） → 基础体验达标
P1 项（7–9） → 夜间模式 + 标注持久化 + 搜索高亮
```

### 中期（2–3 个月）
```
P1 项（10–15） → 布局/快捷键/会话恢复
P2 项（16–18） → 引擎注册表 + MuPDF 复用 + 异步渲染
P3 项（23–24） → CBZ + FB2 格式支持
```

### 长期（6 个月+）
```
P2 项（19–22） → LRU 缓存 + 平坦文字 + 命令系统 + 标注合成
P4 项（29–40） → 打印/触摸/插件/多语言
```

---

*报告生成日期：2026-06-29*
*对比基准：SumatraPDF 3.7 (Windows)*
