import { useEffect, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import type { OperationPlan, OperationResult } from "../../shared/types/operations";
import { SOURCE_LABELS } from "../../shared/types/package";
import { previewUninstall, applyUninstall } from "../../shared/api/operations";

type Phase = "loading" | "confirm" | "running" | "done" | "error";

interface Props {
  pkg: InstalledPackage;
  onClose: () => void;
  onUninstalled: (pkg: InstalledPackage) => void;
}

export function UninstallDialog({ pkg, onClose, onUninstalled }: Props) {
  const [phase, setPhase] = useState<Phase>("loading");
  const [plan, setPlan] = useState<OperationPlan | null>(null);
  const [result, setResult] = useState<OperationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showLogs, setShowLogs] = useState(false);

  // Build the preview plan when the dialog opens.
  useEffect(() => {
    let cancelled = false;
    setPhase("loading");
    setError(null);
    previewUninstall(pkg.key)
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
  }, [pkg.key]);

  async function confirm() {
    if (!plan) return;
    setPhase("running");
    setError(null);
    try {
      const res = await applyUninstall(plan.plan_id);
      setResult(res);
      setPhase("done");
      if (res.success) {
        onUninstalled(pkg);
      }
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }

  const title = plan?.display_name ?? pkg.display_name ?? pkg.name;

  return (
    <div className="modal__overlay" onClick={onClose}>
      <div
        className="modal modal--uninstall"
        role="dialog"
        aria-modal="true"
        aria-label={`Uninstall ${title}`}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="modal__head">
          <h2>Uninstall {title}</h2>
          <button type="button" className="modal__close" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </header>

        {phase === "loading" && (
          <div className="modal__body">
            <p className="modal__muted">Preparing uninstall preview…</p>
          </div>
        )}

        {phase === "error" && (
          <div className="modal__body">
            <div className="banner banner--error">
              {error ?? "Could not prepare the uninstall plan."}
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
                {plan.protection_reason ?? "This package is protected and cannot be removed."}
              </div>
            ) : (
              <>
                <p className="modal__lead">
                  You are about to remove <strong>{title}</strong> ({SOURCE_LABELS[plan.source]}).
                  {plan.requires_auth && (
                    <> Linux will ask for your password to confirm.</>
                  )}
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
                  <div className="plan__row">
                    <dt>Version</dt>
                    <dd>{plan.current_version || "—"}</dd>
                  </div>
                  <div className="plan__row">
                    <dt>Privilege</dt>
                    <dd>{plan.requires_auth ? "Administrator password (Polkit)" : "No password needed"}</dd>
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
                  ⚠ This removes the package from your system. AppImages go to Trash; everything else is removed by its package manager.
                </p>
              </>
            )}
            <div className="modal__actions">
              <button type="button" className="btn" onClick={onClose}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn--danger"
                onClick={confirm}
                disabled={plan.protected}
              >
                {plan.protected ? "Protected" : "Confirm uninstall"}
              </button>
            </div>
          </div>
        )}

        {phase === "running" && (
          <div className="modal__body">
            <p className="modal__muted">
              Removing {title}…{" "}
              {plan?.requires_auth && "If a password dialog appears, enter your administrator password."}
            </p>
            <div className="spinner" aria-hidden />
          </div>
        )}

        {phase === "done" && result && (
          <div className="modal__body">
            <div className={`banner ${result.success ? "banner--ok" : "banner--error"}`}>
              {result.message}
            </div>
            <button
              type="button"
              className="modal__logtoggle"
              onClick={() => setShowLogs((v) => !v)}
            >
              {showLogs ? "Hide" : "Show"} command output
            </button>
            {showLogs && <pre className="modal__logs">{result.logs}</pre>}
            <div className="modal__actions">
              <button type="button" className="btn" onClick={onClose}>
                {result.success ? "Done" : "Close"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
