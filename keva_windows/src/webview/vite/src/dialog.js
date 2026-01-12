'use strict';

export const Dialog = {
    show: function (options) {
        const overlay = document.createElement('div');
        overlay.className = 'dialog-overlay';

        const dialog = document.createElement('div');
        dialog.className = 'dialog';
        dialog.tabIndex = -1;

        const message = document.createElement('div');
        message.className = 'dialog-message';
        message.textContent = options.message;

        const buttonsContainer = document.createElement('div');
        buttonsContainer.className = 'dialog-buttons';

        const buttons = [];
        let focusBtn = null;

        for (let i = 0; i < options.buttons.length; i++) {
            const btnConfig = options.buttons[i];
            const btn = document.createElement('button');
            btn.className = 'dialog-btn';
            btn.textContent = btnConfig.label;

            if (btnConfig.primary) {
                btn.classList.add('dialog-btn-primary');
            } else {
                btn.classList.add('dialog-btn-secondary');
            }

            if (btnConfig.danger) {
                btn.style.background = '#f44336';
            }

            if (btnConfig.focus) {
                focusBtn = btn;
            } else if (btnConfig.primary && !focusBtn) {
                focusBtn = btn;
            }

            btn.onclick = function () {
                overlay.remove();
                if (options.onClose) {
                    options.onClose(btnConfig.action);
                }
            };

            buttons.push(btn);
            buttonsContainer.appendChild(btn);
        }

        dialog.appendChild(message);
        dialog.appendChild(buttonsContainer);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);

        // Focus: explicit focus > primary > first
        (focusBtn || buttons[0]).focus();

        // Keyboard handling
        overlay.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                e.stopPropagation();
                overlay.remove();
                if (options.onEscape) {
                    options.onEscape();
                } else if (options.onClose) {
                    options.onClose(null);
                }
            } else if (e.key === 'Tab') {
                e.preventDefault();
                const currentIndex = buttons.indexOf(document.activeElement);
                let nextIndex;
                if (e.shiftKey) {
                    nextIndex = currentIndex <= 0 ? buttons.length - 1 : currentIndex - 1;
                } else {
                    nextIndex = currentIndex >= buttons.length - 1 ? 0 : currentIndex + 1;
                }
                buttons[nextIndex].focus();
            }
        });

        // Restore keyboard focus without selecting a button
        overlay.addEventListener('click', function (e) {
            if (!e.target.closest('button')) {
                dialog.focus();
            }
        });

        return overlay;
    }
};
