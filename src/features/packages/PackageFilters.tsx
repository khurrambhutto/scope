import { SOURCE_LABELS } from "../../shared/types/package";
import type { KindFilter, SourceFilter } from "./usePackages";
import { Select } from "../../shared/components/Select";

const SOURCE_OPTIONS: { value: SourceFilter; label: string }[] = [
  { value: "all", label: "Any source" },
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
  refreshing,
  onQuery,
  onSource,
  onKind,
  onRescan,
}: {
  query: string;
  source: SourceFilter;
  kind: KindFilter;
  refreshing: boolean;
  onQuery: (q: string) => void;
  onSource: (s: SourceFilter) => void;
  onKind: (k: KindFilter) => void;
  onRescan: () => void;
}) {
  return (
    <div className="filters">
      <div className="filters__row">
        <input
          className="filters__search"
          type="search"
          placeholder="Search"
          value={query}
          autoFocus
          onChange={(e) => onQuery(e.target.value)}
        />
        <span style={{ flex: 1 }} />
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
          className="btn btn--ghost btn--icon"
          onClick={onRescan}
          disabled={refreshing}
          title="Rescan"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true" className={refreshing ? "spin" : ""}>
            <path d="M20 11A8.1 8.1 0 0 0 4.5 9M4 5v4h4m-4 4a8.1 8.1 0 0 0 15.5 2m.5 4v-4h-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </button>
      </div>
    </div>
  );
}