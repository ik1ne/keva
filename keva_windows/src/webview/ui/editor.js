'use strict';

const Editor = {
    instance: null,
    saveTimer: null,
    dom: null,

    init: function (dom, callback) {
        this.dom = dom;
        const self = this;

        require.config({paths: {'vs': 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs'}});
        require(['vs/editor/editor.main'], function () {
            self.instance = monaco.editor.create(dom.editorContainer, {
                value: '',
                language: 'plaintext',
                theme: 'vs-dark',
                automaticLayout: true,
                minimap: {enabled: false},
                fontSize: 14,
                wordWrap: 'on',
                scrollBeyondLastLine: false
            });

            self.instance.onDidChangeModelContent(function () {
                if (State.data.selectedKey) {
                    State.data.isDirty = true;
                    self.scheduleSave();
                }
            });

            if (callback) callback();
        });
    },

    scheduleSave: function () {
        const self = this;
        if (this.saveTimer) clearTimeout(this.saveTimer);
        this.saveTimer = setTimeout(function () {
            if (State.data.isDirty && State.data.selectedKey && self.instance) {
                Api.send({
                    type: 'save',
                    key: State.data.selectedKey,
                    content: self.instance.getValue()
                });
                State.data.isDirty = false;
            }
        }, 500);
    },

    forceSave: function () {
        if (this.saveTimer) clearTimeout(this.saveTimer);
        if (State.data.isDirty && State.data.selectedKey && this.instance) {
            Api.send({
                type: 'save',
                key: State.data.selectedKey,
                content: this.instance.getValue()
            });
            State.data.isDirty = false;
        }
    },

    show: function (content) {
        this.dom.emptyState.style.display = 'none';
        this.dom.editorContainer.style.display = 'block';
        if (this.instance) {
            this.instance.setValue(content || '');
            this.instance.updateOptions({readOnly: State.data.isSelectedTrashed});
            State.data.isDirty = false;
            this.instance.focus();
        }
    },

    showEmpty: function (message) {
        this.dom.emptyState.textContent = message || 'Select a key or type to search';
        this.dom.emptyState.style.display = 'flex';
        this.dom.editorContainer.style.display = 'none';
    },

    applyTheme: function (theme) {
        State.data.currentTheme = theme;
        document.documentElement.setAttribute('data-theme', theme);
        if (this.instance) {
            monaco.editor.setTheme(theme === 'light' ? 'vs' : 'vs-dark');
        }
    }
};
