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

        // Hotkey input capture for all shortcut fields
        this.setupHotkeyInput('setting-global-shortcut');
        this.setupHotkeyInput('setting-copy-markdown');
        this.setupHotkeyInput('setting-copy-html');
        this.setupHotkeyInput('setting-copy-files');

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
            values.config.shortcuts.copy_markdown !== orig.shortcuts.copy_markdown ||
            values.config.shortcuts.copy_html !== orig.shortcuts.copy_html ||
            values.config.shortcuts.copy_files !== orig.shortcuts.copy_files ||
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

    setupHotkeyInput: function (inputId) {
        const self = this;
        const hotkeyInput = document.getElementById(inputId);
        const hotkeyClear = document.getElementById(inputId + '-clear');
        const isGlobalShortcut = inputId === 'setting-global-shortcut';

        hotkeyInput.addEventListener('focus', function () {
            self.isCapturingHotkey = true;
            hotkeyInput.placeholder = 'Press key combination...';
            // Suspend global hotkey so we can capture it
            if (isGlobalShortcut) {
                Api.send({ type: 'suspendGlobalHotkey' });
            }
        });

        hotkeyInput.addEventListener('blur', function () {
            self.isCapturingHotkey = false;
            hotkeyInput.placeholder = 'Click to set shortcut';
            // Resume global hotkey
            if (isGlobalShortcut) {
                Api.send({ type: 'resumeGlobalHotkey' });
            }
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
    },

    populateHotkeyInput: function (inputId, value) {
        const hotkeyInput = document.getElementById(inputId);
        hotkeyInput.dataset.shortcut = value || '';
        hotkeyInput.value = toDisplay(value || '');
        document.getElementById(inputId + '-clear').disabled = !value;
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
        this.populateHotkeyInput('setting-global-shortcut', config.shortcuts.global_shortcut);
        this.populateHotkeyInput('setting-copy-markdown', config.shortcuts.copy_markdown);
        this.populateHotkeyInput('setting-copy-html', config.shortcuts.copy_html);
        this.populateHotkeyInput('setting-copy-files', config.shortcuts.copy_files);

        // Lifecycle
        document.getElementById('setting-trash-ttl').value = config.lifecycle.trash_ttl_days;
        document.getElementById('setting-purge-ttl').value = config.lifecycle.purge_ttl_days;
    },

    getFormValues: function () {
        // Start with original config to preserve non-editable fields (e.g., welcome_shown)
        const config = JSON.parse(JSON.stringify(this.originalConfig));

        // Update editable fields from form
        config.general.theme = document.getElementById('setting-theme').dataset.value;
        config.general.show_tray_icon = document.getElementById('setting-show-tray-icon').checked;
        config.shortcuts.global_shortcut = document.getElementById('setting-global-shortcut').dataset.shortcut || '';
        config.shortcuts.copy_markdown = document.getElementById('setting-copy-markdown').dataset.shortcut || '';
        config.shortcuts.copy_html = document.getElementById('setting-copy-html').dataset.shortcut || '';
        config.shortcuts.copy_files = document.getElementById('setting-copy-files').dataset.shortcut || '';
        config.lifecycle.trash_ttl_days = parseInt(document.getElementById('setting-trash-ttl').value, 10) || 30;
        config.lifecycle.purge_ttl_days = parseInt(document.getElementById('setting-purge-ttl').value, 10) || 7;

        return {
            config: config,
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

        // Check for shortcut conflicts (only non-empty shortcuts can conflict)
        const shortcuts = [
            { name: 'Global Shortcut', value: config.shortcuts.global_shortcut },
            { name: 'Copy Markdown', value: config.shortcuts.copy_markdown },
            { name: 'Copy HTML', value: config.shortcuts.copy_html },
            { name: 'Copy Files', value: config.shortcuts.copy_files },
        ].filter(s => s.value); // Only check non-empty shortcuts

        for (let i = 0; i < shortcuts.length; i++) {
            for (let j = i + 1; j < shortcuts.length; j++) {
                if (shortcuts[i].value === shortcuts[j].value) {
                    errors.push(shortcuts[i].name + ' and ' + shortcuts[j].name + ' have the same shortcut');
                }
            }
        }

        // Check against reserved shortcuts (hardcoded in native code)
        const reserved = [
            { name: 'Open Settings', value: 'Ctrl+Comma' },
        ];
        for (const shortcut of shortcuts) {
            for (const res of reserved) {
                if (shortcut.value === res.value) {
                    errors.push(shortcut.name + ' conflicts with ' + res.name + ' (' + res.value + ')');
                }
            }
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
