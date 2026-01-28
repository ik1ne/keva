'use strict';

export const Api = {
    isMacOS: navigator.platform.includes('Mac'),

    send: function (msg) {
        window.chrome.webview.postMessage(JSON.stringify(msg));
    }
};
