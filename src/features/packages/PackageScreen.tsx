import { useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { PackageList } from "./PackageList";
import { PackageDetail } from "./PackageDetail";
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
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
  } = usePackages();
  const [selected, setSelected] = useState<InstalledPackage | null>(null);

  // Keep the selected detail row in sync after a rescan.
  const selectedRow =
    selected && packages.find((p) => p.key === selected.key)
      ? packages.find((p) => p.key === selected.key)!
      : selected;

  // After a successful uninstall: drop the selection and rescan so the list
  // reflects the new system state.
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
          <span className="topbar__mark" aria-hidden>
            ◉
          </span>
          <h1>Scope</h1>
        </div>
        <div className="topbar__actions">
          {renderAvailability(lastScan?.availability)}
          <button
            type="button"
            className="btn btn--ghost"
            onClick={refresh}
            disabled={refreshing}
          >
            {refreshing ? "Scanning…" : "Rescan"}
          </button>
        </div>
      </header>

      <PackageFilters
        query={query}
        source={sourceFilter}
        kind={kindFilter}
        count={packages.length}
        total={lastScan?.packages.length ?? 0}
        onQuery={setQuery}
        onSource={setSourceFilter}
        onKind={setKindFilter}
      />

      {error && <div className="banner banner--error">{error}</div>}
      {lastScan?.availability?.apt_error && !error && (
        <div className="banner banner--warn">
          APT: {lastScan.availability.apt_error}
        </div>
      )}

      <div className="screen__body">
        <div className="screen__list">
          {loading ? (
            <div className="pkg-list pkg-list--loading">
              Scanning installed apps across APT, Snap, Flatpak, and AppImage…
            </div>
          ) : (
            <PackageList
              packages={packages}
              selectedKey={selectedRow?.key ?? null}
              onSelect={setSelected}
            />
          )}
        </div>
        <PackageDetail pkg={selectedRow} onUninstalled={handleUninstalled} />
      </div>
    </section>
  );
}

function renderAvailability(av?: {
  apt: boolean;
  snap: boolean;
  flatpak: boolean;
  appimage: boolean;
}) {
  if (!av) return null;
  const dots: { label: string; ok: boolean }[] = [
    { label: "APT", ok: av.apt },
    { label: "Snap", ok: av.snap },
    { label: "Flatpak", ok: av.flatpak },
    { label: "AppImage", ok: av.appimage },
  ];
  return (
    <span className="dots" title="Detected package sources">
      {dots.map((d) => (
        <span key={d.label} className={`dot${d.ok ? " dot--ok" : ""}`}>
          {d.label}
        </span>
      ))}
    </span>
  );
}