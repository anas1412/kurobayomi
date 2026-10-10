import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// One cover cell: 58 x 87 cover inside a 64 x 95 cell. Focusable with Tab, activated with Enter and Space,
// accessible name = entry title, tooltip after 500 ms of hover, pointing-hand cursor.
Item {
    id: tile

    property string title: ""
    property var coverPath: null

    signal activated()

    width: 64
    height: 95
    activeFocusOnTab: true

    Accessible.role: Accessible.Button
    Accessible.name: title
    Accessible.onPressAction: tile.activated()

    Keys.onReturnPressed: tile.activated()
    Keys.onEnterPressed: tile.activated()
    Keys.onSpacePressed: tile.activated()

    Rectangle {
        id: cover
        anchors.centerIn: parent
        width: 58
        height: 87

        // the placeholder fill is the only fixed colour of the applet
        color: "#1F888888"
        radius: 12

        Kirigami.Icon {
            anchors.centerIn: parent
            width: 10
            height: 10
            source: "image-x-generic"
            visible: image.status !== Image.Ready
        }

        // a null path, a failed load and a missing file all keep the placeholder
        Image {
            id: image
            anchors.fill: parent
            source: tile.coverPath ? "file://" + tile.coverPath : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            visible: status === Image.Ready
        }

        Rectangle {
            anchors.fill: parent
            radius: cover.radius
            color: "transparent"
            border.width: 2
            border.color: Kirigami.Theme.highlightColor
            visible: tile.activeFocus
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            tile.forceActiveFocus()
            tile.activated()
        }
    }

    QQC2.ToolTip {
        delay: 500
        text: tile.title
        visible: mouse.containsMouse && tile.title.length > 0
    }
}
