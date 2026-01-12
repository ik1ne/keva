'use strict';

import { Api } from './api.js';

export const Welcome = {
    overlay: null,

    init: function () {
        this.overlay = document.getElementById('welcome-overlay');
        this.setupEventHandlers();
    },

    setupEventHandlers: function () {
        const self = this;

        document.getElementById('welcome-get-started').addEventListener('click', function () {
            self.submit();
        });

        // Keyboard: Enter to submit
        this.overlay.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                self.submit();
            }
        });
    },

    show: function () {
        this.overlay.classList.remove('hidden');
        document.getElementById('welcome-get-started').focus();
    },

    submit: function () {
        const launchAtLogin = document.getElementById('welcome-launch-at-login').checked;

        Api.send({
            type: 'welcomeResult',
            launchAtLogin: launchAtLogin,
        });

        this.overlay.classList.add('hidden');
    },
};
