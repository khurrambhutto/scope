#include "TaskModel.h"

#include <QDateTime>

namespace scope {

TaskModel::TaskModel(QObject* parent)
    : QAbstractListModel(parent)
{
}

int TaskModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : m_tasks.size();
}

QHash<int, QByteArray> TaskModel::roleNames() const
{
    QHash<int, QByteArray> roles;
    roles[IdRole] = "taskId";
    roles[TitleRole] = "title";
    roles[StatusRole] = "status";
    roles[StatusTextRole] = "statusText";
    roles[MessageRole] = "message";
    roles[LogsRole] = "logs";
    roles[StartedTextRole] = "startedText";
    roles[DurationTextRole] = "durationText";
    return roles;
}

QVariant TaskModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() >= m_tasks.size())
        return {};
    const Task& task = m_tasks.at(index.row());

    switch (role) {
    case IdRole:
        return task.id;
    case TitleRole:
        return task.title;
    case StatusRole:
        return static_cast<int>(task.status);
    case StatusTextRole:
        switch (task.status) {
        case Status::Running: return QStringLiteral("Running");
        case Status::Success: return QStringLiteral("Done");
        case Status::Failed: return QStringLiteral("Failed");
        }
        return {};
    case MessageRole:
        return task.message;
    case LogsRole:
        return task.logs;
    case StartedTextRole:
        return QDateTime::fromMSecsSinceEpoch(task.startedAtMs).toString(QStringLiteral("hh:mm:ss"));
    case DurationTextRole: {
        if (task.startedAtMs == 0)
            return {};
        const qint64 end = task.finishedAtMs > 0
            ? task.finishedAtMs
            : QDateTime::currentMSecsSinceEpoch();
        return QStringLiteral("%1s").arg((end - task.startedAtMs) / 1000);
    }
    }
    return {};
}

int TaskModel::beginTask(const QString& title)
{
    const int id = m_nextId++;
    beginInsertRows({}, m_tasks.size(), m_tasks.size());
    Task task;
    task.id = id;
    task.title = title;
    task.status = Status::Running;
    task.startedAtMs = QDateTime::currentMSecsSinceEpoch();
    m_tasks.append(std::move(task));
    endInsertRows();
    emit countsChanged();
    return id;
}

void TaskModel::finishTask(int taskId, bool success, const QString& message, const QString& logs)
{
    for (int i = 0; i < m_tasks.size(); ++i) {
        if (m_tasks.at(i).id != taskId)
            continue;
        m_tasks[i].status = success ? Status::Success : Status::Failed;
        m_tasks[i].message = message;
        m_tasks[i].logs = logs;
        m_tasks[i].finishedAtMs = QDateTime::currentMSecsSinceEpoch();
        const QModelIndex idx = index(i);
        emit dataChanged(idx, idx);
        emit countsChanged();
        return;
    }
}

void TaskModel::clearFinished()
{
    for (int i = m_tasks.size() - 1; i >= 0; --i) {
        if (m_tasks.at(i).status != Status::Running) {
            beginRemoveRows({}, i, i);
            m_tasks.removeAt(i);
            endRemoveRows();
        }
    }
}

int TaskModel::activeCount() const
{
    int count = 0;
    for (const Task& task : m_tasks) {
        if (task.status == Status::Running)
            ++count;
    }
    return count;
}

} // namespace scope
