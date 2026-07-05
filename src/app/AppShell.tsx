import { PackageScreen } from "../features/packages/PackageScreen";
import { AppUpdateNotification } from "../features/updater/UpdateNotification";

export function AppShell() {
  return (
    <>
      <PackageScreen />
      <AppUpdateNotification />
    </>
  );
}