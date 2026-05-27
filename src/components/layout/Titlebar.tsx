import { MouseEvent, useEffect, useState } from "react";
import { Maximize2, Minus, Sparkles, X } from "lucide-react";
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
    <header
      className="flex h-11 shrink-0 select-none items-center border-b border-[var(--color-border)] bg-[rgb(253_253_252_/_0.92)]"
      onMouseDown={startDrag}
      onDoubleClick={toggleMaximize}
    >
      <div className="flex items-center gap-3 pl-4">
        <div className="flex h-6 w-6 items-center justify-center rounded-md bg-[var(--color-accent)] text-white shadow-sm">
          <Sparkles size={12} />
        </div>
        <span className="text-[13px] font-semibold text-[var(--color-text)]">Skills Manager</span>
      </div>
      <div className="flex-1 h-full" />
      <div
        className="flex h-full"
        onMouseDown={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <button
          className="flex h-full w-11 items-center justify-center text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface)] hover:text-[var(--color-text)]"
          onClick={() => appWindow()?.minimize()}
          title="最小化"
          type="button"
        >
          <Minus size={14} />
        </button>
        <button
          className="flex h-full w-11 items-center justify-center text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface)] hover:text-[var(--color-text)]"
          onClick={toggleMaximize}
          title={maximized ? "还原" : "最大化"}
          type="button"
        >
          <Maximize2 size={13} />
        </button>
        <button
          className="flex h-full w-11 items-center justify-center text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-danger)] hover:text-white"
          onClick={() => appWindow()?.close()}
          title="关闭"
          type="button"
        >
          <X size={14} />
        </button>
      </div>
    </header>
  );
}
