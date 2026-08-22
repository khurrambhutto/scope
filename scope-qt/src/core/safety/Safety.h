#pragma once

#include <QString>

#include "package/Package.h"

namespace scope {

struct Protection {
    bool isProtected = false;
    QString reason;
};

// Package-level guard used by preview and revalidation for every source.
[[nodiscard]] Protection checkPackage(PackageSource source, const QString& packageId);

// Path-level guards used for AppImage and Manual (loose-file) targets.
enum class PathKind {
    AppImage,
    Manual,
    DesktopEntry,
};

[[nodiscard]] Protection checkPathKind(const QString& path, PathKind kind);
[[nodiscard]] Protection checkManual(const QString& packedId);

} // namespace scope
