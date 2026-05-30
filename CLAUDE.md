# CLAUDE.md

## 项目规则

### Git 操作
- **提交和推送必须由用户确认后再执行**，不要自动 `git commit` 或 `git push`。
- **用户明确说"提交和推送"时可直接执行**，无需二次确认。
- 提交信息使用中文，格式遵循 Conventional Commits（如 `feat:`、`fix:`、`refactor:`）。
- 构建产物（`SkillsManager.exe`、`SkillsManager.zip`）应包含在提交中。

### 构建与发布
- 更新 exe/zip 时若检测到文件占用，直接用 `taskkill /F /IM SkillsManager.exe` 停止进程后继续更新，无需询问用户。
- 构建命令：`npm run native:build`，产物在 `src-tauri/target/release/`。
- 更新 exe 后需同步更新 zip：`powershell -Command "Remove-Item SkillsManager.zip -ErrorAction SilentlyContinue; Compress-Archive -Path SkillsManager.exe -DestinationPath SkillsManager.zip"`。

### 代码风格
- 前端：React 18 + TypeScript + Tailwind CSS 4，组件使用函数式写法。
- 后端：Rust 2021，serde 使用 camelCase 序列化。
- 指标卡片、badge 等 UI 文案使用中文。

### 文档同步
- 修改功能后需同步更新 `AGENT.md` 文档。
- `AGENT.md` 记录项目架构、API、数据模型等完整信息。

### 开发流程
- 后端改动后运行 `cargo test` 验证。
- 前端改动后运行 `npm run build` 验证 TypeScript 编译。
- 涉及 Tauri command 时，同步检查 `src/api.ts`、`src/types.ts`、`src-tauri/src/models.rs` 和 command 函数签名。
