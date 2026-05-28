import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type {
  AgentProfile,
  ConflictPolicy,
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
};

export function useAppState() {
  const [skills, setSkills] = useState<GroupedSkill[]>([]);
  const [agents, setAgents] = useState<AgentProfile[]>([]);
  const [customAgent, setCustomAgent] = useState<AgentProfile>(emptyCustom);
  const [message, setMessage] = useState("正在加载...");
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState("");
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [pendingImport, setPendingImport] = useState<{ fileName: string; files: ImportSkillFile[] } | null>(null);
  const [pendingUrlImport, setPendingUrlImport] = useState<string | null>(null);

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
      setMessage(`已识别 ${data.skills.length} 个去重 skills，${data.agents.length} 个 agent 配置。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
      setIsInitialLoading(false);
    }
  }

  useEffect(() => {
    void refreshAll();
  }, []);

  async function saveCustomAgent() {
    const agent = {
      ...customAgent,
      id: customAgent.id || crypto.randomUUID(),
      name: customAgent.name.trim(),
      skillsPath: customAgent.skillsPath.trim(),
    };
    if (!agent.name || !agent.skillsPath) {
      setMessage("自定义 Agent 需要填写名称和 Skills 安装目录。");
      return;
    }
    setBusy(true);
    try {
      await api.addAgent(agent);
      setCustomAgent(emptyCustom);
      await refreshAll();
      setMessage(`已添加 ${agent.name}。`);
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

  async function syncSkillToAgents(
    title: string,
    targetAgentIds: string[],
    conflictPolicy: ConflictPolicy,
  ): Promise<InstallResult[]> {
    if (!targetAgentIds.length) {
      setMessage("请至少选择一个目标 Agent。");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.syncGroupedSkill(title, null, targetAgentIds, conflictPolicy);
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

  function startUrlImport(url: string) {
    setPendingUrlImport(url);
  }

  async function executeUrlImport(url: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) {
    setBusy(true);
    try {
      const result = await api.importFromUrl(url, targetAgentIds, conflictPolicy);
      await refreshAll();
      setMessage(result.message);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
      setPendingUrlImport(null);
    }
  }

  function cancelUrlImport() {
    setPendingUrlImport(null);
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

  return {
    skills, filteredSkills,
    agents, filteredAgents,
    customAgent, setCustomAgent, saveCustomAgent,
    message, setMessage,
    busy, query, setQuery,
    isInitialLoading,
    pendingImport, executeImport, cancelImport,
    pendingUrlImport, startUrlImport, executeUrlImport, cancelUrlImport,
    refreshAll, syncSkillToAgents, deleteAgent, uninstallSkill, uninstallSkillFromAgents,
    handleSkillDrop, importFiles, fileToUpload,
  };
}
