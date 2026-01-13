'use strict';

import * as monaco from 'monaco-editor';
import markdownit from 'markdown-it';
import DOMPurify from 'dompurify';
import hljs from 'highlight.js';

import { State } from './state.js';
import { Api } from './api.js';

// Main reference set later to avoid circular dependency
let Main = null;

export function setMainRef(main) {
    Main = main;
}

export const Editor = {
    instance: null,
    saveTimer: null,
    dom: null,
    currentHandle: null,
    currentKey: null,
    keyHash: null,
    isReadOnly: false,
    isSaving: false,
    isLargeFile: false,
    placeholderElement: null,
    previewCache: {html: null},
    editScrollTop: 0,
    previewScrollTop: 0,
    markdownIt: null,

    IMAGE_EXTENSIONS: new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico']),

    init: function (dom, callback) {
        this.dom = dom;
        const self = this;

        // Create placeholder element
        this.placeholderElement = document.createElement('div');
        this.placeholderElement.className = 'editor-placeholder';
        this.placeholderElement.textContent = 'Type something, or drag files here...';
        this.placeholderElement.style.display = 'none';
        dom.editorContainer.appendChild(this.placeholderElement);

        // Container for Monaco overflow widgets (popovers) outside clipped editor area.
        // Needs monaco-editor class for proper styling of widgets.
        this.overflowContainer = document.createElement('div');
        this.overflowContainer.className = 'monaco-editor';
        document.body.appendChild(this.overflowContainer);

        self.instance = monaco.editor.create(dom.editorContainer, {
            value: '',
            language: 'markdown',
            theme: 'vs-dark',
            automaticLayout: true,
            minimap: {enabled: false},
            fontSize: 14,
            wordWrap: 'on',
            scrollBeyondLastLine: false,
            readOnlyMessage: {value: 'Restore from trash to edit', isTrusted: true},
            dropIntoEditor: {enabled: false},
            fixedOverflowWidgets: true,
            overflowWidgetsDomNode: self.overflowContainer
        });

        self.instance.onDidChangeModelContent(function () {
            if (State.data.selectedKey && !self.isReadOnly) {
                State.data.isDirty = true;
                self.scheduleSave();
            }
            self.updatePlaceholder();
            self.invalidatePreviewCache();
        });

        self.instance.onDidFocusEditorWidget(function () {
            Main.setActivePane('editor');
        });

        if (callback) callback();
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

    resetState: function () {
        this.currentHandle = null;
        this.currentKey = null;
        this.keyHash = null;
        this.invalidatePreviewCache();
        this.hideError();
    },

    showError: function (message) {
        this.dom.emptyState.textContent = message;
        this.dom.emptyState.style.display = 'flex';
        if (this.dom.editorTabs) this.dom.editorTabs.style.display = 'none';
        if (this.dom.editorViewport) this.dom.editorViewport.style.display = 'none';
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
    },

    isImageFile: function (filename) {
        const ext = (filename.split('.').pop() || '').toLowerCase();
        return this.IMAGE_EXTENSIONS.has(ext);
    },

    transformAttLinks: function (markdown, keyHash, attachments) {
        const attSet = new Set(attachments.map(function (a) {
            return a.filename;
        }));
        const self = this;
        // Match both ![text](att:file) and [text](att:file)
        // Filename in URL should be URL-encoded, so [^)]+ is safe
        return markdown.replace(/(!?)\[([^\]]*)\]\(att:([^)]+)\)/g, function (match, bang, text, urlFilename) {
            // Decode the filename from URL (handles %20, %29, etc.)
            var filename;
            try {
                filename = decodeURIComponent(urlFilename);
            } catch (e) {
                filename = urlFilename;
            }
            if (!attSet.has(filename)) {
                return match;
            }
            var encodedName = encodeURIComponent(filename);
            if (bang && self.isImageFile(filename)) {
                // ![text](att:file) for images: render inline via virtual host
                return '![' + text + '](https://keva-data.local/blobs/' + keyHash + '/' + encodedName + ')';
            } else {
                // [text](att:file) or non-images: clickable link via NavigationStarting
                return '[' + text + '](att:' + keyHash + '/' + encodedName + ')';
            }
        });
    },

    // Transform attachment links for export (e.g., copy HTML)
    transformForExport: function (html, keyHash) {
        // Convert att: links to virtual host blob URLs so exported HTML renders images.
        var attPattern = /att:([^"]+)/g;
        html = html.replace(attPattern, function (match, path) {
            return 'https://keva-data.local/blobs/' + path;
        });

        return html;
    },

    initMarkdownIt: function () {
        if (this.markdownIt) return;
        this.markdownIt = markdownit({
            html: false,
            linkify: true,
            highlight: function (str, lang) {
                if (lang && hljs.getLanguage(lang)) {
                    try {
                        return hljs.highlight(str, {language: lang}).value;
                    } catch (e) {
                        // ignore
                    }
                }
                return '';
            }
        });
        // Allow att: scheme in links (for attachment URLs)
        var defined = this.markdownIt.validateLink;
        this.markdownIt.validateLink = function (url) {
            if (url.startsWith('att:')) return true;
            return defined(url);
        };
    },

    renderPreview: function () {
        if (!this.instance) return '';

        if (this.previewCache.html !== null) {
            return this.previewCache.html;
        }

        this.initMarkdownIt();

        const content = this.instance.getValue();
        const transformed = this.transformAttLinks(content, this.keyHash, State.data.attachments);
        const rawHtml = this.markdownIt.render(transformed);
        // Allow att: scheme for attachment links
        const cleanHtml = DOMPurify.sanitize(rawHtml, {
            ADD_ATTR: ['target'],
            ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto|tel|att):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i
        });

        this.previewCache.html = cleanHtml;
        return cleanHtml;
    },

    // Render preview for external use (copy HTML)
    renderPreviewForExport: function () {
        const html = this.renderPreview();
        return this.transformForExport(html, this.keyHash);
    },

    invalidatePreviewCache: function () {
        this.previewCache.html = null;
    }
};

// Expose retrySave globally for onclick handler
window.Editor = Editor;
