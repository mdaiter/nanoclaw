import Foundation

public enum SelectorMatchType: String, Codable {
    case exact
    case contains
}

public struct AutomationSelector: Codable, Equatable {
    public var role: SelectorField?
    public var title: SelectorField?
    public var value: SelectorField?
    public var path: SelectorField?
    public var attributes: [String: SelectorField]?
    public var bounds: SelectorBounds?
    public var window: SelectorField?
    public var limit: Int?
    public var visibility: SelectorVisibility?
    public var enabled: Bool?
    public var focused: Bool?

    public init(
        role: SelectorField? = nil,
        title: SelectorField? = nil,
        value: SelectorField? = nil,
        path: SelectorField? = nil,
        attributes: [String: SelectorField]? = nil,
        bounds: SelectorBounds? = nil,
        window: SelectorField? = nil,
        limit: Int? = nil,
        visibility: SelectorVisibility? = nil,
        enabled: Bool? = nil,
        focused: Bool? = nil
    ) {
        self.role = role
        self.title = title
        self.value = value
        self.path = path
        self.attributes = attributes
        self.bounds = bounds
        self.window = window
        self.limit = limit
        self.visibility = visibility
        self.enabled = enabled
        self.focused = focused
    }
}

public struct SelectorField: Codable, Equatable {
    public var value: String
    public var match: SelectorMatchType

    public init(value: String, match: SelectorMatchType = .exact) {
        self.value = value
        self.match = match
    }
}

public struct SelectorBounds: Codable, Equatable {
    public var minX: Double?
    public var minY: Double?
    public var maxX: Double?
    public var maxY: Double?

    public init(minX: Double? = nil, minY: Double? = nil, maxX: Double? = nil, maxY: Double? = nil) {
        self.minX = minX
        self.minY = minY
        self.maxX = maxX
        self.maxY = maxY
    }
}

public enum SelectorVisibility: String, Codable {
    case visible
    case hidden
}
