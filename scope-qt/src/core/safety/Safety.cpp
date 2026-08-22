#include "Safety.h"

#include <QDir>
#include <QFileInfo>

namespace scope {

namespace {

constexpr char kUnitSeparator = '\x1f';

bool isLibraryName(const QString& remainder)
{
    // Mirrors the Rust rule: contains an ASCII digit or a dash.
    for (const QChar ch : remainder) {
        if (ch.isDigit() || ch == QLatin1Char('-'))
            return true;
    }
    return false;
}

Protection deny(QString reason)
{
    Protection p;
    p.isProtected = true;
    p.reason = std::move(reason);
    return p;
}

Protection allow()
{
    return {};
}

Protection checkApt(const QString& rawName)
{
    QString name = rawName.toLower();
    for (const char* suffix : { ":amd64", ":i386" }) {
        const QString s = QString::fromLatin1(suffix);
        if (name.endsWith(s))
            name.chop(s.size());
    }

    static const QStringList kCritical = {
        QStringLiteral("ubuntu-desktop"), QStringLiteral("ubuntu-standard"),
        QStringLiteral("ubuntu-minimal"), QStringLiteral("ubuntu-release-upgrader-core"),
        QStringLiteral("systemd"), QStringLiteral("systemd-sysv"),
        QStringLiteral("systemd-timesyncd"), QStringLiteral("systemd-resolved"),
        QStringLiteral("systemd-logind"),
        QStringLiteral("polkitd"), QStringLiteral("policykit-1"), QStringLiteral("polkit"),
        QStringLiteral("pkexec"),
        QStringLiteral("apt"), QStringLiteral("apt-utils"), QStringLiteral("dpkg"),
        QStringLiteral("base-files"), QStringLiteral("base-passwd"), QStringLiteral("bash"),
        QStringLiteral("coreutils"),
        QStringLiteral("linux-image-generic"), QStringLiteral("linux-headers-generic"),
        QStringLiteral("linux-generic"),
        QStringLiteral("gnome-shell"), QStringLiteral("gnome-session"),
        QStringLiteral("gnome-control-center"), QStringLiteral("gdm3"), QStringLiteral("gdm"),
        QStringLiteral("xorg"), QStringLiteral("xserver-xorg-core"),
        QStringLiteral("xserver-xorg"), QStringLiteral("wayland"),
        QStringLiteral("network-manager"), QStringLiteral("network-manager-gnome"),
        QStringLiteral("netplan.io"), QStringLiteral("iproute2"),
        QStringLiteral("sudo"), QStringLiteral("login"), QStringLiteral("passwd"),
        QStringLiteral("shadow"), QStringLiteral("adduser"),
        QStringLiteral("snapd"), QStringLiteral("flatpak"),
        QStringLiteral("libc6"), QStringLiteral("libssl3"), QStringLiteral("libgtk-3-0"),
        QStringLiteral("libgtk-4-1"),
    };
    if (kCritical.contains(name)) {
        return deny(QStringLiteral("'%1' is a system-critical package and cannot be removed "
                                   "through Scope.")
                        .arg(name));
    }

    if (name.startsWith(QLatin1String("linux-image-")) ||
        name.startsWith(QLatin1String("linux-headers-")) ||
        name.startsWith(QLatin1String("linux-modules-"))) {
        return deny(QStringLiteral("'%1' is a kernel package and is protected.").arg(name));
    }

    if (name.startsWith(QLatin1String("lib")) && name.size() > 3) {
        const QString remainder = name.mid(3);
        const bool excepted = name.startsWith(QLatin1String("libreoffice")) ||
                              name.startsWith(QLatin1String("libre2"));
        if (!excepted && isLibraryName(remainder)) {
            return deny(QStringLiteral("'%1' is a shared library; Scope removes applications, "
                                       "not libraries.")
                            .arg(name));
        }
    }

    return allow();
}

Protection checkSnap(const QString& rawName)
{
    const QString name = rawName.toLower();

    static const QStringList kRuntimeExact = {
        QStringLiteral("snapd"), QStringLiteral("bare"), QStringLiteral("core"),
        QStringLiteral("core18"), QStringLiteral("core20"), QStringLiteral("core22"),
        QStringLiteral("core24"),
    };
    if (kRuntimeExact.contains(name) || name.startsWith(QLatin1String("gtk-")) ||
        name.startsWith(QLatin1String("gnome-")) || name.endsWith(QLatin1String("-gtk3"))) {
        return deny(QStringLiteral("'%1' is a Snap runtime/base and is protected.").arg(name));
    }
    return allow();
}

QStringList allowedRoots()
{
    const QString home = QDir::homePath();
    QStringList roots;
    roots << QStringLiteral("/opt")
          << home + QStringLiteral("/Applications")
          << home + QStringLiteral("/apps")
          << home + QStringLiteral("/AppImages")
          << home + QStringLiteral("/Downloads")
          << home + QStringLiteral("/.local/bin")
          << home + QStringLiteral("/.local/opt")
          << home + QStringLiteral("/.local/share")
          << home + QStringLiteral("/.local/zed.app")
          << home + QStringLiteral("/.claude")
          << home + QStringLiteral("/.hermes")
          << home + QStringLiteral("/.config");
    return roots;
}

bool isInsideRoot(const QString& canonicalPath, const QString& root)
{
    if (canonicalPath == root)
        return true;
    const QString prefixedRoot = root.endsWith(QLatin1Char('/')) ? root : root + QLatin1Char('/');
    return canonicalPath.startsWith(prefixedRoot);
}

Protection checkPathKindImpl(const QString& rawPath, PathKind kind)
{
    const QFileInfo info(rawPath);
    const QString canonical = info.canonicalFilePath();
    if (canonical.isEmpty()) {
        return deny(QStringLiteral("Path does not resolve to a real file."));
    }

    bool insideAllowed = false;
    for (const QString& root : allowedRoots()) {
        if (isInsideRoot(canonical, root)) {
            insideAllowed = true;
            break;
        }
    }
    const QString home = QDir::homePath();
    const QString localPrefix = home + QStringLiteral("/.local/");
    const bool specialLocalApp =
        canonical.startsWith(localPrefix) && canonical.contains(QLatin1String(".app"));
    if (!insideAllowed && !specialLocalApp) {
        return deny(QStringLiteral("File is outside the allowed install directories."));
    }

    switch (kind) {
    case PathKind::AppImage: {
        if (!canonical.endsWith(QLatin1String(".appimage"), Qt::CaseInsensitive) ||
            !QFileInfo(canonical).isFile()) {
            return deny(QStringLiteral("Refusing to remove: not a valid AppImage file."));
        }
        break;
    }
    case PathKind::DesktopEntry: {
        const QString userApps =
            home + QStringLiteral("/.local/share/applications/");
        if (!canonical.endsWith(QLatin1String(".desktop")) ||
            !canonical.startsWith(userApps) || !QFileInfo(canonical).isFile()) {
            return deny(QStringLiteral("Refusing to remove: not a user desktop entry."));
        }
        break;
    }
    case PathKind::Manual: {
        const QFileInfo fileInfo(canonical);
        if (!fileInfo.isFile() && !fileInfo.isDir())
            return deny(QStringLiteral("Path does not resolve to a real file."));

        bool equalsRoot = false;
        for (const QString& root : allowedRoots()) {
            if (canonical == root) {
                equalsRoot = true;
                break;
            }
        }
        if (equalsRoot && !canonical.endsWith(QLatin1String(".app"))) {
            return deny(QStringLiteral("Refusing to remove an entire system directory."));
        }
        break;
    }
    }

    return allow();
}

} // namespace

Protection checkPackage(PackageSource source, const QString& packageId)
{
    switch (source) {
    case PackageSource::Apt:
        return checkApt(packageId);
    case PackageSource::Snap:
        return checkSnap(packageId);
    case PackageSource::Flatpak:
        return allow();
    case PackageSource::AppImage:
        return checkPathKind(packageId, PathKind::AppImage);
    case PackageSource::Manual:
        return checkManual(packageId);
    }
    return allow();
}

Protection checkPathKind(const QString& path, PathKind kind)
{
    return checkPathKindImpl(path, kind);
}

Protection checkManual(const QString& packedId)
{
    const QStringList parts = packedId.split(kUnitSeparator);
    const QString primary = parts.value(0);
    const QString desktopPath = parts.size() > 1 ? parts.value(1) : QString();

    const Protection primaryCheck = primary.endsWith(QLatin1String(".desktop"))
        ? checkPathKind(primary, PathKind::DesktopEntry)
        : checkPathKind(primary, PathKind::Manual);
    if (primaryCheck.isProtected)
        return primaryCheck;

    if (!desktopPath.isEmpty()) {
        const Protection desktopCheck = checkPathKind(desktopPath, PathKind::DesktopEntry);
        if (desktopCheck.isProtected)
            return desktopCheck;
    }

    return allow();
}

} // namespace scope
