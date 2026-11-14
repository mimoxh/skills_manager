## Skills Manager v0.5.0

相比 `0.4.1`，本版本带来 **多主题界面、Skills 多设备同步、exe 旁统一中枢与外部 Agent 接入**，并完成一轮全量安全与并发修复。

### 全新主题与组件风格
- **7 个可随时切换的主题**：默认 Material 3 Expressive「钴蓝 · 杏橙」；另有 M3「莓果 · 湖青」「青柠 · 鸢尾」「深青 · 珊瑚」、Linear / Raycast，以及保留的「清爽极简」「经典暖色」。
- **主题同时改变配色与组件形态**：M3 主题使用胶囊按钮、按压圆角收缩、涟漪反馈、无描边色调卡片与色块导航；Linear 主题紧凑精密，发丝描边与 1px 顶部高光。
- **标题栏快捷切换**：右上角新增主题菜单，可直接切换主题和明暗模式；设置页按 Material 3 / Linear / 经典分组展示主题卡片。每个主题都支持浅色、深色与跟随系统。

### Skills 多设备同步
- 支持 **S3 兼容网关** 或 **本地目录（`local://`）** 作为同步端点，内容加密后上传；Secret Key 与加密口令保存在系统钥匙串，不写入 `state.json`。
- 冲突检测（`bothModified` / `deleteVsModify`）与解决（保留本地 / 采用远端 / 重命名）；标签与「无需全覆盖」标记按最新时间戳覆盖（LWW）。
- 提供 `sync_local_demo` 单机双设备演示与 `sync_` 集成测试。

### 统一中枢安装与分发
- 中枢固定为 `SkillsManager.exe` 旁的 `skills/`。所有安装、导入和同步都先进入中枢，可暂不选择 Agent，再由每台设备自行选择分发目标。
- 分发优先软链接，其次 Junction，最后复制；取消选择只移除可核实的管理器分发，Agent 自装的同名 Skill 会保留。
- 旧中枢只迁移能由安装记录和指纹确认的 Skill，原件保留。

### 外部 Agent 接入
- 内置 stdio MCP Server（`SkillsManager.exe mcp`）与命令行工具，Claude Code、Cursor、Windsurf 等 Agent 可远程管理 Skills 与 MCP；设置页可一键接入本机已检测到的 Agent。
- 支持从 GitHub 仓库、`/tree/...` 子目录链接或本地路径安装 Skill 与 MCP 配置。
- OpenCode 配置检测优化，内置 Agent 类型自动补全默认参数。

### 安全与稳定性修复
- 修复同步加密 Nonce 复用问题，区分内容寻址与清单加密。
- 修复 Git URL 参数注入，加强 zip 解压限制与路径安全校验。
- 修复跨进程 `state.json` 并发写入、软链接备份回滚、同步墓碑持久化与 GC 熔断等问题。
- 优化前端状态联动、Catalog 请求防覆盖与 MCP 表单交互。

### 兼容性
- 无需手动迁移；已有 Agent、MCP 配置、标签与主题偏好会保留。之前选择「清爽极简」或「经典暖色」的用户升级后保持原主题。
- 程序目录需允许当前用户写入，否则中枢创建与 Skill 安装会报错。

### 附件说明
- `SkillsManager.exe`：便携单文件版，适合本地快速验证与日常使用。
- `SkillsManager-v0.5.0-windows-portable.zip`：便携发布包（含说明文档）。
