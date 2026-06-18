# 项目目录大小分析报告

## 总览
- **项目总大小**: 27 GB
- **核心代码大小**: ~296 KB
- **可回收空间**: 25.67 GB (95%+)

---

## 目录分解

### 1. 可删除（临时/缓存文件）- 节省 25.67 GB

#### ❌ `src-tauri/target/` - 18 GB
- **性质**: Rust 编译产物目录
- **原因**: 包含所有编译的二进制文件、依赖库、临时文件
- **风险**: 删除后需要重新编译（`cargo build`）
- **建议**: 完全删除，这是最大的空间消耗

#### ❌ `.dev-logs/` - 7.5 GB  
- **性质**: 开发期间生成的日志文件
- **原因**: 保存了所有开发会话的日志、错误输出
- **风险**: 不影响项目功能，日志已过时
- **建议**: 完全删除，可以重新生成

#### ❌ `node_modules/` - 147 MB
- **性质**: npm 包依赖
- **原因**: 包含所有前端依赖包
- **风险**: 删除后需要 `npm install` 重新安装
- **建议**: 删除，可由 `package.json` 重新安装

#### ❌ `.npm-cache/` - 29 MB
- **性质**: npm 缓存目录
- **原因**: 下载过的包缓存
- **风险**: 不影响功能，删除后首次安装会稍慢
- **建议**: 删除，节省空间

---

### 2. 应保留（核心文件）- 保留 1.3 GB

#### ✅ `.git/` - 1.7 GB
- **性质**: 版本控制历史
- **原因**: 包含所有提交历史、分支信息
- **建议**: 必须保留，删除会失去版本历史

#### ✅ `src/` - 296 KB
- **性质**: 前端源代码
- **原因**: 项目核心代码，React/TypeScript 组件
- **建议**: 核心代码，必须保留

#### ✅ `src-tauri/src/` - 496 KB
- **性质**: Rust 后端源代码
- **原因**: Tauri 命令和业务逻辑
- **建议**: 核心代码，必须保留

#### ✅ 配置文件（各 4-124 KB）
- `package.json` - npm 配置
- `Cargo.toml` - Rust 依赖配置
- `tsconfig.json` - TypeScript 配置
- `vite.config.ts` - 构建配置
- `tauri.conf.json` - Tauri 配置
- **建议**: 必须保留

#### ✅ 文档文件（各 4-24 KB）
- `CLAUDE.md`, `AGENT.md`, `AGENTS.md` - 项目文档
- `README.md`, `README.zh-CN.md` - 说明文档
- **建议**: 必须保留

#### ✅ 构建产物 - 19 MB
- `SkillsManager.exe` - 14 MB（可执行文件）
- `dist-native/` - 14 MB（原生构建产物）
- `dist/` - 512 KB（Web 构建产物）
- `SkillsManager-v0.1.1-windows-portable.zip` - 5 MB
- **建议**: 如需发布可保留，否则可删除

---

### 3. 可选删除 - 节省 ~50 KB

#### ⚠️ 日志文件
- `gcm-diagnose.log` - 诊断日志（可删除）
- `.dev-logs/` 目录（上面已包含）

#### ⚠️ 临时文件
- `preview.html` - 预览文件（可删除，可重新生成）

---

## 清理命令

### 安全清理（推荐）
```bash
# 1. 删除 Rust 编译产物（节省 18 GB）
rm -rf src-tauri/target/

# 2. 删除开发日志（节省 7.5 GB）
rm -rf .dev-logs/

# 3. 删除 npm 缓存（节省 29 MB）
rm -rf .npm-cache/

# 4. 删除 node_modules（节省 147 MB）
rm -rf node_modules/

# 5. 清理根目录日志
rm -f gcm-diagnose.log
```

### 重新安装依赖
```bash
# 重新安装前端依赖
npm install

# 重新编译后端（如需要）
cd src-tauri && cargo build
```

---

## 清理后预期状态

- **清理前**: 27 GB
- **清理后**: ~2.0 GB
- **节省空间**: 25 GB (93%)

核心代码只有 ~1.3 MB，其余都是可再生的缓存和构建产物。

---

## 建议

1. **立即执行**: 删除 `.dev-logs/` 和 `src-tauri/target/`（25.5 GB）
2. **可选执行**: 删除 `node_modules/`（需要重新安装）
3. **建议添加到 .gitignore**:
   ```
   .dev-logs/
   .npm-cache/
   node_modules/
   src-tauri/target/
   *.log
   ```
4. **定期清理**: 每次发布前清理 `target/` 和 `node_modules/`

---

## 总结

项目膨胀的主要原因：
1. **Rust 编译缓存** (18 GB) - 最大消耗者
2. **开发日志** (7.5 GB) - 容易被忽视
3. **npm 依赖** (147 MB) - 正常但可再生

通过简单的清理命令可以回收 **95%+** 的空间，不影响项目功能。
