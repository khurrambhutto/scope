import { useAppUpdater, RELEASES_URL } from "./useAppUpdater";

export function AppUpdateNotification() {
  const {
    status,
    update,
    installKind,
    canSelfUpdate,
    error,
    downloaded,
    total,
    download,
    restart,
    dismiss,
  } = useAppUpdater();

  if (status === "idle" || status === "checking" || status === "no-update") {
    return null;
  }

  const progressPct =
    status === "downloading" && total && total > 0
      ? Math.round((downloaded / total) * 100)
      : null;

  return (
    <div className="updater__overlay">
      <div className="updater__banner">
        <div className="updater__body">
          {status === "available" && update && (
            <>
              <div className="updater__content">
                <p className="updater__title">
                  A new version of Scope is available
                </p>
                <p className="updater__version">
                  {update.version}
                  {update.date && (
                    <span className="updater__date">
                      {" "}
                      &middot; {update.date}
                    </span>
                  )}
                </p>
                {update.body && (
                  <p className="updater__notes">{update.body}</p>
                )}
                {installKind === "deb" || installKind === "rpm" ? (
                  <p className="updater__muted">
                    Your Linux desktop will ask for your administrator
                    password to install the update.
                  </p>
                ) : (
                  !canSelfUpdate && (
                    <p className="updater__muted">
                      This install can't update itself automatically. Download
                      the new version here:
                      <br />
                      {RELEASES_URL}
                    </p>
                  )
                )}
              </div>
              <div className="updater__actions">
                <button type="button" className="btn" onClick={dismiss}>
                  Later
                </button>
                {canSelfUpdate && (
                  <button
                    type="button"
                    className="btn btn--update"
                    onClick={download}
                  >
                    Update
                  </button>
                )}
              </div>
            </>
          )}

          {status === "downloading" && (
            <div className="updater__content">
              <p className="updater__title">Downloading update…</p>
              {progressPct !== null && (
                <div className="updater__progress">
                  <div
                    className="updater__progress-bar"
                    style={{ width: `${progressPct}%` }}
                  />
                </div>
              )}
              <p className="updater__muted">
                {progressPct !== null ? `${progressPct}%` : "Preparing…"}
              </p>
              {(installKind === "deb" || installKind === "rpm") && (
                <p className="updater__muted">
                  If a password dialog appears, enter your administrator
                  password.
                </p>
              )}
            </div>
          )}

          {status === "ready" && (
            <>
              <div className="updater__content">
                <p className="updater__title">Update ready</p>
                <p className="updater__muted">
                  Scope will restart to apply the update.
                </p>
              </div>
              <div className="updater__actions">
                <button
                  type="button"
                  className="btn btn--update"
                  onClick={restart}
                >
                  Restart Now
                </button>
              </div>
            </>
          )}

          {status === "error" && (
            <>
              <div className="updater__content">
                <p className="updater__title">Update failed</p>
                <p className="updater__muted">{error}</p>
              </div>
              <div className="updater__actions">
                <button type="button" className="btn" onClick={dismiss}>
                  Dismiss
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
