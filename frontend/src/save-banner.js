'use strict';

// Inline error banner for save failures with retry functionality.
export const SaveBanner = {
    element: null,
    container: null,
    onRetry: null,

    init: function (container) {
        this.container = container;
    },

    show: function (message, onRetry) {
        if (!this.container) return;

        this.onRetry = onRetry || null;

        if (!this.element) {
            var banner = document.createElement('div');
            banner.className = 'save-error-banner';
            this.container.insertBefore(banner, this.container.firstChild);
            this.element = banner;
        }

        var text = message || 'Failed to save.';
        var retryHtml = this.onRetry ? ' <button onclick="SaveBanner.retry()">Retry</button>' : '';
        this.element.innerHTML = '<span>' + text + '</span>' + retryHtml;
        this.element.style.display = 'flex';
    },

    hide: function () {
        if (this.element) {
            this.element.style.display = 'none';
        }
    },

    retry: function () {
        this.hide();
        if (this.onRetry) {
            this.onRetry();
        }
    }
};

// Expose for onclick handler
window.SaveBanner = SaveBanner;
