import { useCallback, useDeferredValue, useEffect, useMemo, useState } from "react";
import { scanPackages, getCachedScan } from "../../shared/api/packages";
import type {
  CachedScan,
  InstalledPackage,
  PackageSource,
  AppKind,
} from "../../shared/types/package";

export type SourceFilter = PackageSource | "all";
export type KindFilter = AppKind | "all";
export type ViewMode = "uninstall" | "updates";

/**
 * Everything a package can be searched by, lowercased. Built once per scan and
 * indexed by package key, so typing never re-allocates per package.
 */
function searchText(pkg: InstalledPackage): string {
  return [
    pkg.name,
    pkg.display_name ?? "",
    pkg.description ?? "",
    pkg.package_id,
    pkg.install_scope ?? "",
    pkg.categories ?? "",
    pkg.version,
  ]
    .join(" ")
    .toLowerCase();
}

export function usePackages() {
  const [scan, setScan] = useState<CachedScan | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");
  const [kindFilter, setKindFilter] = useState<KindFilter>("all");
  const [viewMode, setViewMode] = useState<ViewMode>("uninstall");

  // The search field stays in sync immediately while the filter pass is
  // deferred, which keeps typing smooth as the list grows into the thousands.
  const deferredQuery = useDeferredValue(query);

  const searchIndex = useMemo(() => {
    const index = new Map<string, string>();
    for (const pkg of scan?.packages ?? []) {
      index.set(pkg.key, searchText(pkg));
    }
    return index;
  }, [scan]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    try {
      setScan(await scanPackages());
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
      setLoading(false);
    }
  }, []);

  // Paint the last scan first — the in-memory cache, or the copy the backend
  // persisted during the previous session — then always refresh in the
  // background so nothing on screen stays stale.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cached = await getCachedScan();
        if (!cancelled && cached) {
          setScan(cached);
          setLoading(false);
        }
      } catch {
        // No usable cache: the refresh below covers it.
      }
      if (!cancelled) await refresh();
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  // Derived rather than stored, so the visible list cannot drift out of sync
  // with the scan or the active filters.
  const packages = useMemo(() => {
    const all = scan?.packages ?? [];
    const needle = deferredQuery.trim().toLowerCase();
    return all.filter((pkg) => {
      if (sourceFilter !== "all" && pkg.source !== sourceFilter) return false;
      if (kindFilter !== "all" && pkg.app_kind !== kindFilter) return false;
      if (viewMode === "updates" && !pkg.has_update) return false;
      if (needle) return (searchIndex.get(pkg.key) ?? "").includes(needle);
      return true;
    });
  }, [scan, searchIndex, deferredQuery, sourceFilter, kindFilter, viewMode]);

  return {
    loading,
    refreshing,
    error,
    packages,
    lastScan: scan,
    query,
    sourceFilter,
    kindFilter,
    viewMode,
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
    setViewMode,
  };
}
