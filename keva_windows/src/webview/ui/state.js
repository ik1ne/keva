'use strict';

const State = {
    data: {
        keys: [],
        trashedKeys: [],
        exactMatch: 'none',
        selectedKey: null,
        isSelectedTrashed: false,
        isDirty: false,
        isShuttingDown: false,
        pendingSelectKey: null,
        currentTheme: 'dark',
    },

    setSelectedKey: function (key, isTrashed) {
        this.data.selectedKey = key;
        this.data.isSelectedTrashed = isTrashed;
    },

    clearSelection: function () {
        this.data.selectedKey = null;
        this.data.isSelectedTrashed = false;
    },

    isKeyVisible: function (key) {
        return this.data.keys.indexOf(key) !== -1
            || this.data.trashedKeys.indexOf(key) !== -1;
    }
};
