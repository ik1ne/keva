import Cocoa
import WebKit

/// View controller that hosts the WKWebView displaying the Keva frontend.
class WebViewController: NSViewController, WKNavigationDelegate, WKScriptMessageHandler {
    private(set) var webView: WKWebView!
    private var schemeHandler: KevaSchemeHandler!

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
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        webView.navigationDelegate = self

        if let url = URL(string: "\(KevaSchemeHandler.scheme)://index.html") {
            webView.load(URLRequest(url: url))
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

    // MARK: - WKNavigationDelegate

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // TODO: M6 will send coreReady after keva_core initialization
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

        // TODO: M5c will implement message handlers (hide, theme, window drag, etc.)
        _ = type
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
