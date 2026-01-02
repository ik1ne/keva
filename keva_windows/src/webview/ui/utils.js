'use strict';

const Utils = {
    escapeHtml: function (str) {
        return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    },

    escapeJs: function (str) {
        return str.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
    }
};
