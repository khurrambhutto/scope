#pragma once

#include <QAbstractListModel>
#include <QVector>

namespace scope {

// Non-blocking background operation tasks (uninstall/update) with logs.
class TaskModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int activeCount READ activeCount NOTIFY countsChanged)

public:
    enum class Status { Running, Success, Failed };

    enum Roles {
        IdRole = Qt::UserRole + 1,
        TitleRole,
        StatusRole,
        StatusTextRole,
        MessageRole,
        LogsRole,
        StartedTextRole,
        DurationTextRole,
    };

    explicit TaskModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    int beginTask(const QString& title);
    void finishTask(int taskId, bool success, const QString& message, const QString& logs);

    Q_INVOKABLE void clearFinished();

    [[nodiscard]] int activeCount() const;

signals:
    void countsChanged();

private:
    struct Task {
        int id = 0;
        QString title;
        Status status = Status::Running;
        QString message;
        QString logs;
        qint64 startedAtMs = 0;
        qint64 finishedAtMs = 0;
    };

    QVector<Task> m_tasks;
    int m_nextId = 1;
};

} // namespace scope
