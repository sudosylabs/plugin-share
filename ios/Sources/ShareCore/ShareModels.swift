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

    public init(text: String? = nil, title: String? = nil, url: String? = nil, files: [SharedFile]? = nil) {
        self.text = text
        self.title = title
        self.url = url
        self.files = files
    }
}
