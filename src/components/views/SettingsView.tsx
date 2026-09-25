import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Palette, ResolvedTheme, ThemeMode } from "../../hooks/useTheme";
import type { AgentProfile, SyncConfig, SyncConflict, SyncConflictChoice, SyncStatus } from "../../types";
import { ConflictDialog } from "./ConflictDialog";

interface SettingsViewProps {
  palette: Palette;
  themeMode: ThemeMode;
  resolvedTheme: ResolvedTheme;
  onPaletteChange: (palette: Palette) => void;
  onThemeChange: (mode: ThemeMode) => void;
  syncConfig: SyncConfig | null;
  syncStatus: SyncStatus | null;
  syncConflicts: SyncConflict[];
  syncBusy: boolean;
  onSaveSyncConfig: (config: SyncConfig, secretAccessKey?: string, encryptPassword?: string) => Promise<void>;
  onTestSyncConnection: (config: SyncConfig) => Promise<void>;
  onSyncNow: () => Promise<void>;
  onResolveSyncConflict: (skillId: string, choice: SyncConflictChoice) => Promise<void>;
  onSyncGc: () => Promise<void>;
  agents?: AgentProfile[];
}

const emptySyncConfig: SyncConfig = {
  enabled: false,
  endpoint: "",
  bucket: "",
  region: "us-east-1",
  pathStyle: true,
  accessKeyId: "",
  pollSecs: 60,
  encrypt: true,
};

// 主题风格选项：value / 名称 / 描述 / 迷你预览色（表面、次级表面、强调色）
const paletteOptions: Array<{
  value: Palette;
  label: string;
  desc: string;
  swatch: [string, string, string];
}> = [
  {
    value: "modern",
    label: "清爽极简",
    desc: "纯白画布 + 蓝色点缀，现代干净，适合专注工作",
    swatch: ["#ffffff", "#f4f4f5", "#3b82f6"],
  },
  {
    value: "parchment",
    label: "经典暖色",
    desc: "象牙奶油 + 琥珀暖金，温润通透质感",
    swatch: ["#ffffff", "#f5efe6", "#d97706"],
  },
];

const themeModeOptions: Array<{ mode: ThemeMode; label: string }> = [
  { mode: "system", label: "跟随系统" },
  { mode: "light", label: "浅色" },
  { mode: "dark", label: "深色" },
];

