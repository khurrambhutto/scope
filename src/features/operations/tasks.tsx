// Background operation task store.
//
// Uninstalls and updates run here instead of inside a blocking modal. The
// confirm dialog hands off a backend-issued `plan_id` and closes immediately;
// the task runs in the background and is surfaced through the floating
// TaskCenter. Rows can derive their in-progress state from `busyByKey`, and
// the Apps screen refreshes the scan when a task settles.

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { Operation, OperationResult } from "../../shared/types/operations";
import { applyUninstall, applyUpdate } from "../../shared/api/operations";

export type TaskStatus = "running" | "success" | "error";

export interface OperationTask {
  id: string;
  kind: Operation;
  pkgKey: string;
  title: string;
  status: TaskStatus;
  message?: string;
  logs?: string;
  startedAt: number;
}

interface StartArgs {
  kind: Operation;
  planId: string;
  pkgKey: string;
  title: string;
}

interface OperationTasksValue {
  tasks: OperationTask[];
  /** Map of package key → running operation kind, for per-row busy states. */
  busyByKey: Record<string, Operation>;
  start: (args: StartArgs) => void;
  dismiss: (id: string) => void;
}

const OperationTasksContext = createContext<OperationTasksValue | null>(null);

function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function OperationTasksProvider({ children }: { children: ReactNode }) {
  const [tasks, setTasks] = useState<OperationTask[]>([]);
  const busyRef = useRef<Set<string>>(new Set());

  const dismiss = useCallback((id: string) => {
    setTasks((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const patch = useCallback((id: string, fields: Partial<OperationTask>) => {
    setTasks((prev) => prev.map((t) => (t.id === id ? { ...t, ...fields } : t)));
  }, []);

  const start = useCallback(
    ({ kind, planId, pkgKey, title }: StartArgs) => {
      // One operation per package at a time.
      if (busyRef.current.has(pkgKey)) return;
      busyRef.current.add(pkgKey);

      const id = newId();
      setTasks((prev) => [
        ...prev,
        { id, kind, pkgKey, title, status: "running", startedAt: Date.now() },
      ]);

      const run = kind === "uninstall" ? applyUninstall : applyUpdate;
      run(planId)
        .then((res: OperationResult) => {
          busyRef.current.delete(pkgKey);
          patch(id, {
            status: res.success ? "success" : "error",
            message: res.message,
            logs: res.logs,
          });
          if (res.success) {
            window.setTimeout(() => dismiss(id), 6000);
          }
        })
        .catch((e) => {
          busyRef.current.delete(pkgKey);
          patch(id, { status: "error", message: String(e) });
        });
    },
    [dismiss, patch],
  );

  const busyByKey = useMemo(() => {
    const map: Record<string, Operation> = {};
    for (const t of tasks) {
      if (t.status === "running") map[t.pkgKey] = t.kind;
    }
    return map;
  }, [tasks]);

  const value = useMemo(
    () => ({ tasks, busyByKey, start, dismiss }),
    [tasks, busyByKey, start, dismiss],
  );

  return (
    <OperationTasksContext.Provider value={value}>
      {children}
    </OperationTasksContext.Provider>
  );
}

export function useOperationTasks(): OperationTasksValue {
  const ctx = useContext(OperationTasksContext);
  if (!ctx) {
    throw new Error("useOperationTasks must be used within OperationTasksProvider");
  }
  return ctx;
}
