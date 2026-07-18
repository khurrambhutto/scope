// App sidebar: brand, primary navigation, and About pinned to the bottom.
//
// Only "Apps" is live today. Clean / Analyze / Status are future phases
// (see AGENTS.md) and render disabled with a "Soon" chip.

import type { JSX } from "react";
import { Logo } from "../shared/components/Logo";
import {
  ActivityIcon,
  ChartPieIcon,
  GridIcon,
  InfoIcon,
  SparklesIcon,
} from "../shared/components/icons";

interface NavItem {
  id: string;
  label: string;
  icon: (props: { size?: number; className?: string }) => JSX.Element;
  active?: boolean;
  soon?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { id: "apps", label: "Apps", icon: GridIcon, active: true },
  { id: "clean", label: "Clean", icon: SparklesIcon, soon: true },
  { id: "analyze", label: "Analyze", icon: ChartPieIcon, soon: true },
  { id: "status", label: "Status", icon: ActivityIcon, soon: true },
];

export function Sidebar({ onAbout }: { onAbout: () => void }) {
  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <Logo size={24} className="sidebar__logo" ariaLabel="Scope" />
        <span className="sidebar__name">Scope</span>
      </div>

      <nav className="sidebar__nav" aria-label="Primary">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`sidebar__item${item.active ? " sidebar__item--active" : ""}`}
            disabled={item.soon}
            title={item.soon ? `${item.label} — coming in a future phase` : item.label}
          >
            <item.icon size={17} className="sidebar__item-icon" />
            <span className="sidebar__item-label">{item.label}</span>
            {item.soon && <span className="sidebar__soon">Soon</span>}
          </button>
        ))}
      </nav>

      <div className="sidebar__footer">
        <button type="button" className="sidebar__item" onClick={onAbout}>
          <InfoIcon size={17} className="sidebar__item-icon" />
          <span className="sidebar__item-label">About</span>
        </button>
      </div>
    </aside>
  );
}
