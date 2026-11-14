const RIPPLE_TARGETS = ".btn, .btn-icon, .nav-item, .settings-mode-option, .settings-theme-option, .transport-tab, .quick-action, .skill-item, .settings-anchor";

/** M3 风格下为按钮等可点击元素生成涟漪；其他风格不生效。返回卸载函数。 */
export function installRipple(): () => void {
  function handlePointerDown(event: PointerEvent) {
    if (event.button !== 0 || document.documentElement.dataset.style !== "m3") return;
    const target = event.target instanceof Element ? event.target.closest<HTMLElement>(RIPPLE_TARGETS) : null;
    if (!target || target.matches(":disabled")) return;
    const rect = target.getBoundingClientRect();
    const size = Math.max(rect.width, rect.height) * 2.2;
    const ripple = document.createElement("span");
    ripple.className = "m3-ripple";
    ripple.style.width = ripple.style.height = `${size}px`;
    ripple.style.left = `${event.clientX - rect.left - size / 2}px`;
    ripple.style.top = `${event.clientY - rect.top - size / 2}px`;
    target.appendChild(ripple);
    window.setTimeout(() => ripple.remove(), 600);
  }

  document.addEventListener("pointerdown", handlePointerDown);
  return () => document.removeEventListener("pointerdown", handlePointerDown);
}
