// Counter button with a label
import QtQuick 2.15
import QtQuick.Controls 2.15 as Controls

Rectangle {
    id: root
    width: 320; height: 200
    color: "#f0f0f0"

    property int count: 0
    property string title: "Clicks: " + count
    readonly property real ratio: width / height
    signal reset(int previous)

    function increment(step) {
        count += step || 1;
        if (count > 10) {
            console.log(`Too many clicks: ${count}`);
            root.reset(count);
        }
    }

    Text {
        id: label
        anchors.centerIn: parent
        text: root.title
        font.pixelSize: 24
        color: count > 5 ? "red" : 'black'
    }

    Controls.Button {
        anchors.bottom: parent.bottom
        text: qsTr("Click me")
        onClicked: root.increment(1)
    }

    onReset: function(previous) {
        count = 0;
        Qt.callLater(() => label.text = "reset from " + previous);
    }

    /* animation on the color */
    Behavior on color { ColorAnimation { duration: 250 } }
}
