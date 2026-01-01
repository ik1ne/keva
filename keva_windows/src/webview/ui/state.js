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
        return this.data.keys.some(function (k) {
                return k.name === key;
            })
            || this.data.trashedKeys.some(function (k) {
                return k.name === key;
            });
    }
};
