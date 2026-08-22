#include "System.h"

#include <QElapsedTimer>
#include <QFileInfo>
#include <QProcess>

namespace scope {

namespace {

// Formats argv the way the Rust port logged it: ["a", "b", "c"]
QString debugArgs(const QStringList& args)
{
    QString out = QStringLiteral("[");
    for (int i = 0; i < args.size(); ++i) {
        if (i > 0)
            out += QStringLiteral(", ");
        out += QLatin1Char('"') + args.at(i) + QLatin1Char('"');
    }
    out += QLatin1Char(']');
    return out;
}

} // namespace

ExecOutcome System::capture(const QString& program, const QStringList& args, int timeoutMs)
{
    ExecOutcome outcome;
    QProcess process;

    process.start(program, args);
    if (!process.waitForStarted(5'000))
        return outcome; // spawned == false

    const bool finished = process.waitForFinished(timeoutMs);
    outcome.spawned = true;
    outcome.standardOutput = QString::fromLocal8Bit(process.readAllStandardOutput());
    outcome.standardError = QString::fromLocal8Bit(process.readAllStandardError());
    if (!finished) {
        process.kill();
        process.waitForFinished(2'000);
        outcome.timedOut = true;
        return outcome;
    }

    outcome.exitCode = process.exitCode();
    outcome.exitOk = process.exitStatus() == QProcess::NormalExit && process.exitCode() == 0;
    return outcome;
}

bool System::which(const QString& program)
{
    if (program.isEmpty() || program.contains(QLatin1Char('/')))
        return false;

    QStringList searchDirs;
    searchDirs << homeDir() + QStringLiteral("/.local/bin")
               << QStringLiteral("/usr/local/bin")
               << QStringLiteral("/usr/bin")
               << QStringLiteral("/bin")
               << QStringLiteral("/usr/sbin")
               << QStringLiteral("/sbin");

    for (const QString& dir : std::as_const(searchDirs)) {
        const QFileInfo info(dir + QLatin1Char('/') + program);
        if (info.isFile())
            return true;
    }

    const QStringList dirs =
        qEnvironmentVariable("PATH").split(QLatin1Char(':'), Qt::SkipEmptyParts);
    for (const QString& dir : dirs) {
        const QFileInfo info(dir + QLatin1Char('/') + program);
        if (info.isFile())
            return true;
    }
    return false;
}

QString System::absolutePath(const QString& program)
{
    static const char* kDirs[] = {
        "/usr/bin", "/bin", "/usr/local/bin", "/usr/sbin", "/sbin",
    };
    for (const char* dir : kDirs) {
        const QString candidate = QString::fromLatin1(dir) + QLatin1Char('/') + program;
        if (QFileInfo::exists(candidate))
            return candidate;
    }
    return program;
}

QString System::homeDir()
{
    return qEnvironmentVariable("HOME");
}

OperationResult System::runElevated(const QString& program, const QStringList& args,
                                    AuthMethod auth, int timeoutMs)
{
    OperationResult result;

    const QString absProgram = absolutePath(program);
    const QString displayProgram = auth == AuthMethod::Pkexec
        ? QStringLiteral("pkexec env DEBIAN_FRONTEND=noninteractive ") + absProgram
        : absProgram;

    QElapsedTimer timer;
    timer.start();

    QProcess process;
    if (auth == AuthMethod::Pkexec) {
        const QStringList argv = { QStringLiteral("env"),
                                   QStringLiteral("DEBIAN_FRONTEND=noninteractive"),
                                   absProgram };
        process.start(QStringLiteral("pkexec"), argv + args);
    } else {
        process.start(absProgram, args);
    }

    if (!process.waitForStarted(5'000)) {
        result.success = false;
        result.message = QStringLiteral("Failed to start command: %1").arg(displayProgram);
        result.logs = QStringLiteral("[scope] failed to spawn: %1\n").arg(displayProgram);
        result.exitCode = std::nullopt;
        return result;
    }

    const bool finished = process.waitForFinished(timeoutMs);
    const QString stdoutText = QString::fromLocal8Bit(process.readAllStandardOutput());
    const QString stderrText = QString::fromLocal8Bit(process.readAllStandardError());

    if (!finished) {
        process.kill();
        process.waitForFinished(2'000);
        result.success = false;
        result.exitCode = std::nullopt;
        result.message = QStringLiteral("Operation timed out after %1s.").arg(timeoutMs / 1000);
        result.logs = QStringLiteral("--- stdout ---\n%1\n--- stderr ---\n%2\n\n"
                                      "[scope] ran: %3 %4 (%5ms) [timed out]")
                          .arg(stdoutText, stderrText, displayProgram, debugArgs(args))
                          .arg(timer.elapsed());
        return result;
    }

    const int exitCode = process.exitCode();
    const bool ok = process.exitStatus() == QProcess::NormalExit && exitCode == 0;

    result.logs = QStringLiteral("--- stdout ---\n%1\n--- stderr ---\n%2\n\n[scope] ran: %3 %4 (%5ms)")
                      .arg(stdoutText, stderrText, displayProgram, debugArgs(args))
                      .arg(timer.elapsed());
    result.exitCode = exitCode;
    result.success = ok;

    if (ok) {
        result.message = QStringLiteral("Operation completed successfully.");
    } else {
        QString firstStderr;
        const QStringList lines = stderrText.split(QLatin1Char('\n'));
        for (const QString& line : lines) {
            const QString trimmed = line.trimmed();
            if (!trimmed.isEmpty()) {
                firstStderr = trimmed;
                break;
            }
        }
        result.message = QStringLiteral("Operation failed (exit %1): %2").arg(exitCode).arg(firstStderr);
    }
    return result;
}

} // namespace scope
