import { useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { Logo } from "../../shared/components/Logo";
import { PackageList } from "./PackageList";
import { PackageFilters } from "./PackageFilters";
import { usePackages } from "./usePackages";

export function PackageScreen() {
  const {
    loading,
    refreshing,
    error,
    packages,
    lastScan,
    query,
    sourceFilter,
    kindFilter,
    viewMode,
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
    setViewMode,
  } = usePackages();
  const [selected, setSelected] = useState<InstalledPackage | null>(null);

  const handleSelect = (pkg: InstalledPackage) =>
    setSelected((prev) => (prev?.key === pkg.key ? null : pkg));

  // Keep the selected detail row in sync after a rescan.
  const selectedRow =
    selected && packages.find((p) => p.key === selected.key)
      ? packages.find((p) => p.key === selected.key)!
      : selected;

  const handleUninstalled = (pkg: InstalledPackage) => {
    if (selected?.key === pkg.key) {
      setSelected(null);
    }
    refresh();
  };

  return (
    <section className="screen">
      <header className="topbar">
        <div className="topbar__brand">
          <Logo size={28} className="topbar__logo" ariaLabel="Scope" />
          <h1>Scope</h1>
        </div>
      </header>

      <PackageFilters
        query={query}
        source={sourceFilter}
        kind={kindFilter}
        viewMode={viewMode}
        refreshing={refreshing}
        onQuery={setQuery}
        onSource={setSourceFilter}
        onKind={setKindFilter}
        onViewMode={setViewMode}
        onRescan={refresh}
      />

      {error && <div className="banner banner--error">{error}</div>}
      {lastScan?.availability?.apt_error && !error && (
        <div className="banner banner--warn">
          APT: {lastScan.availability.apt_error}
        </div>
      )}

      <div className="screen__body">
        {loading ? (
          <div className="pkg-list pkg-list--loading">
            Scanning installed apps across APT, Snap, Flatpak, and AppImage…
          </div>
        ) : (
          <PackageList
            packages={packages}
            selectedKey={selectedRow?.key ?? null}
            selectedPkg={selectedRow}
            viewMode={viewMode}
            onSelect={handleSelect}
            onUninstalled={handleUninstalled}
          />
        )}
      </div>

      <footer className="footer" />
    </section>
  );
}
