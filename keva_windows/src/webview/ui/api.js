'use strict';

const Api = {
    send: function (msg) {
        window.chrome.webview.postMessage(JSON.stringify(msg));
    }
};
