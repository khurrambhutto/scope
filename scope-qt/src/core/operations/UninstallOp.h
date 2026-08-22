#pragma once

#include "operations/OperationTypes.h"
#include "package/Package.h"

namespace scope {

namespace uninstall {

// Builds the preview plan. Protected packages get a plan that is never issued.
[[nodiscard]] OperationPlan preview(const InstalledPackage& pkg);

// Re-checks a taken plan against a fresh scan before anything is executed.
// Returns an error message when stale/protected, empty string when valid.
[[nodiscard]] QString revalidate(const OperationPlan& plan, const CachedScan& freshScan);

[[nodiscard]] OperationResult apply(const OperationPlan& plan);

} // namespace uninstall

} // namespace scope
