import type { PackageSource } from "../../shared/types/package";
import { SOURCE_COLORS, SOURCE_LABELS } from "../../shared/types/package";
import type { KindFilter, SourceFilter } from "./usePackages";

const SOURCES: (PackageSource | "all")[] = ["all", "apt", "snap", "flatpak", "appimage"];
const KINDS: { id: KindFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "gui", label: "GUI" },
  { id: "cli", label: "CLI" },
  { id: "unknown", label: "Other" },
];

export function PackageFilters({
  query,
  source,
  kind,
  count,
  total,
  onQuery,
  onSource,
  onKind,
}: {
  query: string;
  source: SourceFilter;
  kind: KindFilter;
  count: number;
  total: number;
  onQuery: (q: string) => void;
  onSource: (s: SourceFilter) => void;
  onKind: (k: KindFilter) => void;
}) {
  return (
    <div className="filters">
      <div className="filters__row">
        <input
          className="filters__search"
          type="search"
          placeholder="Search installed apps by name, description, category…"
          value={query}
          autoFocus
          onChange={(e) => onQuery(e.target.value)}
        />
      </div>
      <div className="filters__row filters__row--chips">
        <div className="chips" role="group" aria-label="Filter by source">
          {SOURCES.map((s) => {
            const active = source === s;
            const color = s === "all" ? undefined : SOURCE_COLORS[s as PackageSource];
            return (
              <button
                key={s}
                type="button"
                className={`chip${active ? " chip--active" : ""}`}
                style={
                  active && color
                    ? { background: color, borderColor: color, color: "#fff" }
                    : undefined
                }
                onClick={() => onSource(s)}
              >
                {s === "all" ? "All sources" : SOURCE_LABELS[s as PackageSource]}
              </button>
            );
          })}
        </div>
        <div className="chips" role="group" aria-label="Filter by kind">
          {KINDS.map((k) => (
            <button
              key={k.id}
              type="button"
              className={`chip chip--kind${kind === k.id ? " chip--active" : ""}`}
              onClick={() => onKind(k.id)}
            >
              {k.label}
            </button>
          ))}
        </div>
        <span className="filters__count">
          {count} / {total}
        </span>
      </div>
    </div>
  );
}