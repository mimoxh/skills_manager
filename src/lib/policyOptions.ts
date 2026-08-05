import type { ConflictPolicy } from "../types";

/** 冲突策略选项（skill 安装/导入共用）；MCP 场景用 mcpPolicyOptions（措辞不同）。 */
export const policyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新目标目录" },
  { value: "skip", label: "跳过冲突", helper: "目标已存在时不做修改" },
  { value: "rename", label: "另存副本", helper: "生成带时间戳的新副本" },
];
