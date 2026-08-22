#include "UpdateOp.h"

#include "safety/Safety.h"
#include "system/System.h"

#include <QDateTime>

namespace scope {
namespace update {

namespace {

struct StepBuild {
    QVector<PlanStep> steps;
    AuthMethod auth = AuthMethod::None;
};

StepBuild buildSteps(const InstalledPackage& pkg, bool isProtected)
{
    StepBuild build;

    if (isProtected) {
        PlanStep step;
        step.description = QStringLiteral("(no command — protected)");
        step.commandSummary = QStringLiteral("Blocked: this package is protected.");
        build.steps.append(step);
        return build;
    }

    switch (pkg.source) {
    case PackageSource::Apt: {
        PlanStep step;
        step.description = QStringLiteral("Upgrade APT package '%1'").arg(pkg.packageId);
        step.commandSummary =
            QStringLiteral("pkexec env DEBIAN_FRONTEND=noninteractive apt install -y %1")
                .arg(pkg.packageId);
        build.steps.append(step);
        build.auth = AuthMethod::Pkexec;
        break;
    }
    case PackageSource::Snap: {
        PlanStep step;
        step.description = QStringLiteral("Refresh Snap package '%1'").arg(pkg.packageId);
        step.commandSummary = QStringLiteral("pkexec snap refresh %1").arg(pkg.packageId);
        build.steps.append(step);
        build.auth = AuthMethod::Pkexec;
        break;
    }
    case PackageSource::Flatpak: {
        const bool user = pkg.installScope == InstallScope::User;
        PlanStep step;
        step.description = QStringLiteral("Update Flatpak app '%1' (%2)")
                               .arg(pkg.packageId,
                                    user ? QStringLiteral("user install")
                                         : QStringLiteral("system install"));
        step.commandSummary = user
            ? QStringLiteral("flatpak update -y --user %1").arg(pkg.packageId)
            : QStringLiteral("pkexec flatpak update -y --system %1").arg(pkg.packageId);
        build.steps.append(step);
        build.auth = user ? AuthMethod::None : AuthMethod::Pkexec;
        break;
    }
    case PackageSource::AppImage: {
        PlanStep step;
        step.description = QStringLiteral("Download and replace %1").arg(pkg.packageId);
        step.commandSummary = QStringLiteral("Download and replace %1").arg(pkg.packageId);
        build.steps.append(step);
        build.auth = AuthMethod::None;
        break;
    }
    case PackageSource::Manual: {
        PlanStep step;
        step.description = QStringLiteral("(no command — manual install)");
        step.commandSummary = QStringLiteral("(no command — manual install)");
        build.steps.append(step);
        build.auth = AuthMethod::None;
        break;
    }
    }

    return build;
}

} // namespace

OperationPlan preview(const InstalledPackage& pkg)
{
    OperationPlan plan;
    plan.operation = Operation::Update;
    plan.source = pkg.source;
    plan.packageId = pkg.packageId;
    plan.installScope = pkg.installScope;
    plan.displayName = pkg.effectiveName();
    plan.currentVersion = pkg.version;

    // Manual installs are force-protected for updates.
    Protection protection;
    if (pkg.source == PackageSource::Manual) {
        protection.isProtected = true;
        protection.reason =
            QStringLiteral("Manual installs cannot be updated through Scope. "
                           "Reinstall from the original source.");
    } else {
        protection = checkPackage(pkg.source, pkg.packageId);
    }

    plan.isProtected = protection.isProtected;
    plan.protectionReason = protection.isProtected
        ? std::make_optional<QString>(protection.reason)
        : std::nullopt;
    plan.targetVersion = pkg.updateVersion.value_or(QStringLiteral("latest"));

    const StepBuild build = buildSteps(pkg, protection.isProtected);
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
            if (!pkg.hasUpdate) {
                return QStringLiteral(
                           "'%1' no longer has updates available. Rescan and try again.")
                    .arg(plan.displayName);
            }
            return {};
        }
    }
    return QStringLiteral(
               "This update plan is stale: '%1' is no longer installed. Rescan and try again.")
        .arg(plan.displayName);
}

OperationResult apply(const OperationPlan& plan)
{
    switch (plan.source) {
    case PackageSource::Apt:
        return System::runElevated(QStringLiteral("apt"),
                                   { QStringLiteral("install"), QStringLiteral("-y"),
                                     plan.packageId },
                                   AuthMethod::Pkexec, System::UpdateTimeoutMs);
    case PackageSource::Snap:
        return System::runElevated(QStringLiteral("snap"),
                                   { QStringLiteral("refresh"), plan.packageId },
                                   AuthMethod::Pkexec, System::UpdateTimeoutMs);
    case PackageSource::Flatpak: {
        const bool user = plan.installScope == InstallScope::User;
        QStringList args = { QStringLiteral("update"), QStringLiteral("-y"),
                             user ? QStringLiteral("--user") : QStringLiteral("--system"),
                             plan.packageId };
        return System::runElevated(QStringLiteral("flatpak"), args,
                                   user ? AuthMethod::None : AuthMethod::Pkexec,
                                   System::UpdateTimeoutMs);
    }
    case PackageSource::AppImage: {
        OperationResult result;
        result.success = false;
        result.message = QStringLiteral("AppImage auto-update is not yet implemented. "
                                        "Download the latest version from the project website.");
        return result;
    }
    case PackageSource::Manual: {
        OperationResult result;
        result.success = false;
        result.message = QStringLiteral("Manual installs cannot be updated through Scope. "
                                        "Reinstall from the original source.");
        return result;
    }
    }

    OperationResult fail;
    fail.success = false;
    fail.message = QStringLiteral("Unsupported source for update.");
    return fail;
}

} // namespace update
} // namespace scope
