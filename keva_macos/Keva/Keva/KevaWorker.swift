import Foundation

/// Manages the Rust worker thread via C FFI.
///
/// All responses from the worker arrive via `onMessage`, dispatched to the main thread.
class KevaWorker {
    private var handle: UnsafeMutableRawPointer?
    var onMessage: (([String: Any]) -> Void)?

    func start() {
        guard handle == nil else { return }

        let dataDir = Self.dataDirectory()

        // Ensure the data directory exists
        try? FileManager.default.createDirectory(
            atPath: dataDir,
            withIntermediateDirectories: true,
            attributes: nil
        )

        let context = Unmanaged.passUnretained(self).toOpaque()
        handle = keva_worker_start(dataDir, workerCallback, context)
    }

    func send(json: String) {
        guard let handle else { return }
        json.withCString { cstr in
            keva_worker_send(handle, cstr)
        }
    }

    /// Send a message dictionary as JSON to the worker.
    func send(message: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: message),
              let json = String(data: data, encoding: .utf8) else {
            return
        }
        send(json: json)
    }

    /// Stop the worker thread. Blocks until the worker exits.
    func stop() {
        guard let handle else { return }
        keva_worker_stop(handle)
        self.handle = nil
    }

    private static func dataDirectory() -> String {
        if let envDir = ProcessInfo.processInfo.environment["KEVA_DATA_DIR"] {
            return envDir
        }
        let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first!
        return appSupport.appendingPathComponent("keva").path
    }
}

/// C callback invoked from the Rust worker thread.
///
/// Dispatches the JSON response to the main thread and invokes `onMessage`.
private func workerCallback(context: UnsafeMutableRawPointer?, json: UnsafePointer<CChar>?) {
    guard let context, let json else { return }

    let jsonString = String(cString: json)

    DispatchQueue.main.async {
        let worker = Unmanaged<KevaWorker>.fromOpaque(context).takeUnretainedValue()
        guard let data = jsonString.data(using: .utf8),
              let msg = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return
        }
        worker.onMessage?(msg)
    }
}
