'use strict';

import { Api } from './api.js';
import { showToast } from './toast.js';
import { isModifierKey, hasRequiredModifier, fromEvent, toDisplay } from './keyboard-shortcut.js';

export const Settings = {
    overlay: null,
    originalConfig: null,
    launchAtLogin: false,
    isCapturingHotkey: false,
    handledEscape: false,

    init: function () {
        this.overlay = document.getElementById('settings-overlay');
        this.setupEventHandlers();
    },

    setupEventHandlers: function () {
        const self = this;

        // Category navigation
        const navItems = document.querySelectorAll('.settings-nav-item');
        navItems.forEach(function (item) {
            item.addEventListener('click', function () {
                self.switchCategory(item.dataset.category);
            });
        });

        // Close button
        document.getElementById('settings-close').addEventListener('click', function () {
            self.close();
        });

        // Cancel button
        document.getElementById('settings-cancel').addEventListener('click', function () {
            self.close();
        });

        // Save button
        document.getElementById('settings-save').addEventListener('click', function () {
            self.save();
        });

        // Segmented control (theme selector)
        this.setupSegmentedControl('setting-theme');

        // Hotkey input capture
        const hotkeyInput = document.getElementById('setting-global-shortcut');
        const hotkeyClear = document.getElementById('setting-global-shortcut-clear');

        hotkeyInput.addEventListener('focus', function () {
            self.isCapturingHotkey = true;
            hotkeyInput.placeholder = 'Press key combination...';
        });

        hotkeyInput.addEventListener('blur', function () {
            self.isCapturingHotkey = false;
            hotkeyInput.placeholder = 'Click to set shortcut';
        });

        hotkeyClear.addEventListener('click', function () {
            hotkeyInput.value = '';
            hotkeyInput.dataset.shortcut = '';
            hotkeyClear.disabled = true;
        });

        hotkeyInput.addEventListener('keydown', function (e) {
            if (!self.isCapturingHotkey) return;

            e.preventDefault();
            e.stopPropagation();

            // Ignore modifier-only presses
            if (isModifierKey(e.code)) {
                return;
            }

            // Require Ctrl or Alt
            if (!hasRequiredModifier(e)) {
                showToast('Shortcut must include Ctrl or Alt');
                return;
            }

            // Build storage format (e.code based)
            const shortcut = fromEvent(e);
            if (!shortcut) {
                showToast('Unsupported key');
                return;
            }

            // Store e.code format in data attribute, display human-readable in value
            hotkeyInput.dataset.shortcut = shortcut;
            hotkeyInput.value = toDisplay(shortcut);
            hotkeyClear.disabled = false;
            hotkeyInput.blur();
        });

        // Global keyboard handler for settings panel (Escape to close, Enter to save)
        document.addEventListener('keydown', function (e) {
            // Only handle when settings panel is visible
            if (self.overlay.classList.contains('hidden')) return;
            // Don't handle when capturing hotkey
            if (self.isCapturingHotkey) return;

            if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                self.handledEscape = true;
                setTimeout(function () { self.handledEscape = false; }, 0);
                self.close();
            } else if (e.key === 'Enter') {
                e.preventDefault();
                e.stopPropagation();
                self.save();
            }
        });

        // Lifecycle inputs: only allow positive integers (max 1000 years)
        ['setting-trash-ttl', 'setting-purge-ttl'].forEach(function (id) {
            const input = document.getElementById(id);
            input.addEventListener('input', function () {
                // Remove non-digits and leading zeros
                let value = input.value.replace(/\D/g, '').replace(/^0+/, '');
                if (value !== '') {
                    const num = parseInt(value, 10);
                    if (num > 365000) {
                        value = '365000';
                        showToast("Thanks for planning to use Keva for 1000+ years, but I won't be around that long to maintain it!");
                    }
                }
                input.value = value;
            });
        });

        // Click outside panel to close (only if no changes)
        this.overlay.addEventListener('click', function (e) {
            if (e.target === self.overlay && !self.hasChanges()) {
                self.close();
            }
        });
    },

    hasChanges: function () {
        const values = this.getFormValues();
        const orig = this.originalConfig;
        return values.config.general.theme !== orig.general.theme ||
            values.config.general.show_tray_icon !== orig.general.show_tray_icon ||
            values.config.shortcuts.global_shortcut !== orig.shortcuts.global_shortcut ||
            values.config.lifecycle.trash_ttl_days !== orig.lifecycle.trash_ttl_days ||
            values.config.lifecycle.purge_ttl_days !== orig.lifecycle.purge_ttl_days ||
            values.launchAtLogin !== this.launchAtLogin;
    },

    setupSegmentedControl: function (id) {
        const control = document.getElementById(id);
        const segments = control.querySelectorAll('.segment');

        segments.forEach(function (segment) {
            segment.addEventListener('click', function () {
                const value = segment.dataset.value;
                control.dataset.value = value;

                segments.forEach(function (s) {
                    s.classList.toggle('active', s === segment);
                });
            });
        });
    },

    switchCategory: function (category) {
        // Update nav items
        const navItems = document.querySelectorAll('.settings-nav-item');
        navItems.forEach(function (item) {
            if (item.dataset.category === category) {
                item.classList.add('active');
            } else {
                item.classList.remove('active');
            }
        });

        // Update category panels
        const categories = ['general', 'shortcuts', 'lifecycle'];
        categories.forEach(function (cat) {
            const panel = document.getElementById('settings-' + cat);
            if (cat === category) {
                panel.classList.remove('hidden');
            } else {
                panel.classList.add('hidden');
            }
        });
    },

    open: function (config, launchAtLogin) {
        this.originalConfig = config;
        this.launchAtLogin = launchAtLogin;
        this.populateForm(config, launchAtLogin);
        this.switchCategory('general');
        this.overlay.classList.remove('hidden');
        // Focus the save button to capture keyboard input
        document.getElementById('settings-save').focus();
    },

    close: function () {
        this.overlay.classList.add('hidden');
        this.isCapturingHotkey = false;
    },

    populateForm: function (config, launchAtLogin) {
        // General - Theme (segmented control)
        const themeControl = document.getElementById('setting-theme');
        themeControl.dataset.value = config.general.theme;
        themeControl.querySelectorAll('.segment').forEach(function (s) {
            s.classList.toggle('active', s.dataset.value === config.general.theme);
        });

        document.getElementById('setting-launch-at-login').checked = launchAtLogin;
        document.getElementById('setting-show-tray-icon').checked = config.general.show_tray_icon;

        // Shortcuts - store e.code format, display human-readable
        const hotkeyInput = document.getElementById('setting-global-shortcut');
        const shortcutValue = config.shortcuts.global_shortcut;
        hotkeyInput.dataset.shortcut = shortcutValue;
        hotkeyInput.value = toDisplay(shortcutValue);
        document.getElementById('setting-global-shortcut-clear').disabled = !shortcutValue;

        // Lifecycle
        document.getElementById('setting-trash-ttl').value = config.lifecycle.trash_ttl_days;
        document.getElementById('setting-purge-ttl').value = config.lifecycle.purge_ttl_days;
    },

    getFormValues: function () {
        // Get shortcut from data attribute (e.code format)
        const shortcut = document.getElementById('setting-global-shortcut').dataset.shortcut || '';

        return {
            config: {
                general: {
                    theme: document.getElementById('setting-theme').dataset.value,
                    show_tray_icon: document.getElementById('setting-show-tray-icon').checked,
                },
                shortcuts: {
                    global_shortcut: shortcut,
                },
                lifecycle: {
                    trash_ttl_days: parseInt(document.getElementById('setting-trash-ttl').value, 10) || 30,
                    purge_ttl_days: parseInt(document.getElementById('setting-purge-ttl').value, 10) || 7,
                },
            },
            launchAtLogin: document.getElementById('setting-launch-at-login').checked,
        };
    },

    validate: function (config) {
        const errors = [];

        if (config.lifecycle.trash_ttl_days < 1 || config.lifecycle.trash_ttl_days > 365000) {
            errors.push('Trash TTL must be between 1 and 365000 days');
        }

        if (config.lifecycle.purge_ttl_days < 1 || config.lifecycle.purge_ttl_days > 365000) {
            errors.push('Purge TTL must be between 1 and 365000 days');
        }

        return errors;
    },

    save: function () {
        const values = this.getFormValues();
        const errors = this.validate(values.config);

        if (errors.length > 0) {
            alert('Invalid settings:\n\n' + errors.join('\n'));
            return;
        }

        Api.send({
            type: 'saveSettings',
            config: values.config,
            launchAtLogin: values.launchAtLogin,
        });
        this.close();
    },
};
