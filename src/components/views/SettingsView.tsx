import { useState } from "react";
import { FolderPlus, Trash2 } from "lucide-react";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "../ui/card";
import { Input } from "../ui/input";
import { Separator } from "../ui/separator";
import { api } from "../../api";
import type { DiscoveryPathEntry } from "../../types";

interface SettingsViewProps {
  repository: string;
  busy: boolean;
  onRepository: (path: string) => void;
  onSave: () => void;
  discoveryPaths: DiscoveryPathEntry[];
  onRefresh: () => Promise<void>;
}

export function SettingsView({ repository, busy, onRepository, onSave, discoveryPaths, onRefresh }: SettingsViewProps) {
  const [newPath, setNewPath] = useState("");
  const [newLabel, setNewLabel] = useState("");
  const [newSubdir, setNewSubdir] = useState("skills");

  async function handleAddPath() {
    if (!newPath.trim()) return;
    await api.addDiscoveryPath(newPath.trim(), newLabel.trim() || newPath.trim(), newSubdir.trim() || "skills");
    setNewPath("");
    setNewLabel("");
    setNewSubdir("skills");
    await onRefresh();
  }

  async function handleRemovePath(path: string) {
    await api.removeDiscoveryPath(path);
    await onRefresh();
  }

  return (
    <div className="flex flex-col gap-5">
      <Card>
        <CardHeader>
          <CardTitle>主仓库</CardTitle>
          <CardDescription>Skills Manager 会从这里扫描和导入 skills</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col gap-2.5 md:flex-row">
            <Input
              value={repository}
              onChange={(e) => onRepository(e.target.value)}
              placeholder="C:\Users\you\skills"
              className="flex-1"
            />
            <Button variant="primary" onClick={onSave} disabled={busy}>
              <FolderPlus size={14} />
              保存并扫描
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>发现路径</CardTitle>
          <CardDescription>自动扫描额外目录来发现 skills</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-col gap-2.5">
            {discoveryPaths.map((entry) => (
              <div
                key={entry.path}
                className="flex min-h-[76px] items-center gap-3 rounded-md border border-[var(--color-border)] bg-[var(--color-surface-raised)] p-3 transition-colors hover:bg-[var(--color-surface)]"
              >
                <div className="flex-1 min-w-0">
                  <p className="truncate text-sm font-semibold text-[var(--color-text)]">{entry.label}</p>
                  <p className="mt-1 truncate text-xs text-[var(--color-text-secondary)]">{entry.path}</p>
                  <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5">子目录: {entry.skillsSubdir}</p>
                </div>
                <button
                  className="flex h-10 w-10 items-center justify-center rounded-md text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-danger-light)] hover:text-[var(--color-danger)]"
                  onClick={() => handleRemovePath(entry.path)}
                  title="删除"
                  type="button"
                >
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
            {!discoveryPaths.length && (
              <p className="rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] py-8 text-center text-sm text-[var(--color-text-secondary)]">没有发现路径</p>
            )}
          </div>

          <Separator />

          <div className="grid grid-cols-1 items-end gap-2 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_120px_auto]">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">路径</label>
              <Input
                value={newPath}
                onChange={(e) => setNewPath(e.target.value)}
                placeholder="C:\Users\you\extra-skills"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">标签</label>
              <Input
                value={newLabel}
                onChange={(e) => setNewLabel(e.target.value)}
                placeholder="可选名称"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">子目录</label>
              <Input
                value={newSubdir}
                onChange={(e) => setNewSubdir(e.target.value)}
                placeholder="skills"
              />
            </div>
            <Button variant="primary" onClick={handleAddPath} disabled={busy || !newPath.trim()}>
              <FolderPlus size={14} />
              添加
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>关于</CardTitle>
          <CardDescription>Tauri + React + Tailwind CSS</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col gap-2 text-[13px]">
            <div className="flex min-h-8 items-center justify-between rounded-md bg-[var(--color-surface)] px-3">
              <span className="text-[var(--color-text-secondary)]">UI</span>
              <span className="font-medium">Tauri / React / Tailwind CSS</span>
            </div>
            <div className="flex min-h-8 items-center justify-between rounded-md bg-[var(--color-surface)] px-3">
              <span className="text-[var(--color-text-secondary)]">窗口</span>
              <span className="font-medium">透明无边框，自绘标题栏</span>
            </div>
            <div className="flex min-h-8 items-center justify-between rounded-md bg-[var(--color-surface)] px-3">
              <span className="text-[var(--color-text-secondary)]">版本</span>
              <span className="font-medium">0.1.0</span>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
