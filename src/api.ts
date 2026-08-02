import { invoke } from "@tauri-apps/api/core";
import type {
  AgentProfile,
  CatalogFilters,
  CatalogRefreshResult,
  CatalogRefreshStatus,
  CatalogSafetyMode,
  CatalogSearchResult,
  CatalogSort,
  CatalogSource,
  ConflictPolicy,
  GroupedMcpServer,
  ImportSkillFile,
  ImportSkillResult,
  InitialData,
  InstallResult,
  McpOperationResult,
  McpServerConfig,
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
      noFullCoverageMcpTitles: [],
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
  repairClaudeCoworkManifest(agentId: string) {
    return command<ImportSkillResult>(
      "repair_claude_cowork_manifest",
      { agentId },
      () => ({
        imported: 0,
        skipped: 0,
        message: "Cowork manifest repair is available in the desktop app",
      }),
    );
  },
  toggleNoFullCoverage(title: string) {
    return command<boolean>("toggle_no_full_coverage", { title }, () => false);
  },
  toggleMcpNoFullCoverage(title: string) {
    return command<boolean>("toggle_no_full_coverage_mcp", { title }, () => false);
  },
  setSkillTags(title: string, tags: string[]) {
    return command<string[]>("set_skill_tags", { title, tags }, () => tags);
  },
  setAgentTags(agentId: string, tags: string[]) {
    return command<string[]>("set_agent_tags", { agentId, tags }, () => tags);
  },
  listCatalogSources() {
    return command<CatalogSource[]>("list_catalog_sources", {}, () => []);
  },
  saveCatalogSource(source: CatalogSource) {
    return command<CatalogSource>("save_catalog_source", { source }, () => source);
  },
  refreshCatalogSource(sourceId: string) {
    return command<CatalogRefreshResult>("refresh_catalog_source", { sourceId }, () => ({
      sourceId,
      refreshed: false,
      skillCount: 0,
      message: "Catalog refresh is available in the desktop app",
    }));
  },
  searchCatalogSkills(query: string, sort: CatalogSort, filters: CatalogFilters, page = 1, pageSize = 100) {
    return command<CatalogSearchResult>(
      "search_catalog_skills",
      { query, sort, filters, page, pageSize },
      () => ({ items: [], total: 0, page, pageSize, hasMore: false }),
    );
  },
  startCatalogRefresh(sourceId: string, safetyMode: CatalogSafetyMode) {
    return command<CatalogRefreshStatus>(
      "start_catalog_refresh",
      { sourceId, safetyMode },
      () => ({
        sourceId,
        safetyMode,
        isRunning: false,
        isComplete: false,
        fetchedCount: 0,
        nextCursor: null,
        generation: 0,
        lastError: "Desktop only",
        updatedAt: null,
      }),
    );
  },
  getCatalogRefreshStatus(sourceId: string, safetyMode: CatalogSafetyMode) {
    return command<CatalogRefreshStatus>(
      "get_catalog_refresh_status",
      { sourceId, safetyMode },
      () => ({
        sourceId,
        safetyMode,
        isRunning: false,
        isComplete: false,
        fetchedCount: 0,
        nextCursor: null,
        generation: 0,
        lastError: null,
        updatedAt: null,
      }),
    );
  },
  cancelCatalogRefresh(sourceId: string, safetyMode: CatalogSafetyMode) {
    return command<CatalogRefreshStatus>(
      "cancel_catalog_refresh",
      { sourceId, safetyMode },
      () => ({
        sourceId,
        safetyMode,
        isRunning: false,
        isComplete: false,
        fetchedCount: 0,
        nextCursor: null,
        generation: 0,
        lastError: "Desktop only",
        updatedAt: null,
      }),
    );
  },
  installCatalogSkill(catalogSkillId: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) {
    return command<InstallResult[]>(
      "install_catalog_skill",
      { catalogSkillId, targetAgentIds, conflictPolicy },
      () => [],
    );
  },
  // ── MCP API ──
  scanMcpServers() {
    return command<GroupedMcpServer[]>("scan_mcp_servers", {}, () => []);
  },
  readAgentSkillReadme(skillPath: string) {
    return command<string | null>("read_agent_skill_readme", { skillPath }, () => null);
  },
  addMcpServer(agentIds: string[], config: McpServerConfig, conflictPolicy: ConflictPolicy) {
    return command<McpOperationResult[]>("add_mcp_server", { agentIds, config, conflictPolicy }, () => []);
  },
  updateMcpServer(agentId: string, originalName: string, config: McpServerConfig) {
    return command<McpOperationResult>("update_mcp_server", { agentId, originalName, config }, () => ({
      agentId,
      serverName: config.name,
      action: "skipped",
      message: "Desktop only",
    }));
  },
  removeMcpServer(agentId: string, name: string) {
    return command<McpOperationResult>("remove_mcp_server", { agentId, name }, () => ({
      agentId,
      serverName: name,
      action: "skipped",
      message: "Desktop only",
    }));
  },
  toggleMcpServer(agentId: string, name: string, disabled: boolean) {
    return command<McpOperationResult>("toggle_mcp_server", { agentId, name, disabled }, () => ({
      agentId,
      serverName: name,
      action: "skipped",
      message: "Desktop only",
    }));
  },
  syncMcpServer(serverName: string, sourceAgentId: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) {
    return command<McpOperationResult[]>("sync_mcp_server", { serverName, sourceAgentId, targetAgentIds, conflictPolicy }, () => []);
  },
  removeMcpServerFromAgents(serverName: string, agentIds: string[]) {
    return command<McpOperationResult[]>("remove_mcp_server_from_agents", { serverName, agentIds }, () => []);
  },
};