export function SettingsView({
  palette,
  themeMode,
  resolvedTheme,
  onPaletteChange,
  onThemeChange,
  syncConfig,
  syncStatus,
  syncConflicts,
  syncBusy,
  onSaveSyncConfig,
  onTestSyncConnection,
  onSyncNow,
  onResolveSyncConflict,
  onSyncGc,
  agents = [],
}: SettingsViewProps) {
  const [draft, setDraft] = useState<SyncConfig>(syncConfig ?? emptySyncConfig);
  const [secretKey, setSecretKey] = useState("");
  const [password, setPassword] = useState("");
  const [conflictOpen, setConflictOpen] = useState(false);

  const [exePath, setExePath] = useState<string>("SkillsManager.exe");
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [registering, setRegistering] = useState(false);
  const [registerResult, setRegisterResult] = useState<string | null>(null);

  useEffect(() => {
    api.getSelfExecutablePath().then(setExePath).catch(() => {});
  }, []);

  function copyText(key: string, text: string) {
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  }

  async function handleRegisterSelf() {
    const mcpAgentIds = (agents || [])
      .filter((a) =>
        ["codex", "claudeCode", "opencode", "trae"].includes(a.type) ||
        (a.type === "custom" && a.adapterConfig?.mcpFormat)
      )
      .map((a) => a.id);

    if (mcpAgentIds.length === 0) {
      setRegisterResult("未检测到支持 MCP 的本地 Agent (Claude Code / Codex / OpenCode / Trae)。");
      return;
    }

    setRegistering(true);
    setRegisterResult(null);
    try {
      const results = await api.registerSelfAsMcp(mcpAgentIds);
      const successful = results.filter((r) => r.action !== "error").length;
      setRegisterResult(`已成功将 Skills Manager 写入 ${successful} 个 Agent 的 MCP 配置中！`);
    } catch (e: unknown) {
      setRegisterResult(`写入失败: ${String(e)}`);
    } finally {
      setRegistering(false);
    }
  }

  useEffect(() => {
    if (syncConfig) setDraft(syncConfig);
  }, [syncConfig]);

  function updateDraft<K extends keyof SyncConfig>(key: K, value: SyncConfig[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  async function handleSave() {
    await onSaveSyncConfig(draft, secretKey || undefined, password || undefined);
    setSecretKey("");
    setPassword("");
  }

  const conflictCount = syncConflicts.length;
  const statusText = !syncStatus
    ? "未加载"
    : !syncStatus.configured
      ? "未配置"
      : syncStatus.enabled
        ? syncStatus.running
          ? "自动同步中"
          : "已启用"
        : "已停用";

  return (
    <>
      <div className="view-header">
        <div>
          <div className="view-title">设置</div>
          <div className="view-subtitle">外观主题与应用信息</div>
        </div>
      </div>

      {/* 外观 */}
      <div className="card" style={{ flexShrink: 0, marginBottom: 20 }}>
        <div className="card-header">
          <div>
            <div className="card-title">外观</div>
            <div className="card-desc">主题风格与明暗模式可自由组合</div>
          </div>
        </div>
        <div className="card-body" style={{ display: "flex", flexDirection: "column", gap: 24 }}>
          {/* 主题风格 */}
          <div>
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 10 }}>
              主题风格
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 12 }}>
              {paletteOptions.map((option) => {
                const active = palette === option.value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => onPaletteChange(option.value)}
                    aria-pressed={active}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      gap: 10,
                      textAlign: "left",
                      cursor: "pointer",
                      padding: 14,
                      borderRadius: "var(--radius-md)",
                      border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                      background: active ? "var(--accent-soft)" : "var(--surface)",
                      transition: "border-color .15s ease, background .15s ease, box-shadow .15s ease",
                      ...(active ? { boxShadow: `0 2px 10px var(--accent-glow)` } : {}),
                    }}
                  >
                    <SwatchPreview colors={option.swatch} />
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <span style={{ fontSize: 13.5, fontWeight: 600, color: "var(--text)" }}>{option.label}</span>
                      {active && <span className="badge badge-success">当前</span>}
                    </div>
                    <span style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.55 }}>{option.desc}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* 主题模式 */}
          <div>
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 10 }}>
              主题模式
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {themeModeOptions.map((option) => {
                const active = themeMode === option.mode;
                return (
                  <button
                    key={option.mode}
                    type="button"
                    onClick={() => onThemeChange(option.mode)}
                    aria-pressed={active}
                    style={{
                      display: "inline-flex",
                      alignItems: "center",
                      gap: 8,
                      padding: "9px 16px",
                      borderRadius: "var(--radius-sm)",
                      border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                      background: active ? "var(--accent-light)" : "var(--surface-raised)",
                      color: active ? "var(--accent)" : "var(--text)",
                      fontSize: 13,
                      fontWeight: active ? 600 : 500,
                      cursor: "pointer",
                      transition: "all .14s ease",
                    }}
                  >
                    <ModeDot mode={option.mode} />
                    {option.label}
                    {option.mode === "system" && (
                      <span style={{ fontSize: 11.5, color: "var(--text-tertiary)" }}>
                        （当前 {resolvedTheme === "dark" ? "深色" : "浅色"}）
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
            <div style={{ marginTop: 10, fontSize: 12, color: "var(--text-tertiary)" }}>
              当前外观：{palette === "modern" ? "清爽极简" : "经典暖色"} · {resolvedTheme === "dark" ? "深色" : "浅色"}
            </div>
          </div>
        </div>
      </div>

      {/* 同步 */}
      <div className="card" style={{ flexShrink: 0, marginBottom: 20 }}>
        <div className="card-header" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div>
            <div className="card-title">Skills 同步</div>
            <div className="card-desc">通过 S3 兼容网关或本地目录，在多设备间同步中枢 skills（客户端加密）</div>
          </div>
          <span className={`badge ${syncStatus?.lastError ? "badge-warning" : syncStatus?.enabled ? "badge-success" : ""}`}>
            {statusText}
          </span>
        </div>
        <div className="card-body" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 14 }}>
            <div className="input-group">
              <label className="input-label">Endpoint</label>
              <input className="input" value={draft.endpoint} placeholder="http://192.168.1.10:5246 或 local://D:/sync-bucket" onChange={(e) => updateDraft("endpoint", e.target.value)} />
              <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>
                单机测试可填 <code>local://绝对路径</code>；S3 填 http(s) 地址。本地目录时 Bucket 可留空或作子目录名。
              </div>
            </div>
            <div className="input-group">
              <label className="input-label">Bucket</label>
              <input className="input" value={draft.bucket} placeholder="test" onChange={(e) => updateDraft("bucket", e.target.value)} />
            </div>
            <div className="input-group">
              <label className="input-label">Region</label>
              <input className="input" value={draft.region} placeholder="us-east-1" onChange={(e) => updateDraft("region", e.target.value)} />
            </div>
            <div className="input-group">
              <label className="input-label">Access Key ID</label>
              <input className="input" value={draft.accessKeyId} onChange={(e) => updateDraft("accessKeyId", e.target.value)} />
            </div>
            <div className="input-group">
              <label className="input-label">Secret Access Key</label>
              <input className="input" type="password" value={secretKey} placeholder="留空则保持不变" onChange={(e) => setSecretKey(e.target.value)} />
            </div>
            <div className="input-group">
              <label className="input-label">加密口令</label>
              <input className="input" type="password" value={password} placeholder="留空则保持不变" onChange={(e) => setPassword(e.target.value)} />
            </div>
            <div className="input-group">
              <label className="input-label">兜底轮询间隔（秒）</label>
              <input className="input" type="number" min={5} value={draft.pollSecs} onChange={(e) => updateDraft("pollSecs", Number(e.target.value) || 60)} />
            </div>
            <div className="input-group" style={{ display: "flex", flexDirection: "column", justifyContent: "flex-end", gap: 10 }}>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text)" }}>
                <input type="checkbox" checked={draft.enabled} onChange={(e) => updateDraft("enabled", e.target.checked)} />
                启用自动同步
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text)" }}>
                <input type="checkbox" checked={draft.encrypt} onChange={(e) => updateDraft("encrypt", e.target.checked)} />
                客户端加密
              </label>
            </div>
          </div>

          <div className="separator" />

          <div>
            <div className="input-label" style={{ marginBottom: 8 }}>Skills 中枢</div>
            <div style={{ marginTop: 8, fontSize: 12, color: "var(--text-tertiary)" }}>
              Skills Manager 添加的 Skill 保存在 <code style={{ fontFamily: "var(--font-mono)" }}>{agents.find((agent) => agent.type === "universal")?.skillsPath ?? "<程序目录>\\skills"}</code>；仅向选中的 Agent 分发，已有独立 Skill 保持不变。
            </div>
          </div>

          <div className="separator" />

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.7 }}>
              <div>设备：{syncStatus?.deviceName || syncStatus?.deviceId || "—"}</div>
              <div>上次同步：{syncStatus?.lastRunAt ? new Date(syncStatus.lastRunAt).toLocaleString() : "从未"}</div>
              {syncStatus?.lastError && <div style={{ color: "var(--danger, #dc2626)" }}>错误：{syncStatus.lastError}</div>}
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {conflictCount > 0 && (
                <button className="btn btn-secondary" type="button" onClick={() => setConflictOpen(true)}>
                  处理冲突 ({conflictCount})
                </button>
              )}
              <button className="btn btn-secondary" type="button" disabled={syncBusy} onClick={() => void onTestSyncConnection(draft)}>
                测试连接
              </button>
              <button className="btn btn-secondary" type="button" disabled={syncBusy} onClick={() => void onSyncGc()}>
                清理未引用对象
              </button>
              <button className="btn btn-secondary" type="button" disabled={syncBusy} onClick={handleSave}>
                保存配置
              </button>
              <button className="btn btn-primary" type="button" disabled={syncBusy} onClick={() => void onSyncNow()}>
                立即同步
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* 外部 Agent 接入 (MCP / CLI) */}
      <div className="card" style={{ flexShrink: 0, marginBottom: 20 }}>
        <div className="card-header">
          <div>
            <div className="card-title">外部 Agent 接入 (MCP Server / CLI)</div>
            <div className="card-desc">
              让 Claude Code、Cursor、Windsurf、Claude Desktop 等外部智能体直接调用管理技能与 MCP
            </div>
          </div>
        </div>
        <div className="card-body" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div>
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
              当前程序物理路径
            </div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <input className="input" readOnly value={exePath} style={{ flex: 1, fontSize: 12 }} />
              <button
                className="btn btn-secondary btn-sm"
                type="button"
                onClick={() => copyText("exe", exePath)}
              >
                {copiedKey === "exe" ? "已复制" : "复制路径"}
              </button>
            </div>
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text)" }}>快速接入配置</div>

            {/* Claude Code */}
            <div style={{ padding: "12px 14px", background: "var(--surface-raised)", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
                <span style={{ fontSize: 12.5, fontWeight: 600, color: "var(--text)" }}>Claude Code 命令行接入</span>
                <button
                  className="btn btn-secondary btn-sm"
                  type="button"
                  style={{ fontSize: 11, padding: "2px 8px" }}
                  onClick={() => copyText("claudeCode", `claude mcp add skills-manager -- "${exePath}" mcp`)}
                >
                  {copiedKey === "claudeCode" ? "已复制" : "复制代码"}
                </button>
              </div>
              <code style={{ fontSize: 11, display: "block", background: "var(--surface)", padding: "6px 8px", borderRadius: 4, color: "var(--text-secondary)", wordBreak: "break-all" }}>
                claude mcp add skills-manager -- "{exePath}" mcp
              </code>
            </div>

            {/* Cursor / Claude Desktop */}
            <div style={{ padding: "12px 14px", background: "var(--surface-raised)", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
                <span style={{ fontSize: 12.5, fontWeight: 600, color: "var(--text)" }}>Cursor / Claude Desktop 配置片段 (mcp.json)</span>
                <button
                  className="btn btn-secondary btn-sm"
                  type="button"
                  style={{ fontSize: 11, padding: "2px 8px" }}
                  onClick={() => {
                    const jsonSnippet = JSON.stringify({
                      mcpServers: {
                        "skills-manager": {
                          command: exePath,
                          args: ["mcp"],
                        },
                      },
                    }, null, 2);
                    copyText("mcpJson", jsonSnippet);
                  }}
                >
                  {copiedKey === "mcpJson" ? "已复制" : "复制 JSON"}
                </button>
              </div>
              <pre style={{ margin: 0, fontSize: 11, background: "var(--surface)", padding: "8px 10px", borderRadius: 4, color: "var(--text-secondary)", overflowX: "auto" }}>
{`{
  "mcpServers": {
    "skills-manager": {
      "command": "${exePath.replace(/\\/g, "\\\\")}",
      "args": ["mcp"]
    }
  }
}`}
              </pre>
            </div>
          </div>

          <div style={{ borderTop: "1px solid var(--border)", paddingTop: 14, display: "flex", flexDirection: "column", gap: 8 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: 8 }}>
              <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                一键自动将自身配置为本机检测到的 Claude Code / Codex / OpenCode / Trae 的 MCP Server
              </div>
              <button
                className="btn btn-primary btn-sm"
                type="button"
                disabled={registering}
                onClick={handleRegisterSelf}
              >
                {registering ? "正在配置..." : "一键注入到本机 Agent"}
              </button>
            </div>
            {registerResult && (
              <div style={{ fontSize: 12, color: registerResult.includes("成功") ? "var(--success)" : "var(--danger, #dc2626)" }}>
                {registerResult}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 关于 */}
      <div className="card" style={{ flexShrink: 0, marginBottom: 20 }}>
        <div className="card-header">
          <div>
            <div className="card-title">关于</div>
            <div className="card-desc">Skills Manager 信息</div>
          </div>
        </div>
        <div className="card-body">
          <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
            <div
              style={{
                width: 44,
                height: 44,
                borderRadius: "var(--radius-md)",
                background: "var(--accent)",
                color: "#fff",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 18,
                fontWeight: 700,
                flexShrink: 0,
              }}
            >
              S
            </div>
            <div>
              <div style={{ fontSize: 15, fontWeight: 650, color: "var(--text)" }}>Skills Manager</div>
              <div style={{ fontSize: 12.5, color: "var(--text-secondary)", marginTop: 2 }}>
                v{__APP_VERSION__} · AI Coding Agent 技能统一管理器
              </div>
            </div>
          </div>
          <div className="separator" />
          <div style={{ fontSize: 12.5, color: "var(--text-secondary)", lineHeight: 1.7 }}>
            管理多个 AI Agent（Codex、Claude Code、Claude Cowork、Cursor、Trae 等）的 skills 目录，支持跨 Agent 同步、仓库搜索安装、标签管理与 MCP 服务器配置同步。明暗主题与界面风格可随时切换。
          </div>
        </div>
      </div>

      {conflictOpen && (
        <ConflictDialog
          conflicts={syncConflicts}
          busy={syncBusy}
          onClose={() => setConflictOpen(false)}
          onResolve={(skillId, choice) => { void onResolveSyncConflict(skillId, choice); }}
        />
      )}
    </>
  );
}

/** 主题风格的迷你预览：左侧页面 + 右侧强调色块 */
function SwatchPreview({ colors }: { colors: [string, string, string] }) {
  return (
    <div style={{ display: "flex", gap: 6, height: 46 }}>
      <div
        style={{
          flex: 2,
          borderRadius: 8,
          border: "1px solid var(--border)",
          background: colors[0],
          display: "flex",
          flexDirection: "column",
          gap: 5,
          padding: 7,
        }}
      >
        <div style={{ height: 7, width: "62%", borderRadius: 3, background: colors[2], opacity: 0.92 }} />
        <div style={{ height: 7, width: "82%", borderRadius: 3, background: colors[1] }} />
        <div style={{ height: 7, width: "70%", borderRadius: 3, background: colors[1] }} />
      </div>
      <div
        style={{
          flex: 1,
          borderRadius: 8,
          border: "1px solid var(--border)",
          background: colors[1],
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <div style={{ width: 16, height: 16, borderRadius: 5, background: colors[2] }} />
      </div>
    </div>
  );
}

/** 主题模式的小色块指示 */
function ModeDot({ mode }: { mode: ThemeMode }) {
  if (mode === "system") {
    return (
      <span
        style={{
          width: 16,
          height: 16,
          borderRadius: 5,
          background: "linear-gradient(135deg, #f4f4f5 50%, #27272a 50%)",
          flexShrink: 0,
        }}
      />
    );
  }
  if (mode === "light") {
    return (
      <span
        style={{
          width: 16,
          height: 16,
          borderRadius: 5,
          background: "#fafafa",
          border: "1px solid var(--border)",
          flexShrink: 0,
        }}
      />
    );
  }
  return (
    <span
      style={{
        width: 16,
        height: 16,
        borderRadius: 5,
        background: "#18181b",
        flexShrink: 0,
      }}
    />
  );
}
