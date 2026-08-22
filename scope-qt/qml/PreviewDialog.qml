import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Dialog {
    id: dialog

    property var plan: null

    function openWithPlan(planMap) {
        plan = planMap
        open()
    }

    width: 520
    anchors.centerIn: Overlay.overlay
    modal: true
    padding: 0

    background: Rectangle {
        radius: Theme.radius + 2
        color: Theme.surface
        border.color: Theme.border
    }

    contentItem: ColumnLayout {
        spacing: 14

        ColumnLayout {
            Layout.margins: 20
            Layout.bottomMargin: 6
            spacing: 4

            Text {
                text: {
                    if (!plan)
                        return ""
                    const verb = plan.operation === "uninstall" ? "Uninstall" : "Update"
                    return verb + " " + plan.display_name
                }
                color: Theme.text
                font.pixelSize: 17
                font.bold: true
            }

            Text {
                visible: plan && plan.operation === "update"
                text: plan ? ("Version: " + (plan.current_version || "?") + "  →  " +
                              (plan.target_version || "latest")) : ""
                color: Theme.warning
                font.pixelSize: 12
            }
        }

        // Protected block
        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: 20
            Layout.rightMargin: 20
            visible: plan && plan.protected === true
            radius: Theme.radius
            color: Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.10)
            implicitHeight: blockedText.implicitHeight + 24

            Text {
                id: blockedText
                anchors.centerIn: parent
                width: parent.width - 24
                text: plan ? ("This package is protected and cannot be modified.\n" +
                              (plan.protection_reason || "")) : ""
                color: Theme.danger
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }
        }

        // Steps preview
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 20
            Layout.rightMargin: 20
            spacing: 8
            visible: plan && plan.protected !== true

            Text {
                text: "This will run:"
                color: Theme.textDim
                font.pixelSize: 11
                font.letterSpacing: 1
            }

            Repeater {
                model: plan ? plan.steps : []

                delegate: Rectangle {
                    Layout.fillWidth: true
                    radius: 8
                    color: Theme.background
                    border.color: Theme.border
                    implicitHeight: stepColumn.implicitHeight + 16

                    ColumnLayout {
                        id: stepColumn
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 3

                        Text {
                            text: modelData.description
                            color: Theme.text
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Text {
                            text: "$ " + modelData.command_summary
                            color: Theme.textDim
                            font.pixelSize: 11
                            font.family: "monospace"
                            wrapMode: Text.WrapAnywhere
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            RowLayout {
                spacing: 6
                visible: plan && plan.requires_auth === true
                Text { text: "⚠"; color: Theme.warning; font.pixelSize: 13 }
                Text {
                    text: "Your system password will be requested via polkit."
                    color: Theme.textDim
                    font.pixelSize: 11
                }
            }
        }

        Item { Layout.preferredHeight: 4 }

        // Buttons
        RowLayout {
            Layout.margins: 20
            Layout.topMargin: 8
            spacing: 10

            Item { Layout.fillWidth: true }

            Button {
                text: "Cancel"
                onClicked: dialog.close()

                background: Rectangle {
                    radius: Theme.radius
                    color: parent.hovered ? Theme.surfaceAlt : "transparent"
                    border.color: Theme.border
                }
                contentItem: Text {
                    text: parent.text
                    color: Theme.text
                    font.pixelSize: 13
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
            }

            Button {
                text: plan && plan.operation === "uninstall" ? "Uninstall" : "Update"
                enabled: plan && plan.protected !== true
                onClicked: {
                    if (!plan)
                        return
                    if (plan.operation === "uninstall")
                        Operations.applyUninstall(plan.plan_id)
                    else
                        Operations.applyUpdate(plan.plan_id)
                    // Non-blocking: close immediately; task runs in background.
                    dialog.close()
                }

                background: Rectangle {
                    radius: Theme.radius
                    color: parent.enabled ? (parent.hovered ? Theme.accentHover : Theme.accent) : Theme.surfaceAlt
                }
                contentItem: Text {
                    text: parent.text
                    color: parent.enabled ? "white" : Theme.textDim
                    font.pixelSize: 13
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }
    }
}
