import Foundation

public struct AutomationElement: Codable, Hashable {
    public let id: String
    public let role: String
    public let title: String?
    public let value: String?
    public let enabled: Bool
    public let focused: Bool
    public let path: String
    public let bounds: AutomationBounds?
    public let actions: [String]

    public init(
        id: String,
        role: String,
        title: String?,
        value: String?,
        enabled: Bool,
        focused: Bool,
        path: String,
        bounds: AutomationBounds?,
        actions: [String]
    ) {
        self.id = id
        self.role = role
        self.title = title
        self.value = value
        self.enabled = enabled
        self.focused = focused
        self.path = path
        self.bounds = bounds
        self.actions = actions
    }
}

public struct AutomationBounds: Codable, Hashable {
    public let minX: Double
    public let minY: Double
    public let maxX: Double
    public let maxY: Double

    public init(minX: Double, minY: Double, maxX: Double, maxY: Double) {
        self.minX = minX
        self.minY = minY
        self.maxX = maxX
        self.maxY = maxY
    }
}

public struct AutomationSnapshot: Codable {
    public let timestamp: String
    public let appName: String
    public let pid: Int32
    public let focusedElement: String?
    public let elements: [AutomationElement]
    public let hash: String

    public init(
        timestamp: String,
        appName: String,
        pid: Int32,
        focusedElement: String?,
        elements: [AutomationElement],
        hash: String
    ) {
        self.timestamp = timestamp
        self.appName = appName
        self.pid = pid
        self.focusedElement = focusedElement
        self.elements = elements
        self.hash = hash
    }
}

public struct AutomationDiff: Codable {
    public struct ElementChange: Codable {
        public let id: String
        public let field: String
        public let before: String?
        public let after: String?

        public init(id: String, field: String, before: String?, after: String?) {
            self.id = id
            self.field = field
            self.before = before
            self.after = after
        }
    }

    public let changed: Bool
    public let added: [AutomationElement]
    public let removed: [AutomationElement]
    public let modified: [ElementChange]
    public let signals: [String]
    public let summary: String

    public init(
        changed: Bool,
        added: [AutomationElement],
        removed: [AutomationElement],
        modified: [ElementChange],
        signals: [String],
        summary: String
    ) {
        self.changed = changed
        self.added = added
        self.removed = removed
        self.modified = modified
        self.signals = signals
        self.summary = summary
    }
}
