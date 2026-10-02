import { useEffect, useRef, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import type {
  OperationPlan,
  OperationResult,
  OperationStage,
} from "../../shared/types/operations";
import {
  previewUpdate,
  applyUpdate,
  onOperationStatus,
  onOperationLog,
  isCancelledResult,
} from "../../shared/api/operations";
import { formatSize, formatElapsed } from "../packages/format";

type Phase = "loading" | "confirm" | "running" | "done" | "error";

interface Props {
  pkg: InstalledPackage;
  onClose: () => void;
  onUpdated: (pkg: InstalledPackage) => void;
}

export function UpdateDialog({ pkg, onClose, onUpdated }: Props) {
  const [phase, setPhase] = useState<Phase>("loading");
  const [plan, setPlan] = useState<OperationPlan | null>(null);
  const [result, setResult] = useState<OperationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showLogs, setShowLogs] = useState(false);
  const [stage, setStage] = useState<OperationStage>("verifying");
  const [lines, setLines] = useState<string[]>([]);
  const [elapsed, setElapsed] = useState(0);
  const logRef = useRef<HTMLPreElement | null>(null);

  // Build the preview plan when the dialog opens.
  useEffect(() => {
    let cancelled = false;
    setPhase("loading");
    setError(null);
    previewUpdate(pkg.key)
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

  // Auto-scroll the live log to the newest line. The panel is display-only:
  // `overflow: hidden` in CSS means the user cannot scroll it themselves.
  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  async function confirm() {
    if (!plan) return;
    setPhase("running");
    setStage("verifying");
    setError(null);
    setLines([]);
    setElapsed(0);
    const startedAt = Date.now();
    const timer = window.setInterval(
      () => setElapsed(Math.floor((Date.now() - startedAt) / 1000)),
      1000,
    );
    let unlistenStatus: (() => void) | undefined;
    let unlistenLog: (() => void) | undefined;
    try {
      try {
        unlistenStatus = await onOperationStatus((status) => {
          if (status.plan_id === plan.plan_id) {
            setStage(status.stage);
          }
        });
        unlistenLog = await onOperationLog((log) => {
          if (log.plan_id === plan.plan_id) {
            setLines((prev) => [...prev, log.line]);
          }
        });
      } catch {
        // Progress events are best-effort; fall back to the default copy.
      }
      const res = await applyUpdate(plan.plan_id);
      setResult(res);
      setPhase("done");
      if (res.success) {
        onUpdated(pkg);
      }
    } catch (e) {
      setError(String(e));
      setPhase("error");
    } finally {
      window.clearInterval(timer);
      unlistenStatus?.();
      unlistenLog?.();
    }
  }

  const title = plan?.display_name ?? pkg.display_name ?? pkg.name;

  return (
    <div className="modal__overlay" onClick={onClose}>
      <div
        className="modal modal--update"
        role="dialog"
        aria-modal="true"
        aria-label={`Update ${title}`}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="modal__head">
          <h2>Update {title}</h2>
          <button type="button" className="modal__close" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </header>

        {phase === "loading" && (
          <div className="modal__body">
            <p className="modal__muted">Preparing update preview…</p>
          </div>
        )}

        {phase === "error" && (
          <div className="modal__body">
            <div className="banner banner--error">
              {error ?? "Could not prepare the update plan."}
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
                {plan.protection_reason ?? "This package is protected and cannot be updated."}
              </div>
            ) : (
              <>
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
                    <dt>Current version</dt>
                    <dd>{plan.current_version || "—"}</dd>
                  </div>
                  <div className="plan__row">
                    <dt>Target version</dt>
                    <dd>{plan.target_version || "latest"}</dd>
                  </div>
                  <div className="plan__row">
                    <dt>Size</dt>
                    <dd>{formatSize(pkg.size_bytes)}</dd>
                  </div>
                </dl>
              </>
            )}
            <div className="modal__actions">
              <button type="button" className="btn" onClick={onClose}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn--update"
                onClick={confirm}
                disabled={plan.protected}
              >
                {plan.protected ? "Protected" : "Confirm"}
              </button>
            </div>
          </div>
        )}

        {phase === "running" && (
          <div className="modal__body">
            <div className="run-head">
              <div className="spinner" aria-hidden />
              <p className="modal__muted run-head__text">
                {stage === "verifying" ? (
                  <>
                    Checking that {title} can be updated…{" "}
                    {plan?.requires_auth && "A system password prompt will appear next."}
                  </>
                ) : (
                  <>
                    Updating {title}…{" "}
                    {plan?.requires_auth &&
                      "If a password dialog appears, enter your administrator password."}
                  </>
                )}
              </p>
              <span className="run-head__timer">{formatElapsed(elapsed)}</span>
            </div>
            <pre className="modal__live-logs" ref={logRef}>
              {lines.length ? lines.join("\n") : "Waiting for output…"}
            </pre>
          </div>
        )}

        {phase === "done" && result && (
          <div className="modal__body">
            {isCancelledResult(result) ? (
              <div className="banner banner--muted">
                Authentication was cancelled. Nothing was changed.
              </div>
            ) : (
              <>
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
              </>
            )}
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
