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
      showToast(`已添加 ${config.name} 到 ${results.length} 个 Agent。`, "success");
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
      showToast(`已同步 ${serverName} 到 ${results.length} 个 Agent。`, "success");
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
      showToast(`已从 ${agentIds.length} 个 Agent 删除 ${serverName}。`, "success");
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