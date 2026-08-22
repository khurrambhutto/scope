import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: drawer

    property var packageInfo: null

    signal uninstallRequested(var pkg)
    signal updateRequested(var pkg)

    radius: Theme.radius
    color: Theme.surface
    border.color: Theme.border

    Connections {
        target: Operations
        function onPreviewReady(plan) {
            // keep drawer open; dialog handles flow
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 14
        visible: packageInfo !== null

        Item {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: 84
            Layout.preferredHeight: 84

            Rectangle {
                anchors.fill: parent
                radius: 20
                color: Theme.background
                border.color: Theme.border

                Image {
                    anchors.fill: parent
                    anchors.margins: 12
                    source: packageInfo ? packageInfo.iconUrl : ""
                    visible: packageInfo && packageInfo.iconUrl !== ""
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                }
                Text {
                    anchors.centerIn: parent
                    visible: !packageInfo || packageInfo.iconUrl === ""
                    text: "▣"
                    color: packageInfo ? Theme.sourceColor(packageInfo.source) : Theme.textDim
                    font.pixelSize: 34
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: packageInfo ? packageInfo.displayName : ""
            color: Theme.text
            font.pixelSize: 17
            font.bold: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            text: packageInfo ? (packageInfo.description || "") : ""
            color: Theme.textDim
            font.pixelSize: 12
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            visible: packageInfo && (packageInfo.description || "") !== ""
        }

        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: Theme.border
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 12
            rowSpacing: 10

            component MetaLabel: ColumnLayout {
                property string label
                property string value
                spacing: 2
                Text { text: parent.label.toUpperCase(); color: Theme.textDim; font.pixelSize: 9; font.letterSpacing: 1 }
                Text {
                    text: parent.value === "" ? "—" : parent.value
                    color: Theme.text
                    font.pixelSize: 12
                    elide: Text.ElideRight
                    Layout.maximumWidth: 130
                }
            }

            MetaLabel { label: "Source"; value: packageInfo ? packageInfo.sourceLabel : "" }
            MetaLabel { label: "Scope"; value: packageInfo ? (packageInfo.installScope || "—") : "" }
            MetaLabel { label: "Version"; value: packageInfo ? packageInfo.version : "" }
            MetaLabel { label: "Size"; value: packageInfo ? packageInfo.sizeText : "" }
            MetaLabel { label: "Type"; value: packageInfo ? packageInfo.appKind : "" }
            MetaLabel { label: "Package ID"; value: packageInfo ? packageInfo.packageId : "" }
        }

        Item { Layout.fillHeight: true }

        // Update banner
        Rectangle {
            Layout.fillWidth: true
            visible: packageInfo && packageInfo.hasUpdate
            radius: Theme.radius
            color: Qt.rgba(Theme.warning.r, Theme.warning.g, Theme.warning.b, 0.12)
            implicitHeight: updateBannerText.implicitHeight + 20

            Text {
                id: updateBannerText
                anchors.centerIn: parent
                width: parent.width - 20
                text: packageInfo && packageInfo.updateVersion !== ""
                    ? ("Update available → " + packageInfo.updateVersion)
                    : "Update available"
                color: Theme.warning
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }
        }

        // Protection notice
        Rectangle {
            Layout.fillWidth: true
            visible: packageInfo && packageInfo.isProtected
            radius: Theme.radius
            color: Qt.rgba(Theme.border.r, Theme.border.g, Theme.border.b, 0.4)
            implicitHeight: protNotice.implicitHeight + 20

            Text {
                id: protNotice
                anchors.centerIn: parent
                width: parent.width - 20
                text: packageInfo ? (packageInfo.protectionReason || "This package is protected.") : ""
                color: Theme.textDim
                font.pixelSize: 11
                wrapMode: Text.WrapAnywhere
            }
        }

        Button {
            Layout.fillWidth: true
            text: "Update"
            enabled: packageInfo && packageInfo.hasUpdate && !packageInfo.isProtected
            visible: packageInfo !== null
            onClicked: drawer.updateRequested(packageInfo)

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

        Button {
            Layout.fillWidth: true
            text: "Uninstall"
            enabled: packageInfo && !packageInfo.isProtected
            visible: packageInfo !== null
            onClicked: drawer.uninstallRequested(packageInfo)

            background: Rectangle {
                radius: Theme.radius
                border.color: parent.enabled ? Theme.danger : Theme.border
                color: parent.enabled ? (parent.hovered ? Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.25) : Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.12)) : "transparent"
            }
            contentItem: Text {
                text: parent.text
                color: parent.enabled ? Theme.danger : Theme.textDim
                font.pixelSize: 13
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }
    }
}
