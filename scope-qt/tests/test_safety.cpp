#include "safety/Safety.h"

#include <QDir>
#include <QFile>
#include <QtTest>

using namespace scope;

class TestSafety : public QObject
{
    Q_OBJECT

private slots:
    void aptCriticalPackagesAreProtected()
    {
        const QStringList critical = {
            "ubuntu-desktop", "systemd", "apt", "dpkg", "bash", "coreutils",
            "linux-image-generic", "gnome-shell", "network-manager", "sudo", "snapd",
            "flatpak", "libc6", "libssl3", "polkitd", "pkexec",
        };
        for (const QString& name : critical) {
            const Protection p = checkPackage(PackageSource::Apt, name);
            QVERIFY2(p.isProtected, qPrintable(QStringLiteral("expected protected: %1").arg(name)));
            QVERIFY(!p.reason.isEmpty());
        }
    }

    void aptArchSuffixStripped()
    {
        QVERIFY(checkPackage(PackageSource::Apt, "libc6:amd64").isProtected);
        QVERIFY(checkPackage(PackageSource::Apt, "libc6:i386").isProtected);
    }

    void aptKernelPrefixesProtected()
    {
        QVERIFY(checkPackage(PackageSource::Apt, "linux-image-6.8.0-40-generic").isProtected);
        QVERIFY(checkPackage(PackageSource::Apt, "linux-headers-6.8.0-40").isProtected);
        QVERIFY(checkPackage(PackageSource::Apt, "linux-modules-extra-6.8.0-40").isProtected);
    }

    void aptLibraryRule()
    {
        QVERIFY(checkPackage(PackageSource::Apt, "libfoo2").isProtected);       // digit
        QVERIFY(checkPackage(PackageSource::Apt, "libglib-1.2-0").isProtected); // dash
        // Exceptions and non-library names are allowed.
        QVERIFY(!checkPackage(PackageSource::Apt, "libreoffice-core").isProtected);
        QVERIFY(!checkPackage(PackageSource::Apt, "libreoffice-writer").isProtected);
        QVERIFY(!checkPackage(PackageSource::Apt, "libre2-dev").isProtected);
        QVERIFY(!checkPackage(PackageSource::Apt, "libfoo").isProtected);  // no digit/dash
        QVERIFY(!checkPackage(PackageSource::Apt, "firefox").isProtected);
    }

    void snapRuntimeBaseProtected()
    {
        for (const QString& name : { "snapd", "bare", "core", "core18", "core20",
                                     "core22", "core24", "gtk-common-themes",
                                     "gnome-42-2204", "gnome-46-2404" }) {
            QVERIFY2(checkPackage(PackageSource::Snap, name).isProtected,
                     qPrintable(name));
        }
        QVERIFY(!checkPackage(PackageSource::Snap, "firefox").isProtected);
        QVERIFY(checkPackage(PackageSource::Snap, "some-app-gtk3").isProtected);
    }

    void flatpakAlwaysAllowed()
    {
        QVERIFY(!checkPackage(PackageSource::Flatpak,
                              "org.mozilla.firefox").isProtected);
    }

    void pathGuardAllowsHomeAppImage()
    {
        const QString dir = QDir::homePath() + "/Downloads/scope-test";
        QDir().mkpath(dir);
        const QString file = dir + "/My.AppImage";
        QFile f(file);
        QVERIFY(f.open(QIODevice::WriteOnly));
        f.write("not-a-real-appimage-but-path-rules-only");
        f.close();

        const Protection p = checkPackage(PackageSource::AppImage, file);
        QVERIFY2(!p.isProtected, qPrintable(p.reason));

        QFile::remove(file);
        QDir(dir).rmdir(dir);
    }

    void pathGuardRejectsOutsidePaths()
    {
        QVERIFY(checkPackage(PackageSource::AppImage, "/etc/passwd").isProtected);
        QVERIFY(checkPackage(PackageSource::AppImage, "/usr/bin/whatever.AppImage").isProtected);
        QVERIFY(checkPackage(PackageSource::Manual, "/opt").isProtected);
    }

    void pathGuardRejectsWholeRootsForManual()
    {
        // Removing an entire allowed root must be denied.
        Protection p = checkPathKind(QDir::homePath() + "/.local/bin", PathKind::Manual);
        QVERIFY(p.isProtected);

        // But a .app bundle under ~/.local is fine.
        const QString bundle = QDir::homePath() + "/.local/share/scope-test.app";
        QDir().mkpath(bundle);
        p = checkPathKind(bundle, PathKind::Manual);
        QVERIFY2(!p.isProtected, qPrintable(p.reason));
        QDir(bundle).rmdir(bundle);
    }

    void desktopEntriesOnlyFromUserApplications()
    {
        // System desktop files must never be removable.
        QVERIFY(checkPathKind("/usr/share/applications/firefox.desktop",
                              PathKind::DesktopEntry).isProtected);
    }
};

QTEST_MAIN(TestSafety)
#include "test_safety.moc"
