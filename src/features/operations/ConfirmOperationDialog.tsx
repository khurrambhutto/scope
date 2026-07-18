// Shared preview → confirm dialog for uninstall and update.
//
// The dialog only covers the preview-first step. On confirm it hands the
// backend-issued plan to the operation task store and closes immediately, so
// the app stays usable while the operation runs in the background.

import { useEffect, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_LABELS } from "../../shared/types/package";
import type { Operation, OperationPlan } from "../../shared/types/operations";
import { previewUninstall, previewUpdate } from "../../shared/api/operations";
import { useOperationTasks } from "./tasks";
import { XIcon } from "../../shared/components/icons";

type Phase = "loading" | "confirm" | "error";

interface Props {
  pkg: InstalledPackage;
  kind: Operation;
  onClose: () => void;
}

export function ConfirmOperationDialog({ pkg, kind, onClose }: Props) {
  const { start } = useOperationTasks();
  const [phase, setPhase] = useState<Phase>("loading");
  const [plan, setPlan] = useState<OperationPlan | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Build the preview plan when the dialog opens.
  useEffect(() => {
    let cancelled = false;
    const preview = kind === "uninstall" ? previewUninstall : previewUpdate;
    preview(pkg.key)
      .then((p) => {
        if (cancelled) return;
        setPlan(p);
        setPhase("confirm");
      })
      .catch((e) => {
        if (cancelled) return;
        setError(String(e));
        setPhase("error");
      });
    return () => {
      cancelled = true;
    };
  }, [pkg.key, kind]);

  function confirm() {
    if (!plan || plan.protected) return;
    start({ kind, planId: plan.plan_id, pkgKey: pkg.key, title });
    onClose();
  }

  const title = plan?.display_name ?? pkg.display_name ?? pkg.name;
  const heading = kind === "uninstall" ? `Uninstall ${title}` : `Update ${title}`;

  return (
    <div className="modal__overlay" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={heading}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="modal__head">
          <h2>{heading}</h2>
          <button type="button" className="modal__close" onClick={onClose} aria-label="Close">
            <XIcon size={16} />
          </button>
        </header>

        {phase === "loading" && (
          <div className="modal__body modal__body--loading">
            <div className="spinner" aria-hidden />
            <p className="modal__muted">
              {kind === "uninstall"
                ? "Preparing uninstall preview…"
                : "Preparing update preview…"}
            </p>
          </div>
        )}

        {phase === "error" && (
          <div className="modal__body">
            <div className="banner banner--error">
              {error ?? "Could not prepare the operation plan."}
            </div>
            <div className="modal__actions">
              <button type="button" className="btn" onClick={onClose}>
                Close
              </button>
            </div>
          </div>
        )}

        {phase === "confirm" && plan && (
          <div className="modal__body">
            {plan.protected ? (
              <div className="banner banner--warn">
                {plan.protection_reason ??
                  `This package is protected and cannot be ${
                    kind === "uninstall" ? "removed" : "updated"
                  }.`}
              </div>
            ) : (
              <>
                <p className="modal__lead">
                  {kind === "uninstall" ? (
                    <>
                      You are about to remove <strong>{title}</strong> (
                      {SOURCE_LABELS[plan.source]}).
                    </>
                  ) : (
                    <>
                      Update <strong>{title}</strong> from{" "}
                      <strong>{plan.current_version || "current"}</strong> to{" "}
                      <strong>{plan.target_version || "latest"}</strong> (
                      {SOURCE_LABELS[plan.source]}).
                    </>
                  )}
                  {plan.requires_auth && <> Linux will ask for your password to confirm.</>}
                </p>
                <dl className="plan">
                  <div className="plan__row">
                    <dt>Package</dt>
                    <dd>{plan.package_id}</dd>
                  </div>
                  {plan.install_scope && (
                    <div className="plan__row">
                      <dt>Scope</dt>
                      <dd>{plan.install_scope}</dd>
                    </div>
                  )}
                  {kind === "update" ? (
                    <>
                      <div className="plan__row">
                        <dt>Current version</dt>
                        <dd>{plan.current_version || "—"}</dd>
                      </div>
                      <div className="plan__row">
                        <dt>Target version</dt>
                        <dd>{plan.target_version || "latest"}</dd>
                      </div>
                    </>
                  ) : (
                    <div className="plan__row">
                      <dt>Version</dt>
                      <dd>{plan.current_version || "—"}</dd>
                    </div>
                  )}
                  <div className="plan__row">
                    <dt>Privilege</dt>
                    <dd>
                      {plan.requires_auth
                        ? "Administrator password (Polkit)"
                        : "No password needed"}
                    </dd>
                  </div>
                </dl>
                <ul className="plan__steps">
                  {plan.steps.map((s, i) => (
                    <li key={i}>
                      <span className="plan__step-desc">{s.description}</span>
                      <code className="plan__step-cmd">{s.command_summary}</code>
                    </li>
                  ))}
                </ul>
                <p className="modal__warn">
                  {kind === "uninstall"
                    ? "This removes the package from your system. AppImages go to Trash; everything else is removed by its package manager."
                    : "The operation runs in the background — you can keep using Scope while it finishes."}
                </p>
              </>
            )}
            <div className="modal__actions">
              <button type="button" className="btn" onClick={onClose}>
                Cancel
              </button>
              <button
                type="button"
                className={`btn ${kind === "uninstall" ? "btn--danger" : "btn--primary"}`}
                onClick={confirm}
                disabled={plan.protected}
              >
                {plan.protected
                  ? "Protected"
                  : kind === "uninstall"
                    ? "Confirm uninstall"
                    : "Confirm update"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
