#pragma once

#include "operations/OperationTypes.h"
#include "package/Package.h"

namespace scope {

namespace update {

[[nodiscard]] OperationPlan preview(const InstalledPackage& pkg);

// Returns an error message when stale, empty string when valid.
[[nodiscard]] QString revalidate(const OperationPlan& plan, const CachedScan& freshScan);

[[nodiscard]] OperationResult apply(const OperationPlan& plan);

} // namespace update

} // namespace scope
