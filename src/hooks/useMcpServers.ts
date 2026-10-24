import { useState } from "react";
import { api } from "../api";
import type { ToastType } from "../components/ui/Toast";
import type {
  ConflictPolicy,
  GroupedMcpServer,
  McpOperationResult,
  McpServerConfig,
} from "../types";

interface Props {
  showToast: (text: string, type?: ToastType) => void;
  setBusy: (busy: boolean) => void;
}

function summarizeMcpResults(
  results: McpOperationResult[],
  successText: (succeededCount: number) => string,
  actionLabel: string,
): { text: string; type: ToastType } {
  const errors = results.filter((r) => r.action === "error");
  const skipped = results.filter((r) => r.action === "skipped");
  const succeededCount = new Set(
    results.filter((r) => r.action !== "error" && r.action !== "skipped").map((r) => r.agentId),
  ).size;
  if (errors.length > 0 || skipped.length > 0) {
    const firstProblem = errors[0] ?? skipped[0];
    const parts: string[] = [];
    if (succeededCount > 0) parts.push(`${succeededCount} 个 Agent 成功`);
    if (errors.length > 0) parts.push(`${errors.length} 个失败`);
    if (skipped.length > 0) parts.push(`${skipped.length} 个跳过`);
    const detail = firstProblem?.message ? `：${firstProblem.message}` : "";
    return {
      text: `${actionLabel}结果：${parts.join("，")}${detail}`,
      type: errors.length > 0 ? "error" : "info",
    };
  }
  return {
    text: successText(succeededCount),
    type: "success",
  };
}

export function useMcpServers({ showToast, setBusy }: Props) {
  const [mcpServers, setMcpServers] = useState<GroupedMcpServer[]>([]);
  const [noFullCoverageMcpTitles, setNoFullCoverageMcpTitles] = useState<Set<string>>(new Set());

  async function refreshMcpServers() {
    setBusy(true);
    try {
      const [mcpData, mcpWarnings] = await api.scanMcpServers();
      setMcpServers(mcpData);
      if (mcpWarnings.length) {
        showToast(mcpWarnings.join("；"), "error");
      }
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setBusy(false);
    }
  }

  async function addMcpServer(
    agentIds: string[],
    config: McpServerConfig,
    conflictPolicy: ConflictPolicy,
  ): Promise<McpOperationResult[]> {
    if (!agentIds.length) {
      showToast("请至少选择一个目标 Agent。", "error");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.addMcpServer(agentIds, config, conflictPolicy);
      await refreshMcpServers();
      const toastInfo = summarizeMcpResults(results, (count) => `已添加 ${config.name} 到 ${count} 个 Agent。`, "添加");
      showToast(toastInfo.text, toastInfo.type);
      return results;
    } catch (error) {
      showToast(String(error), "error");
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
      showToast(`已更新 ${config.name}。`, "success");
      return result;
    } catch (error) {
      showToast(String(error), "error");
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
      showToast(`已从 Agent 删除 ${name}。`, "success");
      return result;
    } catch (error) {
      showToast(String(error), "error");
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
      showToast(`已${disabled ? "禁用" : "启用"} ${name}。`, "success");
      return result;
    } catch (error) {
      showToast(String(error), "error");
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
      showToast("请至少选择一个目标 Agent。", "error");
      return [];
    }
    setBusy(true);
    try {
      const results = await api.syncMcpServer(serverName, sourceAgentId, targetAgentIds, conflictPolicy);
      await refreshMcpServers();
      const toastInfo = summarizeMcpResults(results, (count) => `已同步 ${serverName} 到 ${count} 个 Agent。`, "同步");
      showToast(toastInfo.text, toastInfo.type);
      return results;
    } catch (error) {
      showToast(String(error), "error");
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
      const toastInfo = summarizeMcpResults(results, (count) => `已从 ${count} 个 Agent 删除 ${serverName}。`, "删除");
      showToast(toastInfo.text, toastInfo.type);
      return results;
    } catch (error) {
      showToast(String(error), "error");
      throw error;
    } finally {
      setBusy(false);
    }
  }

  async function toggleMcpNoFullCoverage(title: string) {
    setBusy(true);
    try {
      const isNowMarked = await api.toggleMcpNoFullCoverage(title);
      setNoFullCoverageMcpTitles((previous) => {
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

  return {
    mcpServers,
    setMcpServers,
    noFullCoverageMcpTitles,
    setNoFullCoverageMcpTitles,
    refreshMcpServers,
    addMcpServer,
    updateMcpServer,
    removeMcpServer,
    toggleMcpServer,
    syncMcpServerToAgents,
    removeMcpServerFromAgents,
    toggleMcpNoFullCoverage,
  };
}