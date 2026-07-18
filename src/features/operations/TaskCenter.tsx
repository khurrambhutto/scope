// Floating stack of background operation cards (bottom-right).
//
// While an uninstall/update runs, the app stays fully usable; each task shows
// live status here. Successes auto-dismiss, failures stay with expandable
// logs until dismissed.

import { useState } from "react";
import { useOperationTasks, type OperationTask } from "./tasks";
import { AlertIcon, CheckCircleIcon, XIcon } from "../../shared/components/icons";

function TaskCard({ task }: { task: OperationTask }) {
  const { dismiss } = useOperationTasks();
  const [showLogs, setShowLogs] = useState(false);

  const verb = task.kind === "uninstall" ? "Removing" : "Updating";
  const doneVerb = task.kind === "uninstall" ? "removed" : "updated";

  return (
    <div className={`task task--${task.status}`}>
      <div className="task__icon" aria-hidden>
        {task.status === "running" ? (
          <span className="spinner spinner--sm" />
        ) : task.status === "success" ? (
          <CheckCircleIcon size={18} className="task__icon-ok" />
        ) : (
          <AlertIcon size={18} className="task__icon-err" />
        )}
      </div>
      <div className="task__main">
        <p className="task__title">
          {task.status === "running"
            ? `${verb} ${task.title}…`
            : task.status === "success"
              ? `${task.title} ${doneVerb}`
              : `Failed to ${task.kind === "uninstall" ? "remove" : "update"} ${task.title}`}
        </p>
        <p className="task__sub">
          {task.status === "running"
            ? "You can keep using Scope."
            : (task.message ?? "")}
        </p>
        {task.status === "error" && task.logs && (
          <>
            <button
              type="button"
              className="task__logtoggle"
              onClick={() => setShowLogs((v) => !v)}
            >
              {showLogs ? "Hide" : "Show"} command output
            </button>
            {showLogs && <pre className="task__logs">{task.logs}</pre>}
          </>
        )}
      </div>
      {task.status !== "running" && (
        <button
          type="button"
          className="task__dismiss"
          onClick={() => dismiss(task.id)}
          aria-label="Dismiss"
        >
          <XIcon size={14} />
        </button>
      )}
    </div>
  );
}

export function TaskCenter() {
  const { tasks } = useOperationTasks();
  if (tasks.length === 0) return null;
  return (
    <div className="task-center" aria-live="polite">
      {tasks.map((t) => (
        <TaskCard key={t.id} task={t} />
      ))}
    </div>
  );
}
