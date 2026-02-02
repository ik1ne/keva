import Cocoa
import WebKit

/// View controller that hosts the WKWebView displaying the Keva frontend.
class WebViewController: NSViewController, WKNavigationDelegate, WKScriptMessageHandler {
    private(set) var webView: WKWebView!
    private var schemeHandler: KevaSchemeHandler!
    private var appearanceObserver: NSKeyValueObservation?
    private var lastMouseDownEvent: NSEvent?
    private var mouseMonitor: Any?
    let worker = KevaWorker()

    override func loadView() {
        let distPath = Self.findDistPath()
        schemeHandler = KevaSchemeHandler(distPath: distPath)

        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(schemeHandler, forURLScheme: KevaSchemeHandler.scheme)
        config.userContentController.add(self, name: "keva")
        config.userContentController.addUserScript(WKUserScript(
            source: Self.webViewShimScript,
            injectionTime: .atDocumentStart,
            forMainFrameOnly: true
        ))

        #if DEBUG
        config.preferences.setValue(true, forKey: "developerExtrasEnabled")
        #endif

        webView = WKWebView(frame: .zero, configuration: config)
        webView.autoresizingMask = [.width, .height]
        view = webView

        setupAppearanceObserver()
        setupMouseMonitor()
        setupWorker()
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        webView.navigationDelegate = self

        if let url = URL(string: "\(KevaSchemeHandler.scheme)://index.html") {
            webView.load(URLRequest(url: url))
        }
    }

    deinit {
        appearanceObserver?.invalidate()
        if let monitor = mouseMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }

    // MARK: - Message Bridge

    /// Send a message to the WebView.
    func postMessage(_ message: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: message),
              let json = String(data: data, encoding: .utf8) else {
            return
        }
        let script = "window.dispatchEvent(new MessageEvent('message', { data: \(json) }));"
        webView.evaluateJavaScript(script, completionHandler: nil)
    }

    // MARK: - Worker

    private func setupWorker() {
        worker.onMessage = { [weak self] msg in
            self?.handleWorkerMessage(msg)
        }
        worker.start()
    }

    private func handleWorkerMessage(_ msg: [String: Any]) {
        postMessage(msg)
    }

    /// Initiate graceful shutdown: tell frontend to save, then shut down worker.
    func initiateShutdown() {
        postMessage(["type": "shutdown"])
    }

    /// Called when frontend acknowledges shutdown (after saving).
    func handleShutdownAck() {
        worker.send(message: ["type": "shutdownAck"])
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            self?.worker.stop()
            DispatchQueue.main.async {
                NSApp.reply(toApplicationShouldTerminate: true)
            }
        }
    }

    // MARK: - WKNavigationDelegate

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        sendTheme()
        // Worker sends coreReady after keva_core initialization completes
    }

    // MARK: - WKScriptMessageHandler

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.name == "keva",
              let jsonString = message.body as? String,
              let data = jsonString.data(using: .utf8),
              let msg = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = msg["type"] as? String else {
            return
        }

        switch type {
        case "hide":
            (view.window as? MainWindow)?.hide()

        case "startWindowDrag":
            startWindowDrag()

        case "shutdownAck":
            handleShutdownAck()

        default:
            // Forward all other messages to the worker as JSON
            worker.send(json: jsonString)
        }
    }

    // MARK: - Theme

    private func setupAppearanceObserver() {
        appearanceObserver = NSApp.observe(\.effectiveAppearance) { [weak self] _, _ in
            self?.sendTheme()
        }
    }

    private func sendTheme() {
        let isDark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        postMessage(["type": "theme", "theme": isDark ? "dark" : "light"])
    }

    // MARK: - Window Drag

    private func setupMouseMonitor() {
        mouseMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            self?.lastMouseDownEvent = event
            return event
        }
    }

    private func startWindowDrag() {
        guard let window = view.window, let event = lastMouseDownEvent else { return }
        window.performDrag(with: event)
    }

    // MARK: - Private

    /// JavaScript shim providing `window.chrome.webview` API for compatibility with Windows WebView2.
    private static let webViewShimScript = """
        window.chrome = window.chrome || {};
        window.chrome.webview = {
            postMessage: function(msg) {
                window.webkit.messageHandlers.keva.postMessage(msg);
            },
            addEventListener: function(type, listener) {
                if (type === 'message') window.addEventListener('message', listener);
            },
            removeEventListener: function(type, listener) {
                if (type === 'message') window.removeEventListener('message', listener);
            }
        };
        """

    /// Locates the frontend dist folder in the app bundle.
    private static func findDistPath() -> String {
        guard let bundlePath = Bundle.main.resourcePath else {
            fatalError("Bundle.main.resourcePath is nil")
        }
        return bundlePath + "/dist"
    }
}
