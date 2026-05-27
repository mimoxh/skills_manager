import { invoke } from "@tauri-apps/api/core";
import type {
  AgentProfile,
  ConflictPolicy,
  DiscoveryPathEntry,
  GroupedSkill,
  ImportSkillFile,
  ImportSkillResult,
  InstallResult,
  InstallState,
  SkillSummary,
  SyncCandidate,
} from "./types";

const fallbackRepositoryKey = "skills-manager.repository";
const fallbackRepository = "C:\\Users\\you\\skills";

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function getFallbackRepository() {
  return localStorage.getItem(fallbackRepositoryKey) ?? fallbackRepository;
}

function command<T>(name: string, args: Record<string, unknown>, fallback: () => T | Promise<T>) {
  if (hasTauriRuntime()) {
    return invoke<T>(name, args);
  }
  return Promise.resolve(fallback());
}

export const api = {
  setRepository(path: string) {
    return command("set_repository", { path }, () => {
      const repository = path.trim() || getFallbackRepository();
      localStorage.setItem(fallbackRepositoryKey, repository);
      return repository;
    });
  },
  getRepository() {
    return command("get_repository", {}, getFallbackRepository);
  },
  scanSkills() {
    return command<SkillSummary[]>("scan_skills", {}, () => []);
  },
  scanAgentSkills() {
    return command<GroupedSkill[]>("scan_agent_skills", {}, () => []);
  },
  importSkillUpload(fileName: string, files: ImportSkillFile[]) {
    return command<ImportSkillResult>("import_skill_upload", { fileName, files }, () => ({
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
  listInstallState() {
    return command<InstallState[]>("list_install_state", {}, () => []);
  },
  previewSync(agentId: string) {
    return command<SyncCandidate[]>("preview_sync", { agentId }, () => []);
  },
  installSkills(skillIds: string[], agentIds: string[], conflictPolicy: ConflictPolicy) {
    return command<InstallResult[]>("install_skills", { skillIds, agentIds, conflictPolicy }, () => []);
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
  rollbackLast(agentId: string, skillId: string) {
    return command<void>("rollback_last", { agentId, skillId }, () => undefined);
  },
  addDiscoveryPath(path: string, label: string, skillsSubdir: string) {
    return command<void>("add_discovery_path", { path, label, skillsSubdir }, () => undefined);
  },
  removeDiscoveryPath(path: string) {
    return command<void>("remove_discovery_path", { path }, () => undefined);
  },
  listDiscoveryPaths() {
    return command<DiscoveryPathEntry[]>("list_discovery_paths", {}, () => []);
  },
};
