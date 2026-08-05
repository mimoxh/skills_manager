import { useEffect, useState } from "react";
import { useCatalog } from "./useCatalog";
import { useMcpServers } from "./useMcpServers";
import { useSkillsData } from "./useSkillsData";
import { useToast } from "./useToast";

export function useAppState() {
  const { toast, showToast, dismissToast } = useToast();
  // busy 提升到组合层，跨 skills/mcp 域共享单一加载状态
  const [busy, setBusy] = useState(false);

  const mcp = useMcpServers({ showToast, setBusy });
  const skillsData = useSkillsData({
    showToast,
    setBusy,
    setMcpServers: mcp.setMcpServers,
    setNoFullCoverageMcpTitles: mcp.setNoFullCoverageMcpTitles,
  });
  const catalog = useCatalog({
    showToast,
    refreshAll: skillsData.refreshAll,
    defaultSourceId: skillsData.defaultCatalogSourceId,
  });

  // 启动初始化：全量刷新 skills/agents + MCP，随后拉取两种安全模式的刷新状态
  useEffect(() => {
    void (async () => {
      await skillsData.refreshAll();
      await Promise.all([
        catalog.refreshCatalogStatus("all"),
        catalog.refreshCatalogStatus("nonSuspicious"),
      ]);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    skills: skillsData.skills,
    catalogSources: catalog.catalogSources,
    catalogSkills: catalog.catalogSkills,
    catalogTotal: catalog.catalogTotal,
    catalogPage: catalog.catalogPage,
    catalogPageSize: catalog.catalogPageSize,
    catalogHasMore: catalog.catalogHasMore,
    catalogQuery: catalog.catalogQuery,
    catalogSort: catalog.catalogSort,
    catalogFilters: catalog.catalogFilters,
    catalogRefreshStatuses: catalog.catalogRefreshStatuses,
    refreshCatalogStatus: catalog.refreshCatalogStatus,
    startCatalogRefresh: catalog.startCatalogRefresh,
    cancelCatalogRefresh: catalog.cancelCatalogRefresh,
    agents: skillsData.agents,
    customAgent: skillsData.customAgent,
    setCustomAgent: skillsData.setCustomAgent,
    saveCustomAgent: skillsData.saveCustomAgent,
    saveAgent: skillsData.saveCustomAgent,
    toast,
    showToast,
    dismissToast,
    busy,
    catalogBusy: catalog.catalogBusy,
    catalogStartupRefreshing: catalog.catalogStartupRefreshing,
    setCatalogQuery: catalog.setCatalogQuery,
    setCatalogSort: catalog.setCatalogSort,
    setCatalogFilters: catalog.setCatalogFilters,
    isInitialLoading: skillsData.isInitialLoading,
    pendingImport: skillsData.pendingImport,
    executeImport: skillsData.executeImport,
    cancelImport: skillsData.cancelImport,
    refreshAll: skillsData.refreshAll,
    loadSkillReadme: skillsData.loadSkillReadme,
    syncSkillToAgents: skillsData.syncSkillToAgents,
    deleteAgent: skillsData.deleteAgent,
    uninstallSkill: skillsData.uninstallSkill,
    uninstallSkillFromAgents: skillsData.uninstallSkillFromAgents,
    repairClaudeCoworkManifest: skillsData.repairClaudeCoworkManifest,
    searchCatalog: catalog.searchCatalog,
    changeCatalogPage: catalog.changeCatalogPage,
    refreshCatalogSource: catalog.refreshCatalogSource,
    ensureCatalogLoaded: catalog.ensureCatalogLoaded,
    saveCatalogSource: catalog.saveCatalogSource,
    installCatalogSkill: catalog.installCatalogSkill,
    handleSkillDrop: skillsData.handleSkillDrop,
    importFiles: skillsData.importFiles,
    fileToUpload: skillsData.fileToUpload,
    noFullCoverageTitles: skillsData.noFullCoverageTitles,
    toggleNoFullCoverage: skillsData.toggleNoFullCoverage,
    setSkillTags: skillsData.setSkillTags,
    setAgentTags: skillsData.setAgentTags,
    defaultCatalogSourceId: skillsData.defaultCatalogSourceId,
    mcpServers: mcp.mcpServers,
    refreshMcpServers: mcp.refreshMcpServers,
    addMcpServer: mcp.addMcpServer,
    updateMcpServer: mcp.updateMcpServer,
    removeMcpServer: mcp.removeMcpServer,
    toggleMcpServer: mcp.toggleMcpServer,
    syncMcpServerToAgents: mcp.syncMcpServerToAgents,
    removeMcpServerFromAgents: mcp.removeMcpServerFromAgents,
    noFullCoverageMcpTitles: mcp.noFullCoverageMcpTitles,
    toggleMcpNoFullCoverage: mcp.toggleMcpNoFullCoverage,
  };
}