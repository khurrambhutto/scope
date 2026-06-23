import { useCallback, useEffect, useState } from "react";
import {
  scanPackages,
  getCachedScan,
  searchPackages,
} from "../../shared/api/packages";
import type {
  CachedScan,
  InstalledPackage,
  PackageSource,
  AppKind,
} from "../../shared/types/package";

export type SourceFilter = PackageSource | "all";
export type KindFilter = AppKind | "all";

interface UsePackagesState {
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  packages: InstalledPackage[];
  lastScan: CachedScan | null;
  query: string;
  sourceFilter: SourceFilter;
  kindFilter: KindFilter;
}

export function usePackages() {
  const [state, setState] = useState<UsePackagesState>({
    loading: true,
    refreshing: false,
    error: null,
    packages: [],
    lastScan: null,
    query: "",
    sourceFilter: "all",
    kindFilter: "all",
  });

  // Filter happens client-side on the cached full scan (kept fast & offline).
  const applyFilters = useCallback(
    (scan: CachedScan | null, query: string, source: SourceFilter, kind: KindFilter) => {
      if (!scan) {
        setState((s) => ({ ...s, packages: [] }));
        return;
      }
      const q = query.trim().toLowerCase();
      const filtered = scan.packages.filter((p) => {
        if (source !== "all" && p.source !== source) return false;
        if (kind !== "all" && p.app_kind !== kind) return false;
        if (q) {
          const haystack = [
            p.name,
            p.display_name ?? "",
            p.description ?? "",
            p.package_id,
            p.categories ?? "",
            p.version,
          ]
            .join(" ")
            .toLowerCase();
          if (!haystack.includes(q)) return false;
        }
        return true;
      });
      setState((s) => ({ ...s, packages: filtered }));
    },
    []
  );

  const refresh = useCallback(async () => {
    setState((s) => ({ ...s, refreshing: true, error: null }));
    try {
      const cached = await scanPackages();
      setState((s) => {
        const next = { ...s, loading: false, refreshing: false, lastScan: cached };
        return next;
      });
      applyFilters(cached, state.query, state.sourceFilter, state.kindFilter);
    } catch (e) {
      setState((s) => ({
        ...s,
        loading: false,
        refreshing: false,
        error: String(e),
      }));
    }
  }, [applyFilters, state.query, state.sourceFilter, state.kindFilter]);

  // Initial load: reuse a cached scan if present, else scan fresh.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cached = await getCachedScan();
        if (cancelled) return;
        if (cached) {
          setState((s) => ({ ...s, loading: false, lastScan: cached }));
          applyFilters(cached, "", "all", "all");
        } else {
          await refresh();
        }
      } catch (e) {
        if (!cancelled) {
          setState((s) => ({ ...s, loading: false, error: String(e) }));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const setQuery = useCallback(
    (q: string) => {
      setState((s) => ({ ...s, query: q }));
      applyFilters(state.lastScan, q, state.sourceFilter, state.kindFilter);
    },
    [applyFilters, state.lastScan, state.sourceFilter, state.kindFilter]
  );

  const setSourceFilter = useCallback(
    (src: SourceFilter) => {
      setState((s) => ({ ...s, sourceFilter: src }));
      applyFilters(state.lastScan, state.query, src, state.kindFilter);
    },
    [applyFilters, state.lastScan, state.query, state.kindFilter]
  );

  const setKindFilter = useCallback(
    (k: KindFilter) => {
      setState((s) => ({ ...s, kindFilter: k }));
      applyFilters(state.lastScan, state.query, state.sourceFilter, k);
    },
    [applyFilters, state.lastScan, state.query, state.sourceFilter]
  );

  // Also expose the server-side search for parity; not used by the default UI
  // flow but available for future "search anywhere" affordances.
  const serverSearch = useCallback(
    async (q: string, src: PackageSource | undefined, kind: AppKind | undefined) => {
      return searchPackages(q || undefined, src, kind);
    },
    []
  );

  return {
    ...state,
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
    serverSearch,
  };
}