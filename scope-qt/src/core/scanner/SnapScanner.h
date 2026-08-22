#pragma once

#include "scanner/Scanner.h"

namespace scope {

class SnapScanner final : public IScanner
{
public:
    [[nodiscard]] PackageSource source() const override { return PackageSource::Snap; }
    [[nodiscard]] bool isAvailable() const override;
    [[nodiscard]] ScanOutcome scan() const override;

    void checkUpdates(QList<InstalledPackage>& packages) const;

private:
    [[nodiscard]] static bool isRuntime(const QString& name);
    [[nodiscard]] static quint64 installedSize(const QString& snapName);
};

} // namespace scope
