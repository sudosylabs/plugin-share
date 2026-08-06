public struct ShareAnchor: Decodable {
    public let x: Double
    public let y: Double
    public let width: Double
    public let height: Double

    public init(x: Double, y: Double, width: Double, height: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }
}

public struct SharedFile: Decodable {
    public let data: String
    public let name: String
    public let mimeType: String

    public init(data: String, name: String, mimeType: String) {
        self.data = data
        self.name = name
        self.mimeType = mimeType
    }
}

public struct ShareOptions: Decodable {
    public var text: String?
    public var title: String?
    public var url: String?
    public var files: [SharedFile]?
    public var filePaths: [String]?
    public var anchor: ShareAnchor?

    public init(text: String? = nil, title: String? = nil, url: String? = nil, files: [SharedFile]? = nil, filePaths: [String]? = nil, anchor: ShareAnchor? = nil) {
        self.text = text
        self.title = title
        self.url = url
        self.files = files
        self.filePaths = filePaths
        self.anchor = anchor
    }
}
