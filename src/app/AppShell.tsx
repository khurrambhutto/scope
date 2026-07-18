import { useState } from "react";
import { Sidebar } from "./Sidebar";
import { AboutDialog } from "./AboutDialog";
import { PackageScreen } from "../features/packages/PackageScreen";
import { OperationTasksProvider } from "../features/operations/tasks";
import { TaskCenter } from "../features/operations/TaskCenter";
import { AppUpdateNotification } from "../features/updater/UpdateNotification";

export function AppShell() {
  const [aboutOpen, setAboutOpen] = useState(false);

  return (
    <OperationTasksProvider>
      <div className="shell">
        <Sidebar onAbout={() => setAboutOpen(true)} />
        <main className="main">
          <PackageScreen />
        </main>
      </div>
      <TaskCenter />
      <AppUpdateNotification />
      {aboutOpen && <AboutDialog onClose={() => setAboutOpen(false)} />}
    </OperationTasksProvider>
  );
}
