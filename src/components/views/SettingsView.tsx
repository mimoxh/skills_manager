import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import type { Palette, ResolvedTheme, ThemeMode } from "../../hooks/useTheme";
import type { AgentProfile, SyncConfig, SyncConflict, SyncConflictChoice, SyncStatus } from "../../types";
import { ConfirmDialog } from "../ui/ConfirmDialog";
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
  onRefreshMcp?: () => Promise<void>;
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

const settingsSections = [
  { id: "settings-general", label: "常用配置" },
  { id: "settings-sync", label: "同步连接" },
  { id: "settings-agents", label: "Agent 目标" },
  { id: "settings-maintenance", label: "数据与维护" },
  { id: "settings-advanced", label: "高级配置" },
] as const;

type SettingsSectionId = (typeof settingsSections)[number]["id"];
type SaveState = "idle" | "saving" | "saved" | "error";

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
  onRefreshMcp,
  agents = [],
}: SettingsViewProps) {
  const [draft, setDraft] = useState<SyncConfig>(syncConfig ?? emptySyncConfig);
  const [pollSecsInput, setPollSecsInput] = useState(String((syncConfig ?? emptySyncConfig).pollSecs));
  const [secretKey, setSecretKey] = useState("");
  const [password, setPassword] = useState("");
  const [conflictOpen, setConflictOpen] = useState(false);
  const [gcConfirmOpen, setGcConfirmOpen] = useState(false);
  const [activeSection, setActiveSection] = useState<SettingsSectionId>(settingsSections[0].id);
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const pageRef = useRef<HTMLDivElement>(null);

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
      await onRefreshMcp?.();
      setRegisterResult(`已成功将 Skills Manager 写入 ${successful} 个 Agent 的 MCP 配置中！`);
    } catch (e: unknown) {
      setRegisterResult(`写入失败: ${String(e)}`);
    } finally {
      setRegistering(false);
    }
  }

  useEffect(() => {
    if (syncConfig) {
      setDraft(syncConfig);
      setPollSecsInput(String(syncConfig.pollSecs));
    }
  }, [syncConfig]);

  useEffect(() => {
    const page = pageRef.current;
    const scrollRoot = page?.closest(".content");
    if (!page || !scrollRoot) return;

    let frame = 0;
    const updateActiveSection = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const rootTop = scrollRoot.getBoundingClientRect().top;
        let current: SettingsSectionId = settingsSections[0].id;
        for (const section of settingsSections) {
          const element = page.querySelector<HTMLElement>(`#${section.id}`);
          if (element && element.getBoundingClientRect().top <= rootTop + 132) current = section.id;
        }
        setActiveSection(current);
      });
    };

    updateActiveSection();
    scrollRoot.addEventListener("scroll", updateActiveSection, { passive: true });
    window.addEventListener("resize", updateActiveSection);
    return () => {
      cancelAnimationFrame(frame);
      scrollRoot.removeEventListener("scroll", updateActiveSection);
      window.removeEventListener("resize", updateActiveSection);
    };
  }, []);

  function updateDraft<K extends keyof SyncConfig>(key: K, value: SyncConfig[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
    setSaveState("idle");
  }

  async function handleSave() {
    const normalizedPollSecs = Math.max(5, Number(pollSecsInput) || Number(draft.pollSecs) || 60);
    const nextDraft: SyncConfig = { ...draft, pollSecs: normalizedPollSecs };
    setDraft(nextDraft);
    setPollSecsInput(String(normalizedPollSecs));
    setSaveState("saving");
    try {
      await onSaveSyncConfig(nextDraft, secretKey || undefined, password || undefined);
      setSecretKey("");
      setPassword("");
      setSaveState("saved");
    } catch {
      setSaveState("error");
    }
  }

  function scrollToSection(sectionId: string) {
    const section = pageRef.current?.querySelector<HTMLElement>(`#${sectionId}`);
    section?.scrollIntoView({ behavior: "smooth", block: "start" });
    section?.focus({ preventScroll: true });
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

  const isDirty = JSON.stringify(draft) !== JSON.stringify(syncConfig ?? emptySyncConfig) || Boolean(secretKey || password);
  const supportedAgentCount = agents.filter((agent) =>
    ["codex", "claudeCode", "opencode", "trae"].includes(agent.type) ||
    (agent.type === "custom" && agent.adapterConfig?.mcpFormat)
  ).length;
  const saveText = saveState === "saving"
    ? "正在保存"
    : saveState === "error"
      ? "保存失败"
      : isDirty
        ? "有未保存更改"
        : saveState === "saved"
          ? "已保存"
          : "配置已保存";

  return (
    <div className="settings-page" ref={pageRef}>
      <div className="view-header">
        <div>
          <div className="view-title">设置</div>
          <div className="view-subtitle">集中管理外观、跨设备同步与 Agent 接入；高级选项按需展开。</div>
        </div>
      </div>

      <div className="settings-summary" aria-label="设置状态摘要">
        <SummaryItem label="同步状态" value={statusText} tone={syncStatus?.lastError ? "warning" : syncStatus?.enabled ? "success" : "neutral"} />
        <SummaryItem label="可接入 Agent" value={`${supportedAgentCount} 个`} tone="neutral" />
        <SummaryItem label="待处理冲突" value={`${conflictCount} 个`} tone={conflictCount > 0 ? "warning" : "success"} />
      </div>

      <nav className="settings-anchor-nav" aria-label="设置分区导航">
        <div className="settings-anchor-scroll">
          {settingsSections.map((section) => (
            <button
              key={section.id}
              type="button"
              className={`settings-anchor${activeSection === section.id ? " active" : ""}`}
              aria-current={activeSection === section.id ? "location" : undefined}
              onClick={() => scrollToSection(section.id)}
            >
              {section.label}
            </button>
          ))}
        </div>
        <div className={`settings-save-state ${saveState === "error" ? "error" : isDirty ? "dirty" : ""}`} role="status">
          <span className="settings-save-dot" />
          <span>{saveText}</span>
        </div>
        <button className="btn btn-primary btn-sm" type="button" disabled={syncBusy || !isDirty} onClick={() => void handleSave()}>
          {saveState === "saving" ? "保存中..." : "保存同步配置"}
        </button>
      </nav>

      <section id="settings-general" className="settings-section" tabIndex={-1}>
        <SectionHeading title="常用配置" description="调整应用的视觉风格与明暗模式。" />
        <div className="card settings-card">
          <div className="card-header">
            <div className="card-title">外观</div>
            <div className="card-desc">主题风格与明暗模式可自由组合</div>
          </div>
          <div className="card-body settings-stack-lg">
            <div>
              <div className="settings-field-title">主题风格</div>
              <div className="settings-theme-grid">
                {paletteOptions.map((option) => {
                  const active = palette === option.value;
                  return (
                    <button
                      key={option.value}
                      type="button"
                      onClick={() => onPaletteChange(option.value)}
                      aria-pressed={active}
                      className={`settings-theme-option${active ? " active" : ""}`}
                    >
                      <SwatchPreview colors={option.swatch} />
                      <div className="settings-inline">
                        <span className="settings-option-title">{option.label}</span>
                        {active && <span className="badge badge-success">当前</span>}
                      </div>
                      <span className="settings-option-desc">{option.desc}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div>
              <div className="settings-field-title">主题模式</div>
              <div className="settings-mode-row">
                {themeModeOptions.map((option) => {
                  const active = themeMode === option.mode;
                  return (
                    <button
                      key={option.mode}
                      type="button"
                      onClick={() => onThemeChange(option.mode)}
                      aria-pressed={active}
                      className={`settings-mode-option${active ? " active" : ""}`}
                    >
                      <ModeDot mode={option.mode} />
                      {option.label}
                      {option.mode === "system" && <span className="settings-muted">当前 {resolvedTheme === "dark" ? "深色" : "浅色"}</span>}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      </section>

      <section id="settings-sync" className="settings-section" tabIndex={-1}>
        <SectionHeading title="同步连接" description="配置多设备同步的目标位置与启用状态。" />
        <div className="card settings-card">
          <div className="card-header settings-card-header-row">
            <div>
              <div className="card-title">连接位置</div>
              <div className="card-desc">支持 S3 兼容网关或本地目录</div>
            </div>
            <span className={`badge ${syncStatus?.lastError ? "badge-warning" : syncStatus?.enabled ? "badge-success" : ""}`}>{statusText}</span>
          </div>
          <div className="card-body settings-stack-lg">
            <div className="settings-grid-two">
              <div className="input-group">
                <label className="input-label" htmlFor="sync-endpoint">Endpoint</label>
                <input id="sync-endpoint" className="input" value={draft.endpoint} placeholder="http://192.168.1.10:5246 或 local://D:/sync-bucket" onChange={(e) => updateDraft("endpoint", e.target.value)} />
                <div className="settings-help">本地测试使用 <code>local://绝对路径</code>；S3 使用 http(s) 地址。</div>
              </div>
              <div className="input-group">
                <label className="input-label" htmlFor="sync-bucket">Bucket</label>
                <input id="sync-bucket" className="input" value={draft.bucket} placeholder="test" onChange={(e) => updateDraft("bucket", e.target.value)} />
                <div className="settings-help">本地目录可留空，或作为 Endpoint 下的子目录名。</div>
              </div>
            </div>
            <div className="settings-switch-grid">
              <label className="settings-switch-row">
                <input type="checkbox" checked={draft.enabled} onChange={(e) => updateDraft("enabled", e.target.checked)} />
                <span><strong>启用自动同步</strong><small>后台轮询并监听中枢目录变化</small></span>
              </label>
              <label className="settings-switch-row">
                <input type="checkbox" checked={draft.encrypt} onChange={(e) => updateDraft("encrypt", e.target.checked)} />
                <span><strong>客户端加密</strong><small>上传前在本机加密 Skill 内容</small></span>
              </label>
            </div>
          </div>
        </div>
      </section>

      <section id="settings-agents" className="settings-section" tabIndex={-1}>
        <SectionHeading title="Agent 目标" description="查看中枢位置，并让本机 Agent 接入 Skills Manager。" />
        <div className="card settings-card">
          <div className="card-body settings-stack-lg">
            <div className="settings-hub-row">
              <div>
                <div className="settings-field-title">Skills 中枢</div>
                <div className="settings-help">所有管理器添加的 Skill 先进入中枢，再按设备选择目标 Agent。</div>
              </div>
              <code className="settings-path-code">{agents.find((agent) => agent.type === "universal")?.skillsPath ?? "<程序目录>\\skills"}</code>
            </div>
            <div className="separator" />
            <div className="settings-action-row">
              <div>
                <div className="settings-field-title">本机 Agent 接入</div>
                <div className="settings-help">检测到 {supportedAgentCount} 个支持 MCP 的 Agent，可自动写入 Skills Manager 配置。</div>
              </div>
              <button className="btn btn-primary btn-sm" type="button" disabled={registering} onClick={() => void handleRegisterSelf()}>
                {registering ? "正在配置..." : "一键接入本机 Agent"}
              </button>
            </div>
            {registerResult && <div className={`settings-result ${registerResult.includes("成功") ? "success" : "error"}`}>{registerResult}</div>}

            <details className="settings-disclosure">
              <summary>手动接入与命令行配置</summary>
              <div className="settings-disclosure-body">
                <div>
                  <div className="settings-field-title">当前程序物理路径</div>
                  <div className="settings-copy-row">
                    <input className="input" readOnly value={exePath} />
                    <button className="btn btn-secondary btn-sm" type="button" onClick={() => copyText("exe", exePath)}>{copiedKey === "exe" ? "已复制" : "复制路径"}</button>
                  </div>
                </div>
                <div className="settings-code-card">
                  <div className="settings-code-header">
                    <span>Claude Code 命令行接入</span>
                    <button className="btn btn-secondary btn-sm" type="button" onClick={() => copyText("claudeCode", `claude mcp add skills-manager -- "${exePath}" mcp`)}>{copiedKey === "claudeCode" ? "已复制" : "复制代码"}</button>
                  </div>
                  <code>claude mcp add skills-manager -- "{exePath}" mcp</code>
                </div>
                <div className="settings-code-card">
                  <div className="settings-code-header">
                    <span>Cursor / Claude Desktop 配置片段</span>
                    <button className="btn btn-secondary btn-sm" type="button" onClick={() => copyText("mcpJson", JSON.stringify({ mcpServers: { "skills-manager": { command: exePath, args: ["mcp"] } } }, null, 2))}>{copiedKey === "mcpJson" ? "已复制" : "复制 JSON"}</button>
                  </div>
                  <pre>{`{\n  "mcpServers": {\n    "skills-manager": {\n      "command": "${exePath.replace(/\\/g, "\\\\")}",\n      "args": ["mcp"]\n    }\n  }\n}`}</pre>
                </div>
              </div>
            </details>
          </div>
        </div>
      </section>

      <section id="settings-maintenance" className="settings-section" tabIndex={-1}>
        <SectionHeading title="数据与维护" description="查看设备状态、处理冲突并执行手动维护。" />
        <div className="settings-maintenance-grid">
          <div className="card settings-card">
            <div className="card-header">
              <div className="card-title">同步状态</div>
              <div className="card-desc">当前设备最近一次运行信息</div>
            </div>
            <div className="card-body settings-status-list">
              <div><span>设备</span><strong>{syncStatus?.deviceName || syncStatus?.deviceId || "—"}</strong></div>
              <div><span>上次同步</span><strong>{syncStatus?.lastRunAt ? new Date(syncStatus.lastRunAt).toLocaleString() : "从未"}</strong></div>
              <div><span>冲突</span><strong className={conflictCount > 0 ? "settings-warning-text" : ""}>{conflictCount} 个</strong></div>
              {syncStatus?.lastError && <div className="settings-error-message">{syncStatus.lastError}</div>}
            </div>
          </div>
          <div className="card settings-card">
            <div className="card-header">
              <div className="card-title">手动操作</div>
              <div className="card-desc">按需运行，不改变自动同步设置</div>
            </div>
            <div className="card-body settings-action-list">
              <button className="btn btn-primary" type="button" disabled={syncBusy} onClick={() => void onSyncNow()}>立即同步</button>
              <button className="btn btn-secondary" type="button" disabled={syncBusy || conflictCount === 0} onClick={() => setConflictOpen(true)}>处理冲突 ({conflictCount})</button>
              <button className="btn btn-danger" type="button" disabled={syncBusy} onClick={() => setGcConfirmOpen(true)}>清理未引用对象</button>
            </div>
          </div>
        </div>
      </section>

      <section id="settings-advanced" className="settings-section" tabIndex={-1}>
        <SectionHeading title="高级配置" description="连接凭据、轮询策略和应用信息。" />
        <div className="card settings-card">
          <details className="settings-disclosure settings-disclosure-flat">
            <summary>高级连接与测试</summary>
            <div className="settings-disclosure-body">
              <div className="settings-grid-two">
                <div className="input-group">
                  <label className="input-label" htmlFor="sync-region">Region</label>
                  <input id="sync-region" className="input" value={draft.region} placeholder="us-east-1" onChange={(e) => updateDraft("region", e.target.value)} />
                </div>
                <div className="input-group">
                  <label className="input-label" htmlFor="sync-access-key">Access Key ID</label>
                  <input id="sync-access-key" className="input" value={draft.accessKeyId} onChange={(e) => updateDraft("accessKeyId", e.target.value)} />
                </div>
                <div className="input-group">
                  <label className="input-label" htmlFor="sync-secret-key">Secret Access Key</label>
                  <input id="sync-secret-key" className="input" type="password" value={secretKey} placeholder="留空则保持不变" onChange={(e) => { setSecretKey(e.target.value); setSaveState("idle"); }} />
                </div>
                <div className="input-group">
                  <label className="input-label" htmlFor="sync-password">加密口令</label>
                  <input id="sync-password" className="input" type="password" value={password} placeholder="留空则保持不变" onChange={(e) => { setPassword(e.target.value); setSaveState("idle"); }} />
                </div>
                <div className="input-group">
                  <label className="input-label" htmlFor="sync-poll-secs">兜底轮询间隔（秒）</label>
                  <input
                    id="sync-poll-secs"
                    className="input"
                    type="number"
                    min={5}
                    value={pollSecsInput}
                    onChange={(e) => {
                      const raw = e.target.value;
                      setPollSecsInput(raw);
                      const parsed = Number(raw);
                      if (raw.trim() !== "" && Number.isFinite(parsed)) {
                        updateDraft("pollSecs", parsed);
                      } else {
                        setSaveState("idle");
                      }
                    }}
                    onBlur={() => {
                      const normalized = Math.max(5, Number(pollSecsInput) || Number(draft.pollSecs) || 60);
                      setPollSecsInput(String(normalized));
                      updateDraft("pollSecs", normalized);
                    }}
                  />
                </div>
              </div>
              <div className="settings-advanced-actions">
                <span>凭据与加密口令仅保存在系统钥匙串中。</span>
                <button className="btn btn-secondary" type="button" disabled={syncBusy} onClick={() => void onTestSyncConnection({ ...draft, pollSecs: Math.max(5, Number(pollSecsInput) || Number(draft.pollSecs) || 60) })}>测试连接</button>
              </div>
            </div>
          </details>
        </div>

        <div className="card settings-card settings-about-card">
          <div className="card-body settings-about-row">
            <div className="settings-app-mark">S</div>
            <div>
              <div className="settings-option-title">Skills Manager</div>
              <div className="settings-help">v{__APP_VERSION__} · AI Coding Agent 技能统一管理器</div>
            </div>
          </div>
        </div>
      </section>

      {conflictOpen && (
        <ConflictDialog
          conflicts={syncConflicts}
          busy={syncBusy}
          onClose={() => setConflictOpen(false)}
          onResolve={(skillId, choice) => { void onResolveSyncConflict(skillId, choice); }}
        />
      )}

      {gcConfirmOpen && (
        <ConfirmDialog
          title="清理未引用对象"
          message="将删除未被任何设备清单引用的远端对象。请确认其他设备已完成同步后再继续。"
          confirmLabel="确认清理"
          busy={syncBusy}
          onClose={() => setGcConfirmOpen(false)}
          onConfirm={() => { setGcConfirmOpen(false); void onSyncGc(); }}
        />
      )}
    </div>
  );
}

function SectionHeading({ title, description }: { title: string; description: string }) {
  return (
    <div className="settings-section-heading">
      <h2>{title}</h2>
      <p>{description}</p>
    </div>
  );
}

function SummaryItem({ label, value, tone }: { label: string; value: string; tone: "neutral" | "success" | "warning" }) {
  return (
    <div className="settings-summary-item">
      <span className={`settings-summary-dot ${tone}`} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
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
