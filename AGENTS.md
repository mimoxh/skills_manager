# AGENTS.md

本文件只作为当前 `skills_manager` 项目的 agent 工作提示词，作用范围为本目录及其子目录。

## 文件操作限制

禁止批量删除文件或目录。

不要使用：

- `del /s`
- `rd /s`
- `rmdir /s`
- `Remove-Item -Recurse`
- `rm -rf`

需要删除文件时，只能一次删除一个明确路径的文件。

正确示例：

```powershell
Remove-Item "C:\path\to\file.txt"
```

如果需要批量删除文件，应停止操作，并向用户请求，让用户手动删除。

## Windows 编码

在 Windows PowerShell 中优先使用显式 UTF-8 读取校验文件内容。

## SiYuan 使用边界

`siyuan-mcp` 可以查询所有笔记本中的内容，但只能管理和操作（增删改）笔记本“AI”（ID: `20260531195756-ww623j6`）中的文档。

保存文档时，优先保存在 SiYuan 中。

## 发布产物

如果代码变动会影响应用行为或发布包，在完成变动并通过验证后，同步更新项目根目录的：

- `SkillsManager.exe`
- `SkillsManager-v*-windows-portable.zip`

优先使用项目已有的发布脚本生成发布产物，例如：

```powershell
.\scripts\build-portable.ps1
```
