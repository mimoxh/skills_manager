import { invoke } from "@tauri-apps/api/core";
import type { AgentProfile, ConflictPolicy, InstallResult, InstallState, SkillSummary, SyncCandidate } from "./types";

export const api = {
  setRepository(path: string) {
    return invoke<string>("set_repository", { path });
  },
  getRepository() {
    return invoke<string | null>("get_repository");
  },
  scanSkills() {
    return invoke<SkillSummary[]>("scan_skills");
  },
  detectAgents() {
    return invoke<AgentProfile[]>("detect_agents");
  },
  listAgents() {
    return invoke<AgentProfile[]>("list_agents");
  },
  addAgent(profile: AgentProfile) {
    return invoke<AgentProfile>("add_agent", { profile });
  },
  removeAgent(agentId: string) {
    return invoke<void>("remove_agent", { agentId });
  },
  listInstallState() {
    return invoke<InstallState[]>("list_install_state");
  },
  previewSync(agentId: string) {
    return invoke<SyncCandidate[]>("preview_sync", { agentId });
  },
  installSkills(skillIds: string[], agentIds: string[], conflictPolicy: ConflictPolicy) {
    return invoke<InstallResult[]>("install_skills", { skillIds, agentIds, conflictPolicy });
  },
  uninstallSkill(skillId: string, agentId: string) {
    return invoke<void>("uninstall_skill", { skillId, agentId });
  },
  rollbackLast(agentId: string, skillId: string) {
    return invoke<void>("rollback_last", { agentId, skillId });
  },
};
