pragma Singleton

import QtQuick 2.6
import Sailfish.Silica 1.0

QtObject {
    id: root

    readonly property bool _isAppActive: Qt.application.state === Qt.ApplicationActive

    property date nowMinutes: new Date()
    property date nowHours: new Date()

    on_IsAppActiveChanged: {
        if (_isAppActive) {
            var current = new Date()
            nowMinutes = current
            nowHours = current
        }
    }

    property Timer _minTimer: Timer {
        interval: 60 * 1000
        running: root._isAppActive
        repeat: true
        onTriggered: root.nowMinutes = new Date()
    }

    property Timer _hourTimer: Timer {
        interval: 3600 * 1000
        running: root._isAppActive
        repeat: true
        onTriggered: root.nowHours = new Date()
    }

    function formatDate(inputDate, formatter) {
        if (!inputDate) return ""

        var initialDiffSec = Math.floor((Date.now() - inputDate.getTime()) / 1000)

        if (initialDiffSec < 3600) {
            void root.nowMinutes
        } else if (initialDiffSec < 86400) {
            void root.nowHours
        }

        return Format.formatDate(inputDate, formatter)
    }
}
