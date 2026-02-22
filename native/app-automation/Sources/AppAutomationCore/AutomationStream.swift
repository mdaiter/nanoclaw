import Foundation

public enum AutomationStreamEvent: String, Codable {
    case snapshot
    case diff
    case signal
    case error
}

public struct AutomationStreamToken: Codable, Hashable {
    public let value: String

    public init(value: String) {
        self.value = value
    }
}
