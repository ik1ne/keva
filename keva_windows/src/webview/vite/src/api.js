'use strict';

export const Api = {
    send: function (msg) {
        window.chrome.webview.postMessage(JSON.stringify(msg));
    }
};
