import Foundation

public enum AutomationActionType: String, Codable {
    case click
    case setValue
    case pressKey
    case scroll
    case invoke
    case menu
    case openUrl
    case shortcut
}

public struct AutomationAction: Codable, Equatable {
    public var action: AutomationActionType
    public var selector: AutomationSelector?
    public var elementId: String?
    public var params: [String: AnyCodableValue]?

    public init(
        action: AutomationActionType,
        selector: AutomationSelector? = nil,
        elementId: String? = nil,
        params: [String: AnyCodableValue]? = nil
    ) {
        self.action = action
        self.selector = selector
        self.elementId = elementId
        self.params = params
    }
}
