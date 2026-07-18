// Small About dialog with app identity and version.

import { Logo } from "../shared/components/Logo";
import { XIcon } from "../shared/components/icons";
import pkg from "../../package.json";

export function AboutDialog({ onClose }: { onClose: () => void }) {
  return (
    <div className="modal__overlay" onClick={onClose}>
      <div
        className="modal modal--about"
        role="dialog"
        aria-modal="true"
        aria-label="About Scope"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="modal__head">
          <h2>About</h2>
          <button type="button" className="modal__close" onClick={onClose} aria-label="Close">
            <XIcon size={16} />
          </button>
        </header>
        <div className="about">
          <Logo size={56} className="about__logo" ariaLabel="Scope" />
          <h3 className="about__name">Scope</h3>
          <p className="about__version">Version {pkg.version}</p>
          <p className="about__desc">
            One place to see, update, and uninstall every app on your Linux
            system — APT, Snap, Flatpak, and AppImage.
          </p>
        </div>
      </div>
    </div>
  );
}
