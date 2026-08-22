#pragma once

#include "scanner/Scanner.h"

namespace scope {

class AppImageScanner final : public IScanner
{
public:
    [[nodiscard]] PackageSource source() const override { return PackageSource::AppImage; }
    [[nodiscard]] bool isAvailable() const override { return true; }
    [[nodiscard]] ScanOutcome scan() const override;

    [[nodiscard]] static QStringList searchDirectories();

private:
    [[nodiscard]] static bool looksLikeAppImage(const QString& path);
    [[nodiscard]] static InstalledPackage buildPackage(const QString& path);
};

} // namespace scope
