import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: row

    property bool selected: false
    signal selectRequested(int row)

    height: 58
    radius: 8
    color: selected ? Theme.surfaceAlt : (ma.hovered ? Theme.surfaceAlt : "transparent")

    MouseArea {
        id: ma
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: row.selectRequested(index)
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        spacing: 12

        Rectangle {
            Layout.preferredWidth: 38
            Layout.preferredHeight: 38
            radius: 9
            color: Theme.background

            Image {
                anchors.fill: parent
                anchors.margins: 5
                source: model.iconUrl
                visible: model.iconUrl !== ""
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: true
            }
            Text {
                anchors.centerIn: parent
                visible: model.iconUrl === ""
                text: model.appKind === "cli" ? "⌘" : "▣"
                color: Theme.sourceColor(model.source)
                font.pixelSize: 17
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                spacing: 6
                Text {
                    text: model.name
                    color: Theme.text
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.maximumWidth: 320
                }
                Rectangle {
                    visible: model.hasUpdate
                    radius: 8
                    color: "transparent"
                    border.color: Theme.warning
                    implicitWidth: updateLabel.implicitWidth + 12
                    implicitHeight: 16
                    Text {
                        id: updateLabel
                        anchors.centerIn: parent
                        text: model.updateVersion !== "" ? ("↑ " + model.updateVersion) : "↑ update"
                        color: Theme.warning
                        font.pixelSize: 10
                    }
                }
                Rectangle {
                    visible: model.isProtected
                    radius: 8
                    color: "transparent"
                    border.color: Theme.border
                    implicitWidth: protLabel.implicitWidth + 12
                    implicitHeight: 16
                    Text {
                        id: protLabel
                        anchors.centerIn: parent
                        text: "protected"
                        color: Theme.textDim
                        font.pixelSize: 10
                    }
                }
            }

            Text {
                text: {
                    let id = model.packageId
                    if (id.length > 60)
                        id = id.substring(0, 57) + "…"
                    return id
                }
                color: Theme.textDim
                font.pixelSize: 11
                elide: Text.ElideRight
                Layout.maximumWidth: 380
            }
        }

        Item { Layout.fillWidth: true }

        Rectangle {
            radius: 8
            color: Qt.rgba(Theme.sourceColor(model.source).r, Theme.sourceColor(model.source).g,
                           Theme.sourceColor(model.source).b, 0.15)
            implicitWidth: sourceLabel.implicitWidth + 14
            implicitHeight: 20
            Text {
                id: sourceLabel
                anchors.centerIn: parent
                text: model.installScope !== "" ? (model.sourceLabel + " · " + model.installScope)
                                                : model.sourceLabel
                color: Theme.sourceColor(model.source)
                font.pixelSize: 10
                font.bold: true
            }
        }

        Text {
            text: model.sizeText
            color: Theme.textDim
            font.pixelSize: 11
            Layout.preferredWidth: 64
            horizontalAlignment: Text.AlignRight
        }
    }
}
