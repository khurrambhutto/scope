import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: root

    visible: true
    width: 1180
    height: 760
    minimumWidth: 940
    minimumHeight: 600
    title: "Scope"
    color: Theme.background

    Component.onCompleted: { Packages.refresh() }

    component Badge: Rectangle {
        property string text
        property color bgColor: Theme.accent
        width: Math.max(badgeText.implicitWidth + 12, 20)
        height: 20
        radius: 10
        color: bgColor
        Text {
            id: badgeText
            anchors.centerIn: parent
            text: parent.text
            color: "white"
            font.pixelSize: 11
            font.bold: true
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // Sidebar
        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 210
            color: Theme.surface

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 6

                RowLayout {
                    spacing: 8
                    Rectangle {
                        width: 26; height: 26; radius: 8
                        color: Theme.accent
                        Text {
                            anchors.centerIn: parent
                            text: "S"
                            color: "white"
                            font.pixelSize: 15
                            font.bold: true
                        }
                    }
                    Text {
                        text: "Scope"
                        color: Theme.text
                        font.pixelSize: 17
                        font.bold: true
                    }
                }

                Item { Layout.fillHeight: true; Layout.preferredHeight: 10 }

                Repeater {
                    model: [
                        { key: "packages", label: "Packages", icon: "▤" },
                        { key: "tasks", label: "Tasks", icon: "◷" }
                    ]

                    delegate: AbstractButton {
                        id: navButton
                        property bool active: viewStack.currentIndex === index
                        Layout.fillWidth: true
                        height: 38

                        onClicked: {
                            viewStack.currentIndex = index
                        }

                        background: Rectangle {
                            radius: Theme.radius
                            color: navButton.active ? Theme.surfaceAlt : (navButton.hovered ? Theme.surfaceAlt : "transparent")
                        }

                        contentItem: RowLayout {
                            spacing: 10
                            Text {
                                text: modelData.icon
                                color: navButton.active ? Theme.accent : Theme.textDim
                                font.pixelSize: 14
                            }
                            Text {
                                text: modelData.label
                                color: navButton.active ? Theme.text : Theme.textDim
                                font.pixelSize: 13
                                font.weight: navButton.active ? Font.DemiBold : Font.Normal
                            }
                            Item { Layout.fillWidth: true }
                            Badge {
                                visible: index === 1 && Operations.tasks.activeCount > 0
                                text: Operations.tasks.activeCount
                                bgColor: Theme.accent
                            }
                        }
                    }
                }

                Item { Layout.fillWidth: true; Layout.fillHeight: true }

                ColumnLayout {
                    spacing: 4
                    Text {
                        text: Packages.scanning ? "Scanning…" : ("Updated " + Packages.lastScanText)
                        color: Theme.textDim
                        font.pixelSize: 11
                    }
                }
            }
        }

        // Content
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Theme.background

            StackLayout {
                id: viewStack
                anchors.fill: parent
                currentIndex: 0

                PackagesPage {}
                TaskCenterPage {}
            }
        }
    }

    PreviewDialog {
        id: previewDialog
    }

    Connections {
        target: Operations
        function onPreviewReady(plan) {
            previewDialog.openWithPlan(plan)
        }
        function onPreviewFailed(message) {
            errorToast.show(message)
        }
    }

    // Toast for errors
    Rectangle {
        id: errorToast
        function show(message) {
            toastText.text = message
            toastTimer.restart()
            errorToast.visible = true
        }
        visible: false
        z: 100
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 18
        radius: Theme.radius
        color: Theme.danger
        width: Math.min(toastText.implicitWidth + 32, root.width - 60)
        height: 40

        Text {
            id: toastText
            anchors.centerIn: parent
            color: "white"
            font.pixelSize: 12
            elide: Text.ElideRight
            width: Math.min(implicitWidth, parent.width - 24)
        }
        Timer {
            id: toastTimer
            interval: 4000
            onTriggered: errorToast.visible = false
        }
    }
}
