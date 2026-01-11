'use strict';

// Based on https://github.com/ndp-software/keyboard-event-to-string (MIT license)
// Adapted for Keva: stores e.code format, displays human-readable format

var KeyboardShortcut = (function () {
    var defaultOptions = {
        alt: 'Alt',
        cmd: 'Win',
        ctrl: 'Ctrl',
        shift: 'Shift',
        joinWith: ' + ',
        hideKey: 'alphanumeric'
    };

    var gOptions = defaultOptions;

    var hideKeyRegExp = function () {
        return {
            'alphanumeric': /^Key([A-Z0-9])$/,
            'alpha': /^Key([A-Z])$/,
            'always': /^Key(.*)$/,
            'never': /^(.*)$/
        }[gOptions.hideKey];
    };

    function buildKeyMap(e) {
        var isOnlyModifier = [16, 17, 18, 91, 93, 224].indexOf(e.keyCode) !== -1;
        var character = isOnlyModifier ? null : e.code.replace(hideKeyRegExp(), '$1');

        return {
            character: character,
            modifiers: {
                cmd: e.metaKey,
                ctrl: e.ctrlKey,
                alt: e.altKey,
                shift: e.shiftKey
            }
        };
    }

    function buildKeyArray(e) {
        var map = buildKeyMap(e);
        var result = [];

        if (map.modifiers.ctrl) result.push(gOptions.ctrl);
        if (map.modifiers.alt) result.push(gOptions.alt);
        if (map.modifiers.shift) result.push(gOptions.shift);
        if (map.modifiers.cmd) result.push(gOptions.cmd);

        if (map.character) result.push(map.character);

        return result;
    }

    function details(e) {
        var map = buildKeyMap(e);
        var hasModifier = map.modifiers.ctrl || map.modifiers.alt ||
                          map.modifiers.shift || map.modifiers.cmd;

        return {
            hasKey: map.character != null,
            hasModifier: hasModifier,
            map: map
        };
    }

    function setOptions(userOptions) {
        gOptions = Object.assign({}, defaultOptions, userOptions);
        return gOptions;
    }

    function toString(e) {
        return buildKeyArray(e).join(gOptions.joinWith);
    }

    // === Keva-specific extensions ===

    function isModifierKey(code) {
        return [
            'ControlLeft', 'ControlRight',
            'AltLeft', 'AltRight',
            'ShiftLeft', 'ShiftRight',
            'MetaLeft', 'MetaRight',
            'OSLeft', 'OSRight'
        ].indexOf(code) !== -1;
    }

    function hasRequiredModifier(e) {
        return e.ctrlKey || e.altKey;
    }

    // Converts KeyboardEvent to storage format: "Ctrl+Alt+KeyA"
    function fromEvent(e) {
        if (isModifierKey(e.code)) {
            return null;
        }

        var parts = [];
        if (e.ctrlKey) parts.push('Ctrl');
        if (e.altKey) parts.push('Alt');
        if (e.shiftKey) parts.push('Shift');
        if (e.metaKey) parts.push('Win');
        parts.push(e.code);

        return parts.join('+');
    }

    // Converts storage format to display format: "Ctrl+Alt+KeyA" → "Ctrl + Alt + A"
    function toDisplay(shortcut) {
        if (!shortcut) return '';

        return shortcut.split('+').map(function (part) {
            // Modifiers pass through
            if (['Ctrl', 'Alt', 'Shift', 'Win'].indexOf(part) !== -1) {
                return part;
            }
            // Apply hideKey transformation for display
            return part.replace(hideKeyRegExp(), '$1');
        }).join(gOptions.joinWith);
    }

    return {
        details: details,
        setOptions: setOptions,
        toString: toString,
        isModifierKey: isModifierKey,
        hasRequiredModifier: hasRequiredModifier,
        fromEvent: fromEvent,
        toDisplay: toDisplay
    };
})();
