#include "UninstallOp.h"

#include "operations/PlanStore.h"
#include "safety/Safety.h"
#include "system/System.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>

namespace scope {
namespace uninstall {

namespace {

constexpr char kUnitSeparator = '\x1f';

struct StepBuild {
    QVector<PlanStep> steps;
    AuthMethod auth = AuthMethod::None;
};

StepBuild buildSteps(const InstalledPackage& pkg, const Protection& protection)
{
    StepBuild build;

    if (protection.isProtected) {
        PlanStep step;
        step.description = QStringLiteral("(no command — protected)");
        step.commandSummary =
            QStringLiteral("Blocked: this package is protected and cannot be removed.");
        build.steps.append(step);
        return build;
    }

    switch (pkg.source) {
    case PackageSource::Apt: {
        PlanStep step;
        step.description = QStringLiteral("Remove APT package '%1'").arg(pkg.packageId);
        step.commandSummary =
            QStringLiteral("pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y %1")
                .arg(pkg.packageId);
        build.steps.append(step);
        build.auth = AuthMethod::Pkexec;
        break;
    }
    case PackageSource::Snap: {
        PlanStep step;
        step.description = QStringLiteral("Remove Snap package '%1'").arg(pkg.packageId);
        step.commandSummary = QStringLiteral("pkexec snap remove %1").arg(pkg.packageId);
        build.steps.append(step);
        build.auth = AuthMethod::Pkexec;
        break;
    }
    case PackageSource::Flatpak: {
        const bool user = pkg.installScope == InstallScope::User;
        PlanStep step;
        step.description = QStringLiteral("Uninstall Flatpak app '%1' (%2)")
                               .arg(pkg.packageId,
                                    user ? QStringLiteral("user install")
                                         : QStringLiteral("system install"));
        step.commandSummary = user
            ? QStringLiteral("flatpak uninstall -y --user %1").arg(pkg.packageId)
            : QStringLiteral("pkexec flatpak uninstall -y --system %1").arg(pkg.packageId);
        build.steps.append(step);
        build.auth = user ? AuthMethod::None : AuthMethod::Pkexec;
        break;
    }
    case PackageSource::AppImage: {
        PlanStep step;
        step.description = QStringLiteral("Move AppImage to Trash");
        step.commandSummary = QStringLiteral("gio trash %1").arg(pkg.packageId);
        build.steps.append(step);
        build.auth = AuthMethod::None;
        break;
    }
    case PackageSource::Manual: {
        const QStringList parts = pkg.packageId.split(kUnitSeparator);
        const QString binaryPath = parts.value(0);
        const QString desktopPath = parts.size() > 1 ? parts.value(1) : QString();

        // Removal targets are computed at preview time for display; the apply
        // path recomputes them from the same rules.
        QStringList targets;
        if (desktopPath.endsWith(QLatin1String(".desktop"))) {
            targets << desktopPath;
        } else {
            targets << binaryPath;
            targets << desktopPath;
        }

        bool anyOpt = false;
        for (const QString& target : targets) {
            if (target.startsWith(QLatin1String("/opt/")))
                anyOpt = true;
        }
        build.auth = anyOpt ? AuthMethod::Pkexec : AuthMethod::None;

        for (const QString& target : targets) {
            if (target.isEmpty())
                continue;
            PlanStep step;
            const bool isDesktop = target.endsWith(QLatin1String(".desktop"));
            step.description = isDesktop ? QStringLiteral("Remove desktop entry")
                                         : QStringLiteral("Move to Trash");
            step.commandSummary =
                target.startsWith(QLatin1String("/opt/"))
                    ? QStringLiteral("pkexec gio trash %1").arg(target)
                    : QStringLiteral("gio trash %1").arg(target);
            build.steps.append(step);
        }
        break;
    }
    }

    return build;
}

// ---- Manual removal target computation (mirrors Rust install_root_for) ----

QString installRootFor(const QString& binary)
{
    const QFileInfo info(binary);
    const QString canonical = info.canonicalFilePath();
    const QString path = !canonical.isEmpty() ? canonical : binary;
    const QString home = QDir::homePath();

    auto dirBundleUnder = [&](const QString& parent) -> QString {
        // <parent>/<bundle>/... -> <parent>/<bundle> when it is a directory
        const QString parentPrefix = parent + QLatin1Char('/');
        if (!path.startsWith(parentPrefix))
            return {};
        const QString rest = path.mid(parentPrefix.size());
        const int slash = rest.indexOf(QLatin1Char('/'));
        if (slash <= 0)
            return {};
        const QString bundle = parentPrefix + rest.left(slash);
        return QFileInfo(bundle).isDir() ? bundle : QString();
    };

    // 1) .app bundles anywhere in the path.
    const int appIdx = path.indexOf(QStringLiteral(".app/"));
    if (appIdx > 0) {
        const QString bundle = path.left(appIdx + 4); // include ".app"
        if (QFileInfo(bundle).isDir())
            return bundle;
    }

    // 2) ~/.local/opt/<bundle>/...
    if (auto root = dirBundleUnder(home + QStringLiteral("/.local/opt")); !root.isEmpty())
        return root;

    // 3) /opt/<bundle>/...
    if (auto root = dirBundleUnder(QStringLiteral("/opt")); !root.isEmpty())
        return root;

    // 4) ~/Downloads/<bundle>/... only when it looks like a directory install.
    {
        const QString downloads = home + QStringLiteral("/Downloads");
        const QString downloadsPrefix = downloads + QLatin1Char('/');
        if (path.startsWith(downloadsPrefix)) {
            const QString rest = path.mid(downloadsPrefix.size());
            const int slash = rest.indexOf(QLatin1Char('/'));
            if (slash > 0) {
                const QString bundle = downloadsPrefix + rest.left(slash);
                if (QFileInfo(bundle).isDir() &&
                    (QFileInfo(bundle + QStringLiteral("/bin")).isDir() ||
                     QFileInfo(bundle + QStringLiteral("/lib")).isDir())) {
                    return bundle;
                }
            }
        }
    }

    return path;
}

QStringList manualRemovalTargets(const InstalledPackage& pkg)
{
    const QStringList parts = pkg.packageId.split(kUnitSeparator);
    const QString binaryPath = parts.value(0);
    const QString desktopPath = parts.size() > 1 ? parts.value(1) : QString();

    QStringList targets;
    if (desktopPath.endsWith(QLatin1String(".desktop"))) {
        targets << desktopPath;
        return targets;
    }

    const QString root = installRootFor(binaryPath);
    targets << root;

    // PATH shims in ~/.local/bin pointing into the root or at the binary.
    const QString localBin = QDir::homePath() + QStringLiteral("/.local/bin");
    QDir binDir(localBin);
    if (binDir.exists()) {
        for (const QFileInfo& entry :
             binDir.entryInfoList(QDir::Files | QDir::NoDotAndDotDot)) {
            if (!entry.isSymLink())
                continue;
            const QString target = entry.symLinkTarget();
            if (target == root || target.startsWith(root + QLatin1Char('/')) ||
                target == binaryPath) {
                targets << entry.absoluteFilePath();
            }
        }
    }

    if (!desktopPath.isEmpty())
        targets << desktopPath;

    QStringList unique;
    for (const QString& t : targets) {
        if (!t.isEmpty() && !unique.contains(t))
            unique << t;
    }
    return unique;
}

OperationResult trashOne(const QString& path, AuthMethod auth)
{
    OperationResult result;

    if (System::which(QStringLiteral("gio"))) {
        result = System::runElevated(QStringLiteral("gio"),
                                     { QStringLiteral("trash"), QStringLiteral("-f"), path },
                                     auth, System::TrashTimeoutMs);
        if (result.success || auth == AuthMethod::Pkexec)
            return result;
    }

    // Fallback: move into the user trash ourselves.
    const QFileInfo info(path);
    if (!info.exists()) {
        result.success = false;
        result.message = QStringLiteral("Path no longer exists: %1").arg(path);
        return result;
    }

    QDir trashFiles = QDir::homePath() + QStringLiteral("/.local/share/Trash/files");
    if (!trashFiles.exists())
        QDir::home().mkpath(QStringLiteral(".local/share/Trash/files"));

    const QString destination = trashFiles.absolutePath() + QLatin1Char('/') +
                                info.fileName() + QLatin1Char('.') +
                                QString::number(QDateTime::currentMSecsSinceEpoch());
    if (QFile::rename(path, destination) || QDir().rename(path, destination)) {
        result.success = true;
        result.message = QStringLiteral("Moved %1 to Trash.").arg(path);
        result.logs = QStringLiteral("[scope] moved %1 -> %2\n").arg(path, destination);
    } else {
        result.success = false;
        result.message = QStringLiteral("Failed to move %1 to Trash.").arg(path);
    }
    return result;
}

OperationResult applyManual(const OperationPlan& plan)
{
    InstalledPackage pkg;
    pkg.source = PackageSource::Manual;
    pkg.packageId = plan.packageId;
    pkg.displayName = plan.displayName;

    const QStringList targets = manualRemovalTargets(pkg);

    int removed = 0;
    bool firstAttemptFailed = false;
    bool attemptedAny = false;
    QString failureMessage;
    QString logs;

    for (const QString& target : targets) {
        const Protection check = checkPathKind(
            target, target.endsWith(QLatin1String(".desktop")) ? PathKind::DesktopEntry
                                                               : PathKind::Manual);
        if (check.isProtected) {
            logs += QStringLiteral("[scope] skipping protected path: %1 (%2)\n")
                        .arg(target, check.reason);
            continue;
        }

        const AuthMethod auth = target.startsWith(QLatin1String("/opt/"))
            ? AuthMethod::Pkexec
            : AuthMethod::None;

        const OperationResult trashResult = trashOne(target, auth);
        logs += trashResult.logs;

        if (!trashResult.success && !QFileInfo::exists(target)) {
            // Already gone counts as removed.
            ++removed;
            continue;
        }

        if (!trashResult.success) {
            if (!attemptedAny) {
                firstAttemptFailed = true;
                failureMessage = trashResult.message;
            } else {
                logs += QStringLiteral("[scope] warning: failed on %1: %2\n")
                            .arg(target, trashResult.message);
            }
            continue;
        }

        attemptedAny = true;
        ++removed;
    }

    OperationResult result;
    result.logs = logs;
    if (firstAttemptFailed) {
        result.success = false;
        result.message = failureMessage;
        return result;
    }
    if (removed == 0) {
        result.success = false;
        result.message = QStringLiteral("Nothing was removed; all targets were protected or missing.");
        return result;
    }
    result.success = true;
    result.message = QStringLiteral("Manual install moved to Trash (%1 item(s)).").arg(removed);
    return result;
}

} // namespace

OperationPlan preview(const InstalledPackage& pkg)
{
    OperationPlan plan;
    plan.operation = Operation::Uninstall;
    plan.source = pkg.source;
    plan.packageId = pkg.packageId;
    plan.installScope = pkg.installScope;
    plan.displayName = pkg.effectiveName();
    plan.currentVersion = pkg.version;
    plan.targetVersion = QString();

    const Protection protection = checkPackage(pkg.source, pkg.packageId);
    plan.isProtected = protection.isProtected;
    plan.protectionReason = protection.isProtected
        ? std::make_optional<QString>(protection.reason)
        : std::nullopt;

    const StepBuild build = buildSteps(pkg, protection);
    plan.steps = build.steps;
    plan.authMethod = build.auth;
    plan.requiresAuth = build.auth == AuthMethod::Pkexec;
    plan.createdAtMs = QDateTime::currentMSecsSinceEpoch();
    return plan;
}

QString revalidate(const OperationPlan& plan, const CachedScan& freshScan)
{
    for (const InstalledPackage& pkg : freshScan.packages) {
        if (pkg.source == plan.source && pkg.packageId == plan.packageId &&
            pkg.installScope == plan.installScope) {
            const Protection protection = checkPackage(plan.source, plan.packageId);
            if (protection.isProtected) {
                return QStringLiteral("Refusing to remove protected package: %1")
                    .arg(protection.reason);
            }
            return {};
        }
    }
    return QStringLiteral(
               "This uninstall plan is stale: '%1' is no longer installed. Rescan and try again.")
        .arg(plan.displayName);
}

OperationResult apply(const OperationPlan& plan)
{
    switch (plan.source) {
    case PackageSource::Apt:
        return System::runElevated(QStringLiteral("apt"),
                                   { QStringLiteral("remove"), QStringLiteral("-y"),
                                     plan.packageId },
                                   AuthMethod::Pkexec, System::UninstallTimeoutMs);
    case PackageSource::Snap:
        return System::runElevated(QStringLiteral("snap"),
                                   { QStringLiteral("remove"), plan.packageId },
                                   AuthMethod::Pkexec, System::UninstallTimeoutMs);
    case PackageSource::Flatpak: {
        const bool user = plan.installScope == InstallScope::User;
        QStringList args = { QStringLiteral("uninstall"), QStringLiteral("-y"),
                             user ? QStringLiteral("--user") : QStringLiteral("--system"),
                             plan.packageId };
        return System::runElevated(QStringLiteral("flatpak"), args,
                                   user ? AuthMethod::None : AuthMethod::Pkexec,
                                   System::UninstallTimeoutMs);
    }
    case PackageSource::AppImage:
        return trashOne(plan.packageId, AuthMethod::None);
    case PackageSource::Manual:
        return applyManual(plan);
    }

    OperationResult fail;
    fail.success = false;
    fail.message = QStringLiteral("Unsupported source for uninstall.");
    return fail;
}

} // namespace uninstall
} // namespace scope
