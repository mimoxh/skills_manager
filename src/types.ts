export type AgentType = "codex" | "claude" | "claudeCode" | "cursor" | "trae" | "custom" | "cherryStudio" | "opencode";
export type ConflictPolicy = "prompt" | "backupOverwrite" | "skip" | "rename";

export interface SkillManifest {
  id: string;
  name: string;
  version: string;
  description?: string;
  tags?: string[];
  supportedAgents: string[];
  entry?: string;
  files: string[];
}

export interface SkillSummary {
  manifest: SkillManifest;
  sourcePath: string;
  fingerprint: string;
  manifestPath: string;
}

export interface AgentSkillCopy {
  agentId: string;
  agentName: string;
  skillPath: string;
  title: string;
  version?: string | null;
  fingerprint: string;
  updatedAt?: string | null;
  description?: string;
  readme?: string;
}

export interface GroupedSkill {
  title: string;
  bestCopy: AgentSkillCopy;
  copies: AgentSkillCopy[];
  installedAgentIds: string[];
  missingAgentIds: string[];
  description?: string;
  readme?: string;
}

export interface ImportSkillFile {
  relativePath: string;
  bytes: number[];
}

export interface ImportSkillResult {
  imported: number;
  skipped: number;
  message: string;
}

export interface AgentProfile {
  id: string;
  name: string;
  type: AgentType;
  skillsPath: string;
  adapterConfig?: Record<string, unknown> | null;
}

export interface InstallResult {
  agentId: string;
  skillId: string;
  action: "installed" | "updated" | "skipped" | "renamed";
  targetPath: string;
  backupPath?: string | null;
  message: string;
}

export interface InitialData {
  skills: GroupedSkill[];
  agents: AgentProfile[];
  noFullCoverageTitles: string[];
  noFullCoverageMcpTitles: string[];
}

// ── MCP 类型 ──────────────────────────────────────────────────────────

export type McpTransport = "stdio" | "http" | "sse";

export interface McpServerConfig {
  name: string;
  transport: McpTransport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  disabled?: boolean;
  timeoutSec?: number;
}

export interface AgentMcpServer {
  agentId: string;
  agentName: string;
  configPath: string;
  config: McpServerConfig;
  fingerprint: string;
  rawConfig?: string;
}

export interface GroupedMcpServer {
  name: string;
  copies: AgentMcpServer[];
  agentIds: string[];
  disabledAgentIds?: string[];
}

export interface McpOperationResult {
  agentId: string;
  serverName: string;
  action: string;
  message: string;
}
