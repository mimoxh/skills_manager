import { MouseEvent, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function appWindow() {
  return hasTauriRuntime() ? getCurrentWindow() : null;
}

export function Titlebar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = appWindow();
    if (win) win.isMaximized().then(setMaximized);
  }, []);

  async function startDrag(event: MouseEvent<HTMLElement>) {
    if (event.button !== 0 || event.detail > 1) return;
    await appWindow()?.startDragging();
  }

  async function toggleMaximize() {
    await appWindow()?.toggleMaximize();
    const win = appWindow();
    if (win) setMaximized(await win.isMaximized());
  }

  return (
    <div className="titlebar" onMouseDown={startDrag} onDoubleClick={toggleMaximize}>
      <div className="titlebar-logo">
        <div className="titlebar-logo-icon">
          <svg viewBox="0 0 24 24">
            <polygon points="12 2 2 7 12 12 22 7 12 2" />
            <polyline points="2 17 12 22 22 17" />
            <polyline points="2 12 12 17 22 12" />
          </svg>
        </div>
        <span className="titlebar-text">Skills Manager</span>
      </div>
      <div className="titlebar-spacer" />
      <div onMouseDown={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()} style={{ display: "flex", height: "100%" }}>
        <button className="titlebar-btn" onClick={() => appWindow()?.minimize()} title="最小化" type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><line x1="5" y1="12" x2="19" y2="12" /></svg>
        </button>
        <button className="titlebar-btn" onClick={toggleMaximize} title={maximized ? "还原" : "最大化"} type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" rx="2" /></svg>
        </button>
        <button className="titlebar-btn close" onClick={() => appWindow()?.close()} title="关闭" type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
        </button>
      </div>
    </div>
  );
}
