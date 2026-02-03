import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: MainWindow?
    private var statusItemController: StatusItemController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSLog("Keva launched")

        setupMainMenu()

        window = MainWindow()
        window?.show()

        statusItemController = StatusItemController(window: window!)
    }

    // MARK: - Main Menu

    private func setupMainMenu() {
        let mainMenu = NSMenu()

        // Application menu
        let appMenu = NSMenu()
        let appMenuItem = NSMenuItem()
        appMenuItem.submenu = appMenu
        mainMenu.addItem(appMenuItem)

        appMenu.addItem(withTitle: "About Keva", action: #selector(showAbout), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Settings...", action: #selector(showSettings), keyEquivalent: ",")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Quit Keva", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        NSApp.mainMenu = mainMenu
    }

    @objc private func showAbout() {
        let alert = NSAlert()
        alert.messageText = "Keva"
        alert.informativeText = "Version \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0")\n\nA quick-access note taking app."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    @objc private func showSettings() {
        window?.show()
        // Send default config for now - proper config loading will be added in M12
        let defaultConfig: [String: Any] = [
            "general": [
                "theme": "system",
                "showTrayIcon": true,
                "welcomeShown": true
            ],
            "shortcuts": [
                "globalShortcut": "Meta+Alt+KeyK",
                "focusSearch": "Meta+KeyS",
                "copyMarkdown": "Meta+KeyT",
                "copyHtml": "Meta+KeyR",
                "copyFiles": "Meta+KeyF"
            ],
            "lifecycle": [
                "trashTtlDays": 30,
                "purgeTtlDays": 7
            ]
        ]
        window?.webViewController.postMessage([
            "type": "openSettings",
            "config": defaultConfig,
            "launchAtLogin": false
        ])
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let webVC = window?.webViewController else {
            return .terminateNow
        }

        // Ask frontend to save before shutting down
        webVC.initiateShutdown()
        return .terminateLater
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        window?.show()
        return true
    }
}
