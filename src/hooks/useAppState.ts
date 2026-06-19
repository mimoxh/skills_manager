import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type {
  AgentProfile,
  CatalogFilters,
  CatalogRefreshStatus,
  CatalogSafetyMode,
  CatalogSkill,
  CatalogSort,
  CatalogSource,
  ConflictPolicy,
  GroupedMcpServer,
  GroupedSkill,
  ImportSkillFile,
  InstallResult,
  McpOperationResult,
  McpServerConfig,
} from "../types";

const emptyCustom: AgentProfile = {
  id: "",
  name: "",
  type: "custom",
  skillsPath: "",
  adapterConfig: {},
};

const emptyCatalogFilters: CatalogFilters = {
  sourceIds: [],
  agentTypes: [],
  installStatuses: [],
  hasDownloadData: null,
  timeWindowDays: null,
  contentCapabilities: [],
  safetyMode: "all",
};

export function useAppState() {
  const [skills, setSkills] = useState<GroupedSkill[]>([]);
  const [agents, setAgents] = useState<AgentProfile[]>([]);
  const [customAgent, setCustomAgent] = useState<AgentProfile>(emptyCustom);
  const [message, setMessage] = useState("正在加载...");
  const [busy, setBusy] = useState(false);
  const [catalogBusy, setCatalogBusy] = useState(false);
  const [catalogStartupRefreshing, setCatalogStartupRefreshing] = useState(false);
  const [query, setQuery] = useState("");
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [pendingImport, setPendingImport] = useState<{ fileName: string; files: ImportSkillFile[] } | null>(null);
  const [noFullCoverageTitles, setNoFullCoverageTitles] = useState<Set<string>>(new Set());
  const [mcpServers, setMcpServers] = useState<GroupedMcpServer[]>([]);
  const [noFullCoverageMcpTitles, setNoFullCoverageMcpTitles] = useState<Set<string>>(new Set());
  const [catalogSources, setCatalogSources] = useState<CatalogSource[]>([]);
  const [catalogSkills, setCatalogSkills] = useState<CatalogSkill[]>([]);
  const [catalogTotal, setCatalogTotal] = useState(0);
  const [catalogPage, setCatalogPage] = useState(1);
  const [catalogPageSize] = useState(100);
  const [catalogHasMore, setCatalogHasMore] = useState(false);
  const [catalogRefreshStatuses, setCatalogRefreshStatuses] = useState<Record<CatalogSafetyMode, CatalogRefreshStatus | null>>({
    all: null,
    nonSuspicious: null,
  });
  const [catalogQuery, setCatalogQuery] = useState("");
  const [catalogSort, setCatalogSort] = useState<CatalogSort>("updatedDesc");
  const [catalogFilters, setCatalogFilters] = useState<CatalogFilters>(emptyCatalogFilters);

  const filteredSkills = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return skills;
    return skills.filter((s) =>
      [s.title, s.bestCopy.version ?? "", s.bestCopy.agentName, s.bestCopy.skillPath, ...s.copies.map((c) => c.agentName)]
        .some((v) => v.toLowerCase().includes(q)),
    );
  }, [query, skills]);

  const filteredAgents = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return agents;
    return agents.filter((a) => [a.name, a.id, a.type, a.skillsPath].some((v) => v.toLowerCase().includes(q)));
  }, [agents, query]);

  async function refreshAll() {
    setBusy(true);
    try {
      const data = await api.getInitialData();
      setSkills(data.skills);
      setAgents(data.agents);
      setNoFullCoverageTitles(new Set(data.noFullCoverageTitles));
      setNoFullCoverageMcpTitles(new Set(data.noFullCoverageMcpTitles));
      setMessage(`已识别 ${data.skills.length} 个去重 skills，${data.agents.length} 个 agent 配置。`);
      // 同时刷新 MCP servers
      try {
        const mcpData = await api.scanMcpServers();
        setMcpServers(mcpData);
      } catch {
        // MCP 扫描失败不影响主流程
      }
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
      setIsInitialLoading(false);
    }
  }

  async function refreshMcpServers() {
    setBusy(true);
    try {
      const mcpData = await api.scanMcpServers();
      setMcpServers(mcpData);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function loadSkillReadme(skillPath: string): Promise<string | null> {
    try {
      return await api.readAgentSkillReadme(skillPath);
    } catch (error) {
      setMessage(String(error));
      return null;
    }
  }

  async function searchCatalog(
    nextQuery = catalogQuery,
    nextSort = catalogSort,
    nextFilters = catalogFilters,
    nextPage = catalogPage,
  ) {
    setCatalogBusy(true);
    try {
      const [sources, result] = await Promise.all([
        api.listCatalogSources(),
        api.searchCatalogSkills(nextQuery, nextSort, nextFilters, nextPage, catalogPageSize),
      ]);
      setCatalogSources(sources);
      setCatalogSkills(result.items);
      setCatalogTotal(result.total);
      setCatalogPage(result.page);
      setCatalogHasMore(result.hasMore);
      setMessage(`仓库目录显示第 ${result.page} 页 ${result.items.length} 个 skills，共 ${result.total} 个。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setCatalogBusy(false);
    }
  }

  async function refreshCatalogSource(sourceId: string) {
    setCatalogBusy(true);
    try {
      const result = await api.refreshCatalogSource(sourceId);
      setMessage(result.message);
      await searchCatalog();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setCatalogBusy(false);
    }
  }

  async function refreshCatalogStatus(safetyMode: CatalogSafetyMode = catalogFilters.safetyMode) {
    try {
      const status = await api.getCatalogRefreshStatus("clawhub", safetyMode);
      setCatalogRefreshStatuses((previous) => ({ ...previous, [safetyMode]: status }));
      return status;
    } catch (error) {
      setMessage(String(error));
      return null;
    }
  }

  async function startCatalogRefresh(safetyMode: CatalogSafetyMode = catalogFilters.safetyMode) {
    setCatalogBusy(true);
    try {
      const status = await api.startCatalogRefresh("clawhub", safetyMode);
      setCatalogRefreshStatuses((previous) => ({ ...previous, [safetyMode]: status }));
      setMessage(`ClawHub 后台刷新已启动，当前已索引 ${status.fetchedCount} 个 skills。`);
      return status;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setCatalogBusy(false);
    }
  }

  async function cancelCatalogRefresh(safetyMode: CatalogSafetyMode = catalogFilters.safetyMode) {
    try {
      const status = await api.cancelCatalogRefresh("clawhub", safetyMode);
      setCatalogRefreshStatuses((previous) => ({ ...previous, [safetyMode]: status }));
      setMessage("已请求取消 ClawHub 后台刷新。");
      return status;
    } catch (error) {
      setMessage(String(error));
      throw error;
    }
  }

  async function changeCatalogPage(nextPage: number) {
    const safePage = Math.max(1, nextPage);
    await searchCatalog(catalogQuery, catalogSort, catalogFilters, safePage);
  }

  async function refreshCatalogOnStartup() {
    setCatalogStartupRefreshing(true);
    try {
      await searchCatalog();
      const sources = await api.listCatalogSources();
      let refreshed = 0;
      for (const source of sources.filter((source) => source.enabled && source.id !== "clawhub")) {
        try {
          await api.refreshCatalogSource(source.id);
          refreshed += 1;
        } catch (error) {
          console.warn(`Catalog source refresh failed: ${source.id}`, error);
        }
      }
      const [nextSources, nextSkills] = await Promise.all([
        api.listCatalogSources(),
        api.searchCatalogSkills(catalogQuery, catalogSort, catalogFilters, catalogPage, catalogPageSize),
      ]);
      setCatalogSources(nextSources);
      setCatalogSkills(nextSkills.items);
      setCatalogTotal(nextSkills.total);
      setCatalogPage(nextSkills.page);
      setCatalogHasMore(nextSkills.hasMore);
      if (refreshed > 0) {
        setMessage(`已后台更新 ${refreshed} 个仓库源，仓库目录显示第 ${nextSkills.page} 页 ${nextSkills.items.length} 个 skills，共 ${nextSkills.total} 个。`);
      }
    } catch (error) {
      setMessage(`仓库目录后台更新失败: ${String(error)}`);
    } finally {
      setCatalogStartupRefreshing(false);
    }
  }

  async function saveCatalogSource(source: CatalogSource) {
    setCatalogBusy(true);
    try {
      await api.saveCatalogSource(source);
      await searchCatalog();
      setMessage(`已保存仓库源 ${source.name}。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setCatalogBusy(false);
    }
  }

  async function installCatalogSkill(
    catalogSkillId: string,
    targetAgentIds: string[],
    conflictPolicy: ConflictPolicy,
  ): Promise<InstallResult[]> {
    if (!targetAgentIds.length) {
      setMessage("请至少选择一个目标 Agent。");
      return [];
    }
    setCatalogBusy(true);
    try {
      const results = await api.installCatalogSkill(catalogSkillId, targetAgentIds, conflictPolicy);
      await refreshAll();
      await searchCatalog();
      setMessage(`已完成 ${results.length} 个安装任务。`);
      return results;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setCatalogBusy(false);
    }
  }

  async function addMcpServer(
    agentIds: string[],
    config: McpServerConfig,
    conflictPolicy: ConflictPolicy,
  ): Promise<McpOperationResult[]> {
    if (!agentIds.length) {
      setMessage("请至少选择一个目标 Agent。");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.addMcpServer(agentIds, config, conflictPolicy);
      await refreshMcpServers();
      setMessage(`已添加 ${config.name} 到 ${results.length} 个 Agent。`);
      return results;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function updateMcpServer(
    agentId: string,
    originalName: string,
    config: McpServerConfig,
  ): Promise<McpOperationResult> {
    setBusy(true);
    try {
      const result = await api.updateMcpServer(agentId, originalName, config);
      await refreshMcpServers();
      setMessage(`已更新 ${config.name}。`);
      return result;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function removeMcpServer(
    agentId: string,
    name: string,
  ): Promise<McpOperationResult> {
    setBusy(true);
    try {
      const result = await api.removeMcpServer(agentId, name);
      await refreshMcpServers();
      setMessage(`已从 Agent 删除 ${name}。`);
      return result;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function toggleMcpServer(
    agentId: string,
    name: string,
    disabled: boolean,
  ): Promise<McpOperationResult> {
    setBusy(true);
    try {
      const result = await api.toggleMcpServer(agentId, name, disabled);
      await refreshMcpServers();
      setMessage(`已${disabled ? "禁用" : "启用"} ${name}。`);
      return result;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function syncMcpServerToAgents(
    serverName: string,
    sourceAgentId: string,
    targetAgentIds: string[],
    conflictPolicy: ConflictPolicy,
  ): Promise<McpOperationResult[]> {
    if (!targetAgentIds.length) {
      setMessage("请至少选择一个目标 Agent。");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.syncMcpServer(serverName, sourceAgentId, targetAgentIds, conflictPolicy);
      await refreshMcpServers();
      setMessage(`已同步 ${serverName} 到 ${results.length} 个 Agent。`);
      return results;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function removeMcpServerFromAgents(
    serverName: string,
    agentIds: string[],
  ): Promise<McpOperationResult[]> {
    if (!agentIds.length) return [];
    setBusy(true);
    try {
      const results = await api.removeMcpServerFromAgents(serverName, agentIds);
      await refreshMcpServers();
      setMessage(`已从 ${agentIds.length} 个 Agent 删除 ${serverName}。`);
      return results;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void (async () => {
      await refreshAll();
      await Promise.all([refreshCatalogStatus("all"), refreshCatalogStatus("nonSuspicious")]);
      void refreshCatalogOnStartup();
    })();
  }, []);

  async function saveCustomAgent(override?: AgentProfile) {
    const source = override ?? customAgent;
    const agent = {
      ...source,
      id: source.id || crypto.randomUUID(),
      name: source.name.trim(),
      skillsPath: source.skillsPath.trim(),
    };
    if (!agent.name || !agent.skillsPath) {
      setMessage("自定义 Agent 需要填写名称和 Skills 安装目录。");
      return;
    }
    setBusy(true);
    try {
      await api.addAgent(agent);
      if (!override) setCustomAgent(emptyCustom);
      await refreshAll();
      setMessage(`已保存 ${agent.name}。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function deleteAgent(agentId: string) {
    setBusy(true);
    try {
      await api.removeAgent(agentId);
      await refreshAll();
      setMessage("已删除 Agent。");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function uninstallSkill(skillId: string, agentId: string) {
    setBusy(true);
    try {
      await api.uninstallSkill(skillId, agentId);
      await refreshAll();
      setMessage(`已从 Agent 卸载 ${skillId}。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function uninstallSkillFromAgents(skillId: string, agentIds: string[]) {
    if (!agentIds.length) return;
    setBusy(true);
    try {
      await api.uninstallSkillFromAgents(skillId, agentIds);
      await refreshAll();
      setMessage(`已从 ${agentIds.length} 个 Agent 卸载 ${skillId}。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function repairClaudeCoworkManifest(agentId: string) {
    setBusy(true);
    try {
      const result = await api.repairClaudeCoworkManifest(agentId);
      await refreshAll();
      setMessage(result.message);
      return result;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function syncSkillToAgents(
    title: string,
    targetAgentIds: string[],
    conflictPolicy: ConflictPolicy,
    sourceAgentId?: string | null,
  ): Promise<InstallResult[]> {
    if (!targetAgentIds.length) {
      setMessage("请至少选择一个目标 Agent。");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.syncGroupedSkill(title, sourceAgentId, targetAgentIds, conflictPolicy);
      await refreshAll();
      setMessage(`已完成 ${results.length} 个同步任务。`);
      return results;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function fileToUpload(file: File, relativePath?: string): Promise<ImportSkillFile> {
    return {
      relativePath: relativePath || file.webkitRelativePath || file.name,
      bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
    };
  }

  async function collectEntryFiles(entry: FileSystemEntry, prefix = ""): Promise<ImportSkillFile[]> {
    if (entry.isFile) {
      const file = await new Promise<File>((resolve, reject) => {
        (entry as FileSystemFileEntry).file(resolve, reject);
      });
      return [await fileToUpload(file, `${prefix}${file.name}`)];
    }
    if (!entry.isDirectory) return [];
    const directory = entry as FileSystemDirectoryEntry;
    const reader = directory.createReader();
    const children = await new Promise<FileSystemEntry[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    const nested = await Promise.all(children.map((child) => collectEntryFiles(child, `${prefix}${directory.name}/`)));
    return nested.flat();
  }

  async function importFiles(fileName: string, files: ImportSkillFile[]) {
    if (!files.length) {
      setMessage("没有可导入的文件。");
      return;
    }
    setPendingImport({ fileName, files });
  }

  async function executeImport(targetAgentIds: string[], conflictPolicy: ConflictPolicy) {
    if (!pendingImport) return;
    setBusy(true);
    try {
      const result = await api.importSkillUpload(pendingImport.fileName, pendingImport.files, targetAgentIds, conflictPolicy);
      await refreshAll();
      setMessage(result.message);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
      setPendingImport(null);
    }
  }

  function cancelImport() {
    setPendingImport(null);
  }

  async function handleSkillDrop(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    const entries = [...event.dataTransfer.items]
      .map((item) => item.webkitGetAsEntry?.())
      .filter((e): e is FileSystemEntry => Boolean(e));
    if (entries.length) {
      const files = (await Promise.all(entries.map((e) => collectEntryFiles(e)))).flat();
      await importFiles(entries[0].name, files);
      return;
    }
    const files = [...event.dataTransfer.files];
    await importFiles(files[0]?.name ?? "upload", await Promise.all(files.map((f) => fileToUpload(f))));
  }

  async function toggleNoFullCoverage(title: string) {
    setBusy(true);
    try {
      await api.toggleNoFullCoverage(title);
      await refreshAll();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function setSkillTags(title: string, tags: string[]): Promise<string[]> {
    setBusy(true);
    try {
      const savedTags = await api.setSkillTags(title, tags);
      await refreshAll();
      setMessage("已更新标签。");
      return savedTags;
    } catch (error) {
      setMessage(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function toggleMcpNoFullCoverage(title: string) {
    setBusy(true);
    try {
      await api.toggleMcpNoFullCoverage(title);
      await refreshAll();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  return {
    skills, filteredSkills,
    catalogSources, catalogSkills, catalogTotal, catalogPage, catalogPageSize, catalogHasMore, catalogQuery, catalogSort, catalogFilters,
    catalogRefreshStatuses, refreshCatalogStatus, startCatalogRefresh, cancelCatalogRefresh,
    agents, filteredAgents,
    customAgent, setCustomAgent, saveCustomAgent, saveAgent: saveCustomAgent,
    message, setMessage,
    busy, catalogBusy, catalogStartupRefreshing, query, setQuery, setCatalogQuery, setCatalogSort, setCatalogFilters,
    isInitialLoading,
    pendingImport, executeImport, cancelImport,
    refreshAll, loadSkillReadme, syncSkillToAgents, deleteAgent, uninstallSkill, uninstallSkillFromAgents,
    repairClaudeCoworkManifest,
    searchCatalog, changeCatalogPage, refreshCatalogSource, saveCatalogSource, installCatalogSkill,
    handleSkillDrop, importFiles, fileToUpload,
    noFullCoverageTitles, toggleNoFullCoverage,
    setSkillTags,
    mcpServers, refreshMcpServers, addMcpServer, updateMcpServer, removeMcpServer, toggleMcpServer,
    syncMcpServerToAgents, removeMcpServerFromAgents,
    noFullCoverageMcpTitles, toggleMcpNoFullCoverage,
  };
}
