import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.plasmoid
import org.kde.plasma.plasma5support as P5Support

import "../code/grid.js" as Grid

// Recent unread updates grid. The command line `kurobayomi --updates-json` decides what is shown (including the
// app-lock rule); this file only draws it. No configuration UI. Colours come from Kirigami.Theme.
PlasmoidItem {
    id: root

    // the manga and the anime package share this file; the package id names the kind
    readonly property string kind: Plasmoid.pluginName.indexOf(".anime") >= 0 ? "anime" : "manga"

    property var items: []
    property bool loaded: false
    property bool failed: false
    property string errorLine: ""
    readonly property bool empty: loaded && !failed && items.length === 0

    readonly property var size: Grid.rowsCols(width, height, 0, 0)
    property int requestedRows: size.rows
    property int requestedCols: size.cols

    preferredRepresentation: fullRepresentation

    // 60 s poll, run once at start
    Timer {
        interval: 60000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    // one call 300 ms after the last size change
    Timer {
        id: resizeDebounce
        interval: 300
        onTriggered: root.refresh()
    }
    onWidthChanged: resizeDebounce.restart()
    onHeightChanged: resizeDebounce.restart()

    // the feed command is killed after 6 s
    Timer {
        id: killer
        interval: 6000
        onTriggered: {
            if (executable.pending !== "") {
                executable.disconnectSource(executable.pending)
                executable.pending = ""
                root.fail("timed out after 6 s")
            }
        }
    }

    P5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []
        property string pending: ""

        onNewData: function (source, data) {
            disconnectSource(source)
            if (source !== pending)
                return
            pending = ""
            killer.stop()
            var stdout = (data["stdout"] || "").trim()
            var stderr = (data["stderr"] || "").trim()
            var code = data["exit code"]
            if (code !== 0) {
                root.fail(stderr || "exit code " + code)
                return
            }
            try {
                var payload = JSON.parse(stdout)
                root.items = payload.items
                root.failed = false
                root.loaded = true
            } catch (e) {
                root.fail("bad output: " + e)
            }
        }
    }

    function refresh() {
        // a command still running is replaced
        if (executable.pending !== "")
            executable.disconnectSource(executable.pending)
        // the trailing comment makes each run a new source name, so the engine runs it again
        executable.pending = Grid.feedCommand(kind, requestedRows, requestedCols) + " #" + Date.now()
        executable.connectSource(executable.pending)
        killer.restart()
    }

    function fail(line) {
        console.warn("kurobayomi updates (" + kind + "): " + line)
        errorLine = line
        failed = true
        loaded = true
        items = []
    }

    function run(command) {
        executable.connectSource(command + " #" + Date.now())
    }

    fullRepresentation: Item {
        Layout.minimumWidth: Grid.CELL_W
        Layout.minimumHeight: Grid.CELL_H
        Layout.preferredWidth: Grid.CELL_W * 4
        Layout.preferredHeight: Grid.CELL_H * 2

        QQC2.BusyIndicator {
            anchors.centerIn: parent
            running: visible
            visible: !root.loaded
        }

        // header: only in the error and empty states; a click refreshes at once
        MouseArea {
            id: header
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: label.implicitHeight + Kirigami.Units.smallSpacing * 2
            visible: root.failed || root.empty
            cursorShape: Qt.PointingHandCursor
            onClicked: root.refresh()

            QQC2.Label {
                id: label
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.largeSpacing * 2
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: Kirigami.Theme.textColor
                text: root.failed ? i18nd("kurobayomi", "Updates unavailable") : i18nd("kurobayomi", "No recent updates")
            }

            QQC2.ToolTip {
                text: root.errorLine
                visible: header.containsMouse && root.failed
            }
            hoverEnabled: true
        }

        // the grid is centred, and so is a short last row
        Column {
            anchors.centerIn: parent
            visible: !root.failed && root.items.length > 0

            Repeater {
                model: Grid.chunk(root.items, root.requestedCols)

                Row {
                    anchors.horizontalCenter: parent.horizontalCenter

                    Repeater {
                        model: modelData

                        CoverTile {
                            required property var modelData
                            title: modelData.title
                            coverPath: modelData.cover_path
                            onActivated: root.run(Grid.openCommand(root.kind, modelData.entry_id))
                        }
                    }
                }
            }
        }
    }
}
