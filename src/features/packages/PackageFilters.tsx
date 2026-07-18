import { SOURCE_LABELS } from "../../shared/types/package";
import type { KindFilter, SourceFilter, ViewMode } from "./usePackages";
import { Select } from "../../shared/components/Select";
import { RefreshIcon, SearchIcon } from "../../shared/components/icons";

const SOURCE_OPTIONS: { value: SourceFilter; label: string }[] = [
  { value: "all", label: "All Sources" },
  ...(["apt", "snap", "flatpak", "appimage"] as const).map((s) => ({
    value: s,
    label: SOURCE_LABELS[s],
  })),
];

const KIND_OPTIONS: { value: KindFilter; label: string }[] = [
  { value: "all", label: "Any kind" },
  { value: "gui", label: "GUI" },
  { value: "cli", label: "CLI" },
  { value: "unknown", label: "Other" },
];

export function PackageFilters({
  query,
  source,
  kind,
  viewMode,
  updatesCount,
  refreshing,
  onQuery,
  onSource,
  onKind,
  onViewMode,
  onRescan,
}: {
  query: string;
  source: SourceFilter;
  kind: KindFilter;
  viewMode: ViewMode;
  updatesCount: number;
  refreshing: boolean;
  onQuery: (q: string) => void;
  onSource: (s: SourceFilter) => void;
  onKind: (k: KindFilter) => void;
  onViewMode: (v: ViewMode) => void;
  onRescan: () => void;
}) {
  return (
    <div className="filters">
      <div className="filters__search-wrap">
        <SearchIcon size={15} className="filters__search-icon" />
        <input
          className="filters__search"
          type="search"
          placeholder="Search applications…"
          value={query}
          autoFocus
          onChange={(e) => onQuery(e.target.value)}
        />
      </div>

      <div className="view-toggle" role="tablist" aria-label="View mode">
        <button
          type="button"
          role="tab"
          aria-selected={viewMode === "uninstall"}
          className={`view-toggle__btn${viewMode === "uninstall" ? " view-toggle__btn--active" : ""}`}
          onClick={() => onViewMode("uninstall")}
        >
          Uninstall
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={viewMode === "updates"}
          className={`view-toggle__btn${viewMode === "updates" ? " view-toggle__btn--active" : ""}`}
          onClick={() => onViewMode("updates")}
        >
          Updates
          {updatesCount > 0 && (
            <span className="view-toggle__count">{updatesCount}</span>
          )}
        </button>
      </div>

      <span className="filters__spacer" />

      <Select
        options={SOURCE_OPTIONS}
        value={source}
        onChange={onSource}
        ariaLabel="Filter by source"
      />
      <Select
        options={KIND_OPTIONS}
        value={kind}
        onChange={onKind}
        ariaLabel="Filter by kind"
        iconTrigger
      />
      <button
        type="button"
        className="btn btn--ghost btn--icon filters__rescan"
        onClick={onRescan}
        disabled={refreshing}
        title="Rescan installed packages"
      >
        <RefreshIcon size={15} className={refreshing ? "spin" : ""} />
      </button>
    </div>
  );
}
