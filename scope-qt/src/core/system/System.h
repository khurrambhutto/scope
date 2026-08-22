#pragma once

#include <QString>
#include <QStringList>

#include <optional>

namespace scope {

enum class AuthMethod {
    None,
    Pkexec,
};

struct ExecOutcome {
    bool spawned = false;
    bool timedOut = false;
    int exitCode = -1;
    bool exitOk = false;
    QString standardOutput;
    QString standardError;
};

struct OperationResult {
    bool success = false;
    QString message;
    QString logs;
    std::optional<int> exitCode;
};

class System
{
public:
    static constexpr int ScanTimeoutMs = 30'000;
    static constexpr int DpkgTimeoutMs = 20'000;
    static constexpr int DuTimeoutMs = 5'000;
    static constexpr int TrashTimeoutMs = 20'000;
    static constexpr int UninstallTimeoutMs = 180'000;
    static constexpr int UpdateTimeoutMs = 300'000;

    // Runs a command to completion capturing stdout/stderr. Never uses a shell.
    [[nodiscard]] static ExecOutcome capture(const QString& program, const QStringList& args,
                                             int timeoutMs);

    [[nodiscard]] static bool which(const QString& program);
    [[nodiscard]] static QString absolutePath(const QString& program);
    [[nodiscard]] static QString homeDir();

    // Single execution path for all mutating operations. Returns structured
    // results (never throws); logs contain stdout/stderr plus an audit line.
    [[nodiscard]] static OperationResult runElevated(const QString& program, const QStringList& args,
                                                     AuthMethod auth, int timeoutMs);
};

} // namespace scope
