import { invoke } from "@tauri-apps/api/core";
import type {
  AgentProfile,
  ConflictPolicy,
  ImportSkillFile,
  ImportSkillResult,
  InitialData,
  InstallResult,
} from "./types";

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function command<T>(name: string, args: Record<string, unknown>, fallback: () => T | Promise<T>) {
  if (hasTauriRuntime()) {
    return invoke<T>(name, args);
  }
  return Promise.resolve(fallback());
}

export const api = {
  getInitialData() {
    return command<InitialData>("get_initial_data", {}, () => ({
      skills: [],
      agents: [],
      noFullCoverageTitles: [],
    }));
  },
  importSkillUpload(fileName: string, files: ImportSkillFile[], targetAgentIds: string[], conflictPolicy: ConflictPolicy) {
    return command<ImportSkillResult>("import_skill_upload", { fileName, files, targetAgentIds, conflictPolicy }, () => ({
      imported: 0,
      skipped: 0,
      message: "Upload import is available in the desktop app",
    }));
  },
  detectAgents() {
    return command<AgentProfile[]>("detect_agents", {}, () => []);
  },
  listAgents() {
    return command<AgentProfile[]>("list_agents", {}, () => []);
  },
  addAgent(profile: AgentProfile) {
    return command("add_agent", { profile }, () => profile);
  },
  removeAgent(agentId: string) {
    return command<void>("remove_agent", { agentId }, () => undefined);
  },
  syncGroupedSkill(title: string, sourceAgentId: string | null | undefined, targetAgentIds: string[], conflictPolicy: ConflictPolicy) {
    return command<InstallResult[]>(
      "sync_grouped_skill",
      { title, sourceAgentId, targetAgentIds, conflictPolicy },
      () => [],
    );
  },
  uninstallSkill(skillId: string, agentId: string) {
    return command<void>("uninstall_skill", { skillId, agentId }, () => undefined);
  },
  uninstallSkillFromAgents(skillId: string, agentIds: string[]) {
    return command<void>("uninstall_skill_from_agents", { skillId, agentIds }, () => undefined);
  },
  rollbackLast(agentId: string, skillId: string) {
    return command<void>("rollback_last", { agentId, skillId }, () => undefined);
  },
  toggleNoFullCoverage(title: string) {
    return command<boolean>("toggle_no_full_coverage", { title }, () => false);
  },
};
