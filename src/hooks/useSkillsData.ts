import { useRef, useState } from "react";
import { api } from "../api";
import type { ToastType } from "../components/ui/Toast";
import { fileToUpload } from "../lib/utils";
import type {
  AgentProfile,
  ConflictPolicy,
  GroupedMcpServer,
  GroupedSkill,
  ImportSkillFile,
  InstallResult,
} from "../types";

const emptyCustom: AgentProfile = {
  id: "",
  name: "",
  type: "custom",
  skillsPath: "",
  adapterConfig: {},
  userTags: [],
};

interface Props {
  showToast: (text: string, type?: ToastType) => void;
  setBusy: (busy: boolean) => void;
  setMcpServers: (servers: GroupedMcpServer[]) => void;
  setNoFullCoverageMcpTitles: (titles: Set<string>) => void;
}

export function useSkillsData({ showToast, setBusy, setMcpServers, setNoFullCoverageMcpTitles }: Props) {
  const [skills, setSkills] = useState<GroupedSkill[]>([]);
  const [agents, setAgents] = useState<AgentProfile[]>([]);
  const [customAgent, setCustomAgent] = useState<AgentProfile>(emptyCustom);
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [pendingImport, setPendingImport] = useState<{ fileName: string; files: ImportSkillFile[] } | null>(null);
  const [noFullCoverageTitles, setNoFullCoverageTitles] = useState<Set<string>>(new Set());
  const [defaultCatalogSourceId, setDefaultCatalogSourceId] = useState("clawhub");
  const readmeCacheRef = useRef(new Map<string, string>());

  async function refreshAll() {
    setBusy(true);
    try {
      const data = await api.getInitialData();
      setSkills(data.skills);
      setAgents(data.agents);
      setNoFullCoverageTitles(new Set(data.noFullCoverageTitles));
      setNoFullCoverageMcpTitles(new Set(data.noFullCoverageMcpTitles));
      setDefaultCatalogSourceId(data.defaultCatalogSourceId || "clawhub");
      showToast(`已识别 ${data.skills.length} 个去重 skills，${data.agents.length} 个 agent 配置。`, "info");
      // 同时刷新 MCP servers
      try {
        const [mcpData, mcpWarnings] = await api.scanMcpServers();
        setMcpServers(mcpData);
        if (mcpWarnings.length) {
          showToast(mcpWarnings.join("；"), "error");
        }
      } catch {
        // MCP 扫描失败不影响主流程
      }
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setBusy(false);
      setIsInitialLoading(false);
    }
  }

  async function loadSkillReadme(skillPath: string): Promise<string | null> {
    // 内存缓存：同一 skill 路径的 readme 避免重复 invoke（L-F8）
    const cached = readmeCacheRef.current.get(skillPath);
    if (cached !== undefined) return cached;
    try {
      const readme = await api.readAgentSkillReadme(skillPath);
      if (readme !== null) readmeCacheRef.current.set(skillPath, readme);
      return readme;
    } catch (error) {
      showToast(String(error), "error");
      return null;
    }
  }

  async function saveCustomAgent(override?: AgentProfile) {
    const source = override ?? customAgent;
    const agent = {
      ...source,
      id: source.id || crypto.randomUUID(),
      name: source.name.trim(),
      skillsPath: source.skillsPath.trim(),
      userTags: source.userTags ?? [],
    };
    if (!agent.name || !agent.skillsPath) {
      showToast("自定义 Agent 需要填写名称和 Skills 安装目录。", "error");
      return;
    }
    setBusy(true);
    try {
      await api.addAgent(agent);
      if (!override) setCustomAgent(emptyCustom);
      await refreshAll();
      showToast(`已保存 ${agent.name}。`, "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setBusy(false);
    }
  }

  async function deleteAgent(agentId: string) {
    setBusy(true);
    try {
      await api.removeAgent(agentId);
      await refreshAll();
      showToast("已删除 Agent。", "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setBusy(false);
    }
  }

  async function uninstallSkill(skillId: string, agentId: string) {
    setBusy(true);
    try {
      await api.uninstallSkill(skillId, agentId);
      await refreshAll();
      showToast(`已从 Agent 卸载 ${skillId}。`, "success");
    } catch (error) {
      showToast(String(error), "error");
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
      showToast(`已从 ${agentIds.length} 个 Agent 卸载 ${skillId}。`, "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setBusy(false);
    }
  }

  async function repairClaudeCoworkManifest(agentId: string) {
    setBusy(true);
    try {
      const result = await api.repairClaudeCoworkManifest(agentId);
      await refreshAll();
      showToast(result.message, "success");
      return result;
    } catch (error) {
      showToast(String(error), "error");
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
      showToast("请至少选择一个目标 Agent。", "error");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.syncGroupedSkill(title, sourceAgentId, targetAgentIds, conflictPolicy);
      await refreshAll();
      showToast(`已完成 ${results.length} 个同步任务。`, "success");
      return results;
    } catch (error) {
      showToast(String(error), "error");
      throw error;
    } finally {
      setBusy(false);
    }
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
    // readEntries 每批最多返回约 100 条，需循环读取直到返回空数组，避免大目录导入被静默截断
    const children: FileSystemEntry[] = [];
    while (true) {
      const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => {
        reader.readEntries(resolve, reject);
      });
      if (batch.length === 0) break;
      children.push(...batch);
    }
    const nested = await Promise.all(children.map((child) => collectEntryFiles(child, `${prefix}${directory.name}/`)));
    return nested.flat();
  }

  async function importFiles(fileName: string, files: ImportSkillFile[]) {
    if (!files.length) {
      showToast("没有可导入的文件。", "error");
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
      showToast(result.message, "success");
    } catch (error) {
      const msg = String(error).replace(/^Error invoking plugin /, "");
      showToast(msg, "error");
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
      const isNowMarked = await api.toggleNoFullCoverage(title);
      setNoFullCoverageTitles((previous) => {
        const next = new Set(previous);
        if (isNowMarked) next.add(title);
        else next.delete(title);
        return next;
      });
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setBusy(false);
    }
  }

  async function setSkillTags(title: string, tags: string[]): Promise<string[]> {
    setBusy(true);
    try {
      const savedTags = await api.setSkillTags(title, tags);
      setSkills((previous) =>
        previous.map((skill) => (skill.title === title ? { ...skill, userTags: savedTags } : skill)),
      );
      showToast("已更新标签。", "success");
      return savedTags;
    } catch (error) {
      showToast(String(error), "error");
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function setAgentTags(agentId: string, tags: string[]): Promise<string[]> {
    setBusy(true);
    try {
      const savedTags = await api.setAgentTags(agentId, tags);
      setAgents((previous) =>
        previous.map((agent) => (agent.id === agentId ? { ...agent, userTags: savedTags } : agent)),
      );
      showToast("已更新 Agent 标签。", "success");
      return savedTags;
    } catch (error) {
      showToast(String(error), "error");
      throw error;
    } finally {
      setBusy(false);
    }
  }

  return {
    skills,
    agents,
    customAgent,
    setCustomAgent,
    saveCustomAgent,
    isInitialLoading,
    pendingImport,
    executeImport,
    cancelImport,
    refreshAll,
    loadSkillReadme,
    syncSkillToAgents,
    deleteAgent,
    uninstallSkill,
    uninstallSkillFromAgents,
    repairClaudeCoworkManifest,
    handleSkillDrop,
    importFiles,
    fileToUpload,
    noFullCoverageTitles,
    toggleNoFullCoverage,
    setSkillTags,
    setAgentTags,
    defaultCatalogSourceId,
  };
}