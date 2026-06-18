import { useState } from "react";
import type { View } from "../../App";

const navItems: Array<{ id: View; label: string; icon: string }> = [
  { id: "overview", label: "概览", icon: "M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z" },
  { id: "skills", label: "Skills", icon: "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" },
  { id: "catalog", label: "skills 仓库", icon: "M4 19.5A2.5 2.5 0 0 1 6.5 17H20M4 4.5A2.5 2.5 0 0 1 6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5z" },
  { id: "mcp", label: "MCP", icon: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z" },
  { id: "agents", label: "Agents", icon: "M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8z" },
];

interface SidebarProps {
  view: View;
  onNavigate: (view: View) => void;
  skillCount?: number;
  agentCount?: number;
}

export function Sidebar({ view, onNavigate, skillCount = 0, agentCount = 0 }: SidebarProps) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <aside className={`sidebar${collapsed ? " collapsed" : ""}`}>
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <div className="sidebar-logo-icon">S</div>
          <div>
            <div className="sidebar-logo-text">Skills Manager</div>
            <div className="sidebar-logo-badge">v0.1.0</div>
          </div>
        </div>
      </div>
      <nav className="sidebar-nav">
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "8px 12px 6px" }}>
          <span className="nav-section-label" style={{ padding: 0 }}>导航</span>
          <button
            className="sidebar-toggle"
            onClick={() => setCollapsed((c) => !c)}
            title={collapsed ? "展开侧边栏" : "收纳侧边栏"}
            type="button"
            style={{ margin: 0 }}
          >
            <svg className="icon icon-sm" viewBox="0 0 24 24">
              <polyline points={collapsed ? "9 18 15 12 9 6" : "15 18 9 12 15 6"} />
            </svg>
          </button>
        </div>
        {navItems.map((item) => (
          <button
            key={item.id}
            className={`nav-item${view === item.id ? " active" : ""}`}
            onClick={() => onNavigate(item.id)}
            type="button"
            title={collapsed ? item.label : undefined}
          >
            <svg className="icon" viewBox="0 0 24 24">
              <path d={item.icon} />
            </svg>
            <span>{item.label}</span>
          </button>
        ))}
      </nav>
      <div className="sidebar-footer">
        <div className="sidebar-stat">
          <span className="sidebar-stat-dot" />
          <span className="sidebar-footer-text">已识别 {skillCount} 个 skills，{agentCount} 个 agents</span>
        </div>
      </div>
    </aside>
  );
}
