import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { ToastType } from "../components/ui/Toast";
import { hasTauriRuntime } from "../lib/env";
import type { AppUpdateInfo, AppUpdateInstallResult, SkillUpdateInfo, SkillUpdateResult } from "../types";

interface Props {
  showToast: (text: string, type?: ToastType) => void;
  refreshAll: () => Promise<void>;
}

export function useUpdates({ showToast, refreshAll }: Props) {
  const [skillUpdates, setSkillUpdates] = useState<SkillUpdateInfo[] | null>(null);
  const [skillUpdatesChecking, setSkillUpdatesChecking] = useState(false);
  const [skillUpdating, setSkillUpdating] = useState(false);
  const [appUpdate, setAppUpdate] = useState<AppUpdateInfo | null>(null);
  const [appUpdateChecking, setAppUpdateChecking] = useState(false);
  const [appUpdateInstalling, setAppUpdateInstalling] = useState(false);
  const [appUpdateInstalled, setAppUpdateInstalled] = useState<AppUpdateInstallResult | null>(null);
  const startupCheckedRef = useRef(false);

  async function checkSkillUpdates() {
    setSkillUpdatesChecking(true);
    try {
      const infos = await api.checkSkillUpdates();
      setSkillUpdates(infos);
      const available = infos.filter((info) => info.state === "updateAvailable").length;
      const failed = infos.filter((info) => info.state === "error").length;
      if (!infos.length) {
        showToast("中枢中没有记录来源的 Skill（通过 Git / URL 安装的 Skill 才能检查更新）。", "info");
      } else {
        const parts = [`已检查 ${infos.length} 个 Skill`, available ? `${available} 个可更新` : "均为最新"];
        if (failed) parts.push(`${failed} 个检查失败`);
        showToast(parts.join("，") + "。", failed ? "error" : available ? "info" : "success");
      }
    } catch (error) {
      showToast(`检查 Skill 更新失败：${String(error)}`, "error");
    } finally {
      setSkillUpdatesChecking(false);
    }
  }

  async function updateSkills(dirNames: string[]): Promise<SkillUpdateResult[]> {
    if (!dirNames.length) return [];
    setSkillUpdating(true);
    try {
      const results = await api.updateSkillsFromSource(dirNames);
      const updated = results.filter((result) => result.updated);
      const failed = results.filter((result) => result.error);
      setSkillUpdates((current) =>
        current?.map((info) =>
          updated.some((result) => result.dirName === info.dirName)
            ? { ...info, title: updated.find((result) => result.dirName === info.dirName)?.title ?? info.title, state: "upToDate", localModified: false, message: null }
            : info,
        ) ?? current,
      );
      await refreshAll();
      if (failed.length) {
        showToast(`已更新 ${updated.length} 个，${failed.length} 个失败：${failed[0].message}`, "error");
      } else {
        showToast(updated.length ? `已更新 ${updated.length} 个 Skill，原内容已备份。` : "所选 Skill 已是最新版本。", "success");
      }
      return results;
    } catch (error) {
      showToast(`更新 Skill 失败：${String(error)}`, "error");
      return [];
    } finally {
      setSkillUpdating(false);
    }
  }

  const checkAppUpdate = useCallback(async (silent = false) => {
    setAppUpdateChecking(true);
    try {
      const info = await api.checkAppUpdate();
      setAppUpdate(info);
      if (info.hasUpdate) {
        showToast(`发现新版本 v${info.latestVersion}，可在设置页更新。`, "info");
      } else if (!silent) {
        showToast(info.message ?? "当前已是最新版本。", "success");
      }
    } catch (error) {
      if (!silent) showToast(`检查程序更新失败：${String(error)}`, "error");
    } finally {
      setAppUpdateChecking(false);
    }
  }, [showToast]);

  async function installAppUpdate() {
    setAppUpdateInstalling(true);
    try {
      const result = await api.installAppUpdate();
      setAppUpdateInstalled(result);
      showToast(result.message, "success");
    } catch (error) {
      showToast(`安装更新失败：${String(error)}`, "error");
    } finally {
      setAppUpdateInstalling(false);
    }
  }

  async function restartApp() {
    try {
      await api.restartApp();
    } catch (error) {
      showToast(`重启失败，请手动重新打开程序：${String(error)}`, "error");
    }
  }

  // 启动后静默检查一次程序更新；失败不打扰用户
  useEffect(() => {
    if (startupCheckedRef.current || !hasTauriRuntime()) return;
    const timer = window.setTimeout(() => {
      startupCheckedRef.current = true;
      void checkAppUpdate(true);
    }, 3000);
    return () => window.clearTimeout(timer);
  }, [checkAppUpdate]);

  return {
    skillUpdates,
    skillUpdatesChecking,
    skillUpdating,
    checkSkillUpdates,
    updateSkills,
    appUpdate,
    appUpdateChecking,
    appUpdateInstalling,
    appUpdateInstalled,
    checkAppUpdate,
    installAppUpdate,
    restartApp,
  };
}
