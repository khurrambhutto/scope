import { useEffect, useMemo, useState } from "react";

import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_COLORS } from "../../shared/types/package";

/**
 * Render a package's app icon when one is available, with a graceful
 * colored-initials fallback.
 *
 * The `icon` field on `InstalledPackage` is either a `scope-icon://localhost/…`
 * URL produced by the backend's icon resolver, or `undefined` for non-GUI
 * packages / unresolved icons. We never touch the filesystem from the
 * webview — the backend serves the bytes through the registered
 * `scope-icon://` URI-scheme protocol.
 */
export function AppIcon({
  pkg,
  title,
  size,
}: {
  pkg: InstalledPackage;
  title: string;
  size: "row" | "detail";
}) {
  const [failed, setFailed] = useState(false);
  const url = useMemo(() => (pkg.icon ? pkg.icon : null), [pkg.icon]);

  useEffect(() => {
    setFailed(false);
  }, [url]);

  const color = SOURCE_COLORS[pkg.source];

  if (url && !failed) {
    return (
      <span className={`app-icon app-icon--${size}`} aria-hidden="true">
        <img
          className="app-icon__img"
          src={url}
          alt=""
          onError={() => setFailed(true)}
          draggable={false}
        />
      </span>
    );
  }

  return (
    <span
      className={`app-icon app-icon--${size} app-icon--fallback`}
      style={{ background: color }}
      aria-hidden="true"
    >
      {initials(title)}
    </span>
  );
}

function initials(name: string): string {
  const parts = name.trim().split(/[\s_\-]+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[1][0]).toUpperCase();
}
