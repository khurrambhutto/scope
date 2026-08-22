#pragma once

#include "package/Package.h"

namespace scope {

struct ScanOutcome {
    PackageSource source = PackageSource::Apt;
    bool available = false;
    QList<InstalledPackage> packages;
    QString error;
};

class IScanner
{
public:
    virtual ~IScanner() = default;

    [[nodiscard]] virtual PackageSource source() const = 0;
    [[nodiscard]] virtual bool isAvailable() const = 0;
    // Blocking scan; callers are expected to run it on a worker thread.
    [[nodiscard]] virtual ScanOutcome scan() const = 0;
};

} // namespace scope
