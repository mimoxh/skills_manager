export type AgentType = "codex" | "claude" | "custom";
export type ConflictPolicy = "prompt" | "backupOverwrite" | "skip" | "rename";
export type InstallStatus = "installed" | "stale" | "conflict" | "missing";

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
}

export interface GroupedSkill {
  title: string;
  bestCopy: AgentSkillCopy;
  copies: AgentSkillCopy[];
  installedAgentIds: string[];
  missingAgentIds: string[];
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

export interface InstallState {
  agentId: string;
  skillId: string;
  status: InstallStatus;
  installedFingerprint?: string | null;
  targetFingerprint?: string | null;
  message: string;
}

export interface SyncCandidate {
  skill: SkillSummary;
  states: InstallState[];
}

export interface InstallResult {
  agentId: string;
  skillId: string;
  action: "installed" | "updated" | "skipped" | "renamed";
  targetPath: string;
  backupPath?: string | null;
  message: string;
}
