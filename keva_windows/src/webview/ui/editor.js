'use strict';

const Editor = {
    instance: null,
    saveTimer: null,
    dom: null,
    currentHandle: null,
    currentKey: null,
    isReadOnly: false,
    isSaving: false,
    isLargeFile: false,
    placeholderElement: null,

    init: function (dom, callback) {
        this.dom = dom;
        const self = this;

        // Create placeholder element
        this.placeholderElement = document.createElement('div');
        this.placeholderElement.className = 'editor-placeholder';
        this.placeholderElement.textContent = 'Type something, or drag files here...';
        this.placeholderElement.style.display = 'none';
        dom.editorContainer.appendChild(this.placeholderElement);

        require.config({paths: {'vs': 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs'}});
        require(['vs/editor/editor.main'], function () {
            self.instance = monaco.editor.create(dom.editorContainer, {
                value: '',
                language: 'markdown',
                theme: 'vs-dark',
                automaticLayout: true,
                minimap: {enabled: false},
                fontSize: 14,
                wordWrap: 'on',
                scrollBeyondLastLine: false,
                readOnlyMessage: {value: 'Restore from trash to edit', isTrusted: true}
            });

            self.instance.onDidChangeModelContent(function () {
                if (State.data.selectedKey && !self.isReadOnly) {
                    State.data.isDirty = true;
                    self.scheduleSave();
                }
                self.updatePlaceholder();
            });

            self.instance.onDidFocusEditorWidget(function () {
                Main.setActivePane('editor');
            });

            if (callback) callback();
        });
    },

    updatePlaceholder: function () {
        if (!this.placeholderElement || !this.instance) return;
        const isEmpty = this.instance.getValue() === '';
        const shouldShow = isEmpty && !this.isReadOnly;
        this.placeholderElement.style.display = shouldShow ? 'block' : 'none';

        if (shouldShow) {
            const layout = this.instance.getLayoutInfo();
            this.placeholderElement.style.left = layout.contentLeft + 'px';
            this.placeholderElement.style.top = this.instance.getTopForLineNumber(1) + 'px';
        }
    },

    showWithHandle: async function (handle, key, readOnly) {
        this.currentHandle = handle;
        this.currentKey = key;
        this.isReadOnly = readOnly;

        this.dom.emptyState.style.display = 'none';
        this.dom.editorContainer.style.display = 'block';
        this.hideError();

        if (!this.instance) return;

        try {
            const file = await handle.getFile();
            const content = await file.text();
            this.instance.setValue(content);
            this.applyFileSizeOptions(content.length);
            this.instance.updateOptions({readOnly: readOnly});
            State.data.isDirty = false;
            this.updatePlaceholder();

            if (State.data.focusEditorOnLoad) {
                this.instance.focus();
                State.data.focusEditorOnLoad = false;
            }
        } catch (error) {
            console.error('[Editor] Failed to read file:', error);
            this.showError('Failed to read content');
        }
    },

    writeToHandle: async function () {
        if (!this.currentHandle || !this.instance || this.isReadOnly) {
            return false;
        }

        if (this.isSaving) {
            return false;
        }

        this.isSaving = true;
        try {
            const content = this.instance.getValue();
            const writable = await this.currentHandle.createWritable();
            await writable.write(content);
            await writable.close();

            // Update timestamp
            if (this.currentKey) {
                Api.send({type: 'touch', key: this.currentKey});
            }
            return true;
        } catch (error) {
            console.error('[Editor] Failed to write file:', error);
            this.showWriteError();
            return false;
        } finally {
            this.isSaving = false;
        }
    },

    scheduleSave: function () {
        const self = this;
        if (this.saveTimer) clearTimeout(this.saveTimer);
        this.saveTimer = setTimeout(async function () {
            if (State.data.isDirty && State.data.selectedKey && self.currentHandle) {
                const success = await self.writeToHandle();
                if (success) {
                    State.data.isDirty = false;
                }
            }
        }, 500);
    },

    forceSave: async function () {
        if (this.saveTimer) clearTimeout(this.saveTimer);
        if (State.data.isDirty && State.data.selectedKey && this.currentHandle) {
            const success = await this.writeToHandle();
            if (success) {
                State.data.isDirty = false;
            }
        }
    },

    show: function (content) {
        this.dom.emptyState.style.display = 'none';
        this.dom.editorContainer.style.display = 'block';
        this.hideError();
        if (this.instance) {
            this.instance.setValue(content || '');
            this.instance.updateOptions({readOnly: State.data.isSelectedTrashed});
            State.data.isDirty = false;
        }
    },

    showEmpty: function (message) {
        this.currentHandle = null;
        this.currentKey = null;
        this.dom.emptyState.textContent = message || 'Select a key or type to search';
        this.dom.emptyState.style.display = 'flex';
        this.dom.editorContainer.style.display = 'none';
        this.hideError();
    },

    showError: function (message) {
        this.dom.emptyState.textContent = message;
        this.dom.emptyState.style.display = 'flex';
        this.dom.editorContainer.style.display = 'none';
    },

    showWriteError: function () {
        // Show warning banner but keep editor visible so user can copy content
        if (!this.dom.writeError) {
            const banner = document.createElement('div');
            banner.id = 'write-error';
            banner.className = 'write-error-banner';
            banner.innerHTML = '<span>Failed to save. </span><button onclick="Editor.retrySave()">Retry</button>';
            this.dom.editorContainer.parentNode.insertBefore(banner, this.dom.editorContainer);
            this.dom.writeError = banner;
        }
        this.dom.writeError.style.display = 'flex';
    },

    hideError: function () {
        if (this.dom.writeError) {
            this.dom.writeError.style.display = 'none';
        }
    },

    retrySave: async function () {
        this.hideError();
        State.data.isDirty = true;
        const success = await this.writeToHandle();
        if (success) {
            State.data.isDirty = false;
        }
    },

    applyFileSizeOptions: function (contentLength) {
        const isLarge = contentLength > 1024 * 1024;

        if (isLarge && !this.isLargeFile) {
            // Switching to large file mode
            this.instance.updateOptions({
                largeFileOptimizations: true,
                maxTokenizationLineLength: 500,
                wordWrap: 'off',
                bracketPairColorization: {enabled: false},
                matchBrackets: 'never',
                renderWhitespace: 'none',
                renderLineHighlight: 'none',
                folding: false,
                links: false,
                hover: {enabled: false},
                quickSuggestions: false,
                wordBasedSuggestions: 'off',
                glyphMargin: false
            });
            monaco.editor.setModelLanguage(this.instance.getModel(), 'plaintext');
        } else if (!isLarge && this.isLargeFile) {
            // Switching back to normal mode
            this.instance.updateOptions({
                largeFileOptimizations: false,
                maxTokenizationLineLength: 20000,
                wordWrap: 'on',
                bracketPairColorization: {enabled: true},
                matchBrackets: 'always',
                renderWhitespace: 'selection',
                renderLineHighlight: 'line',
                folding: true,
                links: true,
                hover: {enabled: true},
                quickSuggestions: true,
                wordBasedSuggestions: 'currentDocument',
                glyphMargin: true
            });
            monaco.editor.setModelLanguage(this.instance.getModel(), 'markdown');
        }

        this.isLargeFile = isLarge;
    },

    applyTheme: function (theme) {
        State.data.currentTheme = theme;
        document.documentElement.setAttribute('data-theme', theme);
        if (this.instance) {
            monaco.editor.setTheme(theme === 'light' ? 'vs' : 'vs-dark');
        }
    }
};
