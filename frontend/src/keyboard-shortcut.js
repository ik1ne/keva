'use strict';

import { toString, details, setOptions } from 'keyboard-event-to-string';

// Re-export original functions
export { toString, details, setOptions };

// Keva-specific extensions

const MODIFIER_KEYS = new Set([
    'ControlLeft', 'ControlRight',
    'AltLeft', 'AltRight',
    'ShiftLeft', 'ShiftRight',
    'MetaLeft', 'MetaRight',
    'OSLeft', 'OSRight'
]);

export function isModifierKey(code) {
    return MODIFIER_KEYS.has(code);
}

export function hasRequiredModifier(e) {
    return e.ctrlKey || e.altKey || e.metaKey || e.shiftKey;
}

// Converts KeyboardEvent to storage format: "Ctrl+Alt+KeyA"
export function fromEvent(e) {
    if (isModifierKey(e.code)) {
        return null;
    }

    const parts = [];
    if (e.ctrlKey) parts.push('Ctrl');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');
    if (e.metaKey) parts.push('Win');
    parts.push(e.code);

    return parts.join('+');
}

// Converts storage format to display format: "Ctrl+Alt+KeyA" → "Ctrl + Alt + A"
export function toDisplay(shortcut) {
    if (!shortcut) return '';

    const hideKeyRegExp = /^Key([A-Z0-9])$/;

    return shortcut.split('+').map(function (part) {
        // Modifiers pass through
        if (['Ctrl', 'Alt', 'Shift', 'Win'].includes(part)) {
            return part;
        }
        // Apply hideKey transformation for display
        return part.replace(hideKeyRegExp, '$1');
    }).join(' + ');
}
