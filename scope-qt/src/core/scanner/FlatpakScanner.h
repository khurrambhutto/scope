#pragma once

#include "scanner/Scanner.h"

namespace scope {

// Exposed for unit tests: parses flatpak's human size strings with binary
// multipliers ("1,5 GB" -> 1610612736).
namespace flatpak_detail {
[[nodiscard]] quint64 parseSize(QString raw);
}

class FlatpakScanner final : public IScanner
{
public:
    [[nodiscard]] PackageSource source() const override { return PackageSource::Flatpak; }
    [[nodiscard]] bool isAvailable() const override;
    [[nodiscard]] ScanOutcome scan() const override;

    void checkUpdates(QList<InstalledPackage>& packages) const;

private:
    [[nodiscard]] static ScanOutcome scanScope(InstallScope scope);
    static void checkScopeUpdates(InstallScope scope, QList<InstalledPackage>& packages);
};

} // namespace scope
