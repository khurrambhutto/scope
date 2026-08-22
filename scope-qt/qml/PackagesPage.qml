import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: page
    color: Theme.background

    property string selectedKey: ""
    property var selectedPackage: null

    component EmptyState: ColumnLayout {
        spacing: 8
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: "▤"
            color: Theme.border
            font.pixelSize: 40
        }
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: Packages.scanning ? "Scanning…" : (Packages.packages.totalCount === 0 ? "No packages detected" : "No matches for this filter")
            color: Theme.textDim
            font.pixelSize: 13
        }
        Text {
            Layout.alignment: Qt.AlignHCenter
            visible: !Packages.scanning && Packages.packages.totalCount === 0
            text: "Try Refresh, or install apt/snap/flatpak packages"
            color: Theme.textDim
            font.pixelSize: 11
        }
    }

    function selectAt(row) {
        const pkg = packagesModel.packageData(row)
        if (!pkg || Object.keys(pkg).length === 0) {
            selectedPackage = null
            selectedKey = ""
            return
        }
        selectedKey = pkg.key
        selectedPackage = pkg
    }

    // model alias usable from delegates and functions
    readonly property var packagesModel: Packages.packages

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // List column
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 20
            spacing: 14

            // Header
            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                ColumnLayout {
                    spacing: 2
                    Text {
                        text: "Packages"
                        color: Theme.text
                        font.pixelSize: 22
                        font.bold: true
                    }
                    Text {
                        text: {
                            const total = Packages.packages.totalCount
                            const updates = Packages.updateCount
                            if (Packages.scanning)
                                return "Scanning installed apps…"
                            return total + " installed · " + updates + " update" + (updates === 1 ? "" : "s") + " available"
                        }
                        color: Theme.textDim
                        font.pixelSize: 12
                    }
                }

                Item { Layout.fillWidth: true }

                Button {
                    text: "Refresh"
                    enabled: !Packages.scanning
                    onClicked: Packages.refresh()

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

            // Toolbar
            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Rectangle {
                    Layout.preferredWidth: 320
                    Layout.preferredHeight: 36
                    radius: Theme.radius
                    color: Theme.surface
                    border.color: searchField.activeFocus ? Theme.accent : Theme.border

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 6

                        Text { text: "⌕"; color: Theme.textDim; font.pixelSize: 15 }
                        TextField {
                            id: searchField
                            Layout.fillWidth: true
                            placeholderText: "Search packages…"
                            color: Theme.text
                            placeholderTextColor: Theme.textDim
                            background: null
                            font.pixelSize: 13
                            onTextChanged: Packages.setQuery(text)
                        }
                    }
                }

                ComboBox {
                    id: sourceFilter
                    Layout.preferredHeight: 36
                    model: [
                        { value: "", label: "All sources" },
                        { value: "apt", label: "APT" },
                        { value: "snap", label: "Snap" },
                        { value: "flatpak", label: "Flatpak" },
                        { value: "appimage", label: "AppImage" },
                        { value: "manual", label: "Manual" }
                    ]
                    textRole: "label"
                    valueRole: "value"
                    font.pixelSize: 13
                    onActivated: Packages.setSourceFilter(currentValue)
                }

                ComboBox {
                    id: kindFilter
                    Layout.preferredHeight: 36
                    model: [
                        { value: "", label: "All types" },
                        { value: "gui", label: "GUI apps" },
                        { value: "cli", label: "CLI tools" },
                        { value: "unknown", label: "Unclassified" }
                    ]
                    textRole: "label"
                    valueRole: "value"
                    font.pixelSize: 13
                    onActivated: Packages.setKindFilter(currentValue)
                }

                Item { Layout.fillWidth: true }
            }

            // List
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Theme.radius
                color: Theme.surface
                border.color: Theme.border

                ListView {
                    id: listView
                    anchors.fill: parent
                    anchors.margins: 6
                    clip: true
                    spacing: 2
                    model: Packages.packages

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                    }

                    delegate: PackageRow {
                        width: listView.width
                        selected: model.key === page.selectedKey
                        onSelectRequested: (row) => page.selectAt(row)
                    }

                    EmptyState {
                        anchors.centerIn: parent
                        visible: listView.count === 0 && !Packages.scanning
                    }

                    BusyIndicator {
                        anchors.centerIn: parent
                        running: Packages.scanning && listView.count === 0
                        visible: running
                    }
                }
            }
        }

        // Detail drawer
        DetailDrawer {
            id: detailDrawer
            Layout.preferredWidth: 330
            Layout.fillHeight: true
            Layout.topMargin: 20
            Layout.bottomMargin: 20
            Layout.rightMargin: 20
            packageInfo: page.selectedPackage
            visible: page.selectedPackage !== null

            onUninstallRequested: (pkg) => Operations.previewUninstall(pkg.key)
            onUpdateRequested: (pkg) => Operations.previewUpdate(pkg.key)
        }
    }
}
