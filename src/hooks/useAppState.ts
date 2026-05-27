import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type {
  AgentProfile,
  ConflictPolicy,
  DiscoveryPathEntry,
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
  const [repository, setRepository] = useState("");
  const [skills, setSkills] = useState<GroupedSkill[]>([]);
  const [agents, setAgents] = useState<AgentProfile[]>([]);
  const [customAgent, setCustomAgent] = useState<AgentProfile>(emptyCustom);
  const [message, setMessage] = useState("正在加载...");
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState("");
  const [discoveryPaths, setDiscoveryPaths] = useState<DiscoveryPathEntry[]>([]);

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
      const [repo, scanned, savedAgents, detected, paths] = await Promise.all([
        api.getRepository(),
        api.scanAgentSkills().catch(() => []),
        api.listAgents(),
        api.detectAgents(),
        api.listDiscoveryPaths().catch(() => []),
      ]);
      setRepository(repo ?? "");
      setSkills(scanned);
      setDiscoveryPaths(paths);
      const merged = new Map<string, AgentProfile>();
      [...savedAgents, ...detected].forEach((a) => merged.set(a.id, a));
      const nextAgents = [...merged.values()];
      setAgents(nextAgents);
      setMessage(`已识别 ${scanned.length} 个去重 skills，${nextAgents.length} 个 agent 配置。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => { void refreshAll(); }, []);

  async function saveRepository() {
    setBusy(true);
    try {
      await api.setRepository(repository.trim());
      setMessage("主仓库已保存，正在重新扫描...");
      await refreshAll();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

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
    setBusy(true);
    try {
      const result = await api.importSkillUpload(fileName, files);
      await refreshAll();
      setMessage(result.message);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
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
    repository, setRepository,
    skills, filteredSkills,
    agents, filteredAgents,
    customAgent, setCustomAgent, saveCustomAgent,
    message, setMessage,
    busy, query, setQuery,
    discoveryPaths,
    refreshAll, saveRepository, syncSkillToAgents,
    handleSkillDrop, importFiles, fileToUpload,
  };
}
