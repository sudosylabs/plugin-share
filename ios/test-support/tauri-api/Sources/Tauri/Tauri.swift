import Foundation

public typealias JsonObject = [String: Any?]

open class Plugin: NSObject {
    public let manager = PluginManager()

    public required override init() {
        super.init()
    }
}

open class PluginManager: NSObject {
    public var viewController: AnyObject?
}

open class Invoke: NSObject {
    open func parseArgs<T: Decodable>(_ type: T.Type) throws -> T {
        throw NSError(domain: "TauriTestSupport", code: 1)
    }

    open func resolve() {}

    open func resolve(_ value: JsonObject) {}

    open func resolve<T: Encodable>(_ value: T) {}

    open func reject(_ message: String) {}
}
