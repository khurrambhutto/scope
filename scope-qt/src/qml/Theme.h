#pragma once

#include <QColor>
#include <QObject>
#include <QString>

namespace scope {

// Design tokens for the QML UI. Exposed as a context property named "Theme".
class ThemeColors : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QColor background READ background CONSTANT)
    Q_PROPERTY(QColor surface READ surface CONSTANT)
    Q_PROPERTY(QColor surfaceAlt READ surfaceAlt CONSTANT)
    Q_PROPERTY(QColor border READ border CONSTANT)
    Q_PROPERTY(QColor text READ text CONSTANT)
    Q_PROPERTY(QColor textDim READ textDim CONSTANT)
    Q_PROPERTY(QColor accent READ accent CONSTANT)
    Q_PROPERTY(QColor accentHover READ accentHover CONSTANT)
    Q_PROPERTY(QColor success READ success CONSTANT)
    Q_PROPERTY(QColor danger READ danger CONSTANT)
    Q_PROPERTY(QColor warning READ warning CONSTANT)
    Q_PROPERTY(int radius READ radius CONSTANT)
    Q_PROPERTY(int padding READ padding CONSTANT)

public:
    explicit ThemeColors(QObject* parent = nullptr)
        : QObject(parent)
    {
    }

    [[nodiscard]] QColor background() const { return { "#0f1115" }; }
    [[nodiscard]] QColor surface() const { return { "#161a22" }; }
    [[nodiscard]] QColor surfaceAlt() const { return { "#1d2330" }; }
    [[nodiscard]] QColor border() const { return { "#2a3142" }; }
    [[nodiscard]] QColor text() const { return { "#e6e9f0" }; }
    [[nodiscard]] QColor textDim() const { return { "#8b93a7" }; }
    [[nodiscard]] QColor accent() const { return { "#4f8cff" }; }
    [[nodiscard]] QColor accentHover() const { return { "#6b9fff" }; }
    [[nodiscard]] QColor success() const { return { "#3fb96f" }; }
    [[nodiscard]] QColor danger() const { return { "#e5534b" }; }
    [[nodiscard]] QColor warning() const { return { "#d9a53a" }; }
    [[nodiscard]] int radius() const { return 10; }
    [[nodiscard]] int padding() const { return 16; }

    Q_INVOKABLE [[nodiscard]] QColor sourceColor(const QString& source) const
    {
        if (source == QLatin1String("apt"))
            return { "#d9a53a" };
        if (source == QLatin1String("snap"))
            return { "#e5534b" };
        if (source == QLatin1String("flatpak"))
            return { "#4f8cff" };
        if (source == QLatin1String("appimage"))
            return { "#9d6bff" };
        return { "#3fb96f" };
    }
};

} // namespace scope
