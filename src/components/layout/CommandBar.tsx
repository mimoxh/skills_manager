interface CommandBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  busy: boolean;
  onRefresh: () => void;
  onFolder?: () => void;
  onArchive?: () => void;
}

export function CommandBar({ query, onQueryChange, busy, onRefresh, onFolder, onArchive }: CommandBarProps) {
  return (
    <div className="topbar">
      <div className="search-box">
        <svg className="icon icon-sm search-icon" viewBox="0 0 24 24">
          <circle cx="11" cy="11" r="8" />
          <path d="M21 21l-4.35-4.35" />
        </svg>
        <input
          type="text"
          placeholder="搜索 skills、agents 或路径..."
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
        />
      </div>
      {onFolder && (
        <button className="btn btn-secondary btn-sm" onClick={onFolder} disabled={busy} type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
          文件夹
        </button>
      )}
      {onArchive && (
        <button className="btn btn-secondary btn-sm" onClick={onArchive} disabled={busy} type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>
          zip
        </button>
      )}
      <button className="btn btn-secondary btn-sm" onClick={onRefresh} disabled={busy} type="button">
        <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
        刷新
      </button>
    </div>
  );
}
