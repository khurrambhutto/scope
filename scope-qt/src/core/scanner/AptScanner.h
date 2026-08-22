#pragma once

#include "scanner/Scanner.h"

namespace scope {

class AptScanner final : public IScanner
{
public:
    [[nodiscard]] PackageSource source() const override { return PackageSource::Apt; }
    [[nodiscard]] bool isAvailable() const override;
    [[nodiscard]] ScanOutcome scan() const override;

    // Update availability; mutates matching packages in place.
    void checkUpdates(QList<InstalledPackage>& packages) const;

private:
    [[nodiscard]] static AppKind classify(const QString& name);
};

} // namespace scope
