import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: page
    color: Theme.background

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 14

        RowLayout {
            Layout.fillWidth: true

            ColumnLayout {
                spacing: 2
                Text {
                    text: "Tasks"
                    color: Theme.text
                    font.pixelSize: 22
                    font.bold: true
                }
                Text {
                    text: Operations.tasks.activeCount > 0
                        ? Operations.tasks.activeCount + " running in the background"
                        : "Operations never block the app"
                    color: Theme.textDim
                    font.pixelSize: 12
                }
            }

            Item { Layout.fillWidth: true }

            Button {
                text: "Clear finished"
                onClicked: Operations.tasks.clearFinished()

                background: Rectangle {
                    radius: Theme.radius
                    color: parent.hovered ? Theme.surfaceAlt : Theme.surface
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
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Theme.radius
            color: Theme.surface
            border.color: Theme.border

            ListView {
                id: taskList
                anchors.fill: parent
                anchors.margins: 8
                clip: true
                spacing: 6
                model: Operations.tasks

                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

                delegate: Rectangle {
                    width: taskList.width
                    height: expanded ? (200 + logsArea.implicitHeight) : 64
                    Behavior on height { NumberAnimation { duration: 120 } }
                    radius: 8
                    color: Theme.background
                    border.color: Theme.border

                    property bool expanded: false

                    MouseArea {
                        anchors.fill: parent
                        onClicked: parent.expanded = !parent.expanded
                        cursorShape: Qt.PointingHandCursor
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 6

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Rectangle {
                                radius: 9
                                implicitWidth: statusChip.implicitWidth + 16
                                implicitHeight: 18
                                color: {
                                    if (model.status === 0) return Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.15)
                                    if (model.status === 1) return Qt.rgba(Theme.success.r, Theme.success.g, Theme.success.b, 0.15)
                                    return Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.15)
                                }
                                Text {
                                    id: statusChip
                                    anchors.centerIn: parent
                                    text: model.statusText
                                    color: model.status === 0 ? Theme.accent : (model.status === 1 ? Theme.success : Theme.danger)
                                    font.pixelSize: 10
                                    font.bold: true
                                }
                            }

                            Text {
                                text: model.title
                                color: Theme.text
                                font.pixelSize: 13
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                                Layout.maximumWidth: 400
                            }

                            Item { Layout.fillWidth: true }

                            Text {
                                text: model.startedText + " · " + model.durationText
                                color: Theme.textDim
                                font.pixelSize: 11
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: model.message
                            color: model.status === 2 ? Theme.danger : Theme.textDim
                            font.pixelSize: 11
                            elide: Text.ElideRight
                            visible: model.message !== ""
                        }

                        TextArea {
                            id: logsArea
                            Layout.fillWidth: true
                            text: model.logs
                            visible: expanded && model.logs !== ""
                            readOnly: true
                            wrapMode: TextArea.WrapAnywhere
                            color: Theme.textDim
                            font.family: "monospace"
                            font.pixelSize: 10
                            background: Rectangle {
                                color: "transparent"
                                border.color: Theme.border
                                radius: 6
                            }
                        }
                    }
                }

                Text {
                    anchors.centerIn: parent
                    visible: taskList.count === 0
                    text: "No operations yet.\nUninstall or update a package to see it here."
                    horizontalAlignment: Text.AlignHCenter
                    color: Theme.textDim
                    font.pixelSize: 13
                }
            }
        }
    }
}
