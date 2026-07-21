static ZH_CN: &[(&str, &str)] = &[
    // Main window
    ("Folix", "Folix"),

    // Menu bar
    ("File", "文件"),
    ("Open...", "打开..."),
    ("Close", "关闭"),
    ("Quit", "退出"),
    ("Mode", "模式"),
    ("Help", "帮助"),
    ("About Folix", "关于 Folix"),

    // Mode names
    ("Basic", "基础浏览"),
    ("Page Edit", "页面编辑"),
    ("Content", "内容"),

    // Layout
    ("Paged", "分页"),
    ("Scroll", "滚动"),

    // Tab titles
    ("⚙ Settings", "⚙ 设置"),
    ("📄 PDF Tools", "📄 PDF工具"),
    ("Untitled", "未命名"),
    ("+ New Tab", "+ 新标签"),

    // New tab page
    ("PDF / EPUB / TXT Reader", "PDF / EPUB / TXT 阅读器"),
    ("📂  Open File", "📂  打开文件"),
    ("📄  PDF Tools", "📄  PDF工具"),
    ("Recent Files", "最近文件"),
    ("No recent files", "暂无最近文件"),
    ("Open a file or drag-and-drop to get started.", "打开文件或拖拽文件到窗口以开始阅读。"),

    // Settings
    ("Appearance", "外观"),
    ("Toolbar Icon Size:", "工具栏图标大小："),
    ("Reader Background:", "阅读器背景："),
    ("Text Area Background:", "文字区背景："),
    ("Toolbars:", "工具栏："),
    ("📖 Nav  ◀▶ ▲▼", "📖 导航  ◀▶ ▲▼"),
    ("🔍 View  Zoom+Layout", "🔍 视图  缩放+布局"),
    ("Page", "页"),
    ("📄 Page", "📄 页码"),
    ("▶ Auto-read", "▶ 自动阅读"),
    ("✏ Page Edit", "✏ 页面编辑"),
    ("Dark Mode (Night)", "深色模式（夜间）"),
    ("Language", "语言"),
    ("简体中文", "简体中文"),
    ("English", "English"),
    ("Scrolling", "滚动"),
    ("Scroll Speed (px/s):", "滚动速度（像素/秒）："),
    (" px/s", " 像素/秒"),
    ("Keyboard Shortcuts", "键盘快捷键"),
    ("Click a shortcut row to edit its key binding.", "点击快捷键行以编辑按键绑定。"),
    ("Action", "操作"),
    ("Key", "按键"),
    ("Shift", "Shift"),
    ("Alt", "Alt"),
    ("(unset)", "（未设置）"),
    ("Reset Shortcuts to Default", "重置快捷键为默认"),
    ("Save Config Now", "立即保存配置"),
    ("Config saved", "配置已保存"),

    // Toolbar
    ("Zoom", "缩放"),
    ("Speed:", "速度："),
    ("↻ CW", "↻ 右转"),
    ("↻ CCW", "↻ 左转"),
    ("Del", "删除"),
    ("+ Page", "+ 页面"),
    ("A-", "A-"),
    ("A+", "A+"),
    ("B", "B"),
    ("I", "I"),

    // Sidebar
    ("Sidebar", "侧边栏"),
    ("📖 Table of Contents", "📖 目录"),
    ("No table of contents", "无目录"),
    ("🔍 Search", "🔍 搜索"),
    ("Search text...", "搜索文本..."),
    ("0 matches", "0 个匹配"),
    ("🔖 Bookmarks", "🔖 书签"),
    ("+ Add Bookmark", "+ 添加书签"),

    ("Save", "保存"),

    // PDF operations
    ("PDF Operations", "PDF 操作"),
    ("INPUT", "输入"),
    ("📂 Add Files", "📂 添加文件"),
    ("No files selected", "未选择文件"),
    ("CONVERT", "转换"),
    ("Merge PDFs", "合并PDF"),
    ("Split PDF", "拆分PDF"),
    ("Extract Images", "提取图片"),
    ("Extract Text", "提取文本"),
    ("Image(s) → PDF", "图片 → PDF"),
    ("Split by:", "拆分方式："),
    ("Page range", "页码范围"),
    ("Every N pages", "每N页一份"),
    ("TOC chapters", "按目录章节"),
    ("From:", "从："),
    ("To:", "到："),
    ("Pages per chunk:", "每份页数："),
    ("chapters found", "个章节"),
    ("No TOC data. Select a PDF first.", "无目录数据。请先选择PDF文件。"),
    ("Each page is exported as a separate PNG.", "每个页面将导出为单独的PNG文件。"),
    ("All pages from the input PDF will be extracted.", "将提取输入PDF中的所有页面。"),
    ("Extracts all text from the PDF into a .txt file.", "将PDF中的所有文本提取到.txt文件中。"),
    ("Select at least 2 PDF files in the input panel.", "在输入面板中选择至少2个PDF文件。"),
    ("They will be merged in order.", "它们将按顺序合并。"),
    ("Select one or more images.", "选择一个或多个图片。"),
    ("They will be combined into a single PDF, one per page.", "它们将合并为一个PDF，每页一张图片。"),
    ("OUTPUT", "输出"),
    ("Folder:", "文件夹："),
    ("Browse...", "浏览..."),
    ("Preview:", "预览："),
    ("⏳ Running…", "⏳ 执行中…"),
    ("▶ Execute", "▶ 执行"),
    ("LOG", "日志"),
    ("Clear Log", "清除日志"),
    ("No operations yet.", "暂无操作。"),

    // Status messages
    ("Opened:", "已打开："),
    ("Failed to open:", "打开失败："),
    ("Saved:", "已保存："),
    ("Reloaded:", "已重新加载："),
    ("Failed to reload:", "重新加载失败："),

    // Info
    ("Config file: ./folix.conf", "配置文件：./folix.conf"),

    // Shortcut action labels
    ("Open File", "打开文件"),
    ("Close Tab", "关闭标签"),
    ("Quit", "退出"),
    ("Reload", "重新加载"),
    ("Zoom In", "放大"),
    ("Zoom Out", "缩小"),
    ("Fit Page", "适应页面"),
    ("Actual Size", "实际大小"),
    ("Fit Width", "适应宽度"),
    ("Next Page", "下一页"),
    ("Prev Page", "上一页"),
    ("First Page", "首页"),
    ("Last Page", "末页"),
    ("Scroll Down", "向下滚动"),
    ("Scroll Up", "向上滚动"),
    ("Highlight Selection", "高亮选中"),
    ("Add Bookmark", "添加书签"),
    ("Toggle Sidebar", "切换侧边栏"),
    ("Toggle UI", "切换界面"),
    ("Copy", "复制"),

    // About dialog
    ("PDF/EPUB Reader", "PDF/EPUB 阅读器"),
    ("Version:", "版本："),
    ("Built with egui + mupdf", "基于 egui + mupdf 构建"),
    // Vocabulary
    ("📝 Vocabulary", "📝 生词本"),

    // Sentences
    ("💬 Sentences", "💬 句子收藏"),

    // Context menu
    ("📝 Add to Vocabulary", "📝 加入生词本"),
    ("💬 Save Sentence", "💬 收藏句子"),

    // Add vocab dialog
    ("Add Vocabulary", "添加生词"),
    ("Word / Phrase:", "单词/短语："),
    ("+ Add", "+ 添加"),

    // View
    ("Fit Page", "适应页面"),
    ("Fit Width", "适应宽度"),
    ("Actual Size", "实际大小"),
    ("↻ Rotate View", "↻ 旋转视图"),
    ("↻ 90°", "↻ 90°"),
    ("↺ 90°", "↺ 90°"),

    // Menu
    ("Navigate", "导航"),
    ("Go to Page...", "跳转到页..."),
    ("Go to Page", "跳转到页"),
    ("Page number:", "页码："),
    ("Go", "跳转"),
    ("Tools", "工具"),
    // Tools
    ("Line Numbers", "行号"),

    // Reading settings
    ("Aa", "Aa"),
    ("Reading Settings", "阅读排版"),
    ("Font Size", "字体大小"),
    ("Line Height", "行间距"),
    ("Margin", "页边距"),
    ("Max Width", "最大宽度"),
];

pub fn tr(lang: &str, text: &'static str) -> &'static str {
    if lang != "zh-CN" {
        return text;
    }
    for (k, v) in ZH_CN {
        if *k == text {
            return v;
        }
    }
    text
}
