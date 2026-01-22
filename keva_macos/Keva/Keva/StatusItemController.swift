import Cocoa

class StatusItemController {
    private var statusItem: NSStatusItem?
    private weak var window: MainWindow?

    init(window: MainWindow) {
        self.window = window
        setupStatusItem()
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
            window?.toggle()
        }
    }

    private func showStatusMenu() {
        let menu = NSMenu()

        let showItem = NSMenuItem(title: "Show Keva", action: #selector(showWindow), keyEquivalent: "")
        showItem.target = self
        showItem.isEnabled = !(window?.isVisible ?? false)
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

    @objc private func showWindow() {
        window?.show()
    }
}
