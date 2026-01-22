import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var window: NSWindow?
    var eventMonitor: Any?
    var statusItem: NSStatusItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSLog("Keva launched")

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.borderless, .resizable],
            backing: .buffered,
            defer: false
        )
        window?.delegate = self
        window?.level = .floating
        window?.minSize = NSSize(width: 400, height: 300)
        window?.isMovableByWindowBackground = true
        window?.center()
        window?.makeKeyAndOrderFront(nil)

        setupStatusItem()
        setupKeyEventMonitor()
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showWindow()
        return true
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        return false
    }

    var isWindowVisible: Bool {
        window?.isVisible ?? false
    }

    @objc func showWindow() {
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func hideWindow() {
        window?.orderOut(nil)
    }

    func toggleWindow() {
        if isWindowVisible {
            hideWindow()
        } else {
            showWindow()
        }
    }

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        guard let button = statusItem?.button else { return }

        button.image = NSImage(systemSymbolName: "k.square", accessibilityDescription: "Keva")
        button.toolTip = "Keva"
        button.target = self
        button.action = #selector(statusItemClicked)
        button.sendAction(on: [.leftMouseUp, .rightMouseUp])
    }

    @objc private func statusItemClicked() {
        guard let event = NSApp.currentEvent else { return }

        if event.type == .rightMouseUp {
            showStatusMenu()
        } else {
            toggleWindow()
        }
    }

    private func showStatusMenu() {
        let menu = NSMenu()

        let showItem = NSMenuItem(title: "Show Keva", action: #selector(showWindow), keyEquivalent: "")
        showItem.target = self
        showItem.isEnabled = !isWindowVisible
        menu.addItem(showItem)

        let settingsItem = NSMenuItem(title: "Settings...", action: nil, keyEquivalent: ",")
        settingsItem.isEnabled = false
        menu.addItem(settingsItem)

        menu.addItem(NSMenuItem.separator())

        let launchItem = NSMenuItem(title: "Launch at Login", action: nil, keyEquivalent: "")
        launchItem.isEnabled = false
        menu.addItem(launchItem)

        menu.addItem(NSMenuItem.separator())

        let quitItem = NSMenuItem(title: "Quit Keva", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        menu.addItem(quitItem)

        statusItem?.menu = menu
        statusItem?.button?.performClick(nil)
        statusItem?.menu = nil
    }

    private func setupKeyEventMonitor() {
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self = self else { return event }

            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

            // Cmd+Q: Quit application
            // Use `characters` (not `charactersIgnoringModifiers`) because macOS switches to
            // Latin layer when Command is pressed, even on non-Latin keyboards (Korean, Hebrew, etc.)
            if modifiers == .command && event.characters?.lowercased() == "q" {
                NSApp.terminate(nil)
                return nil
            }

            // Esc: Hide window
            if event.keyCode == 53 {
                self.hideWindow()
                return nil
            }

            return event
        }
    }
}
