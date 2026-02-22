import Foundation

public enum AutomationError: String, Codable, Error {
    case invalidRequest
    case unsupportedAction
    case elementNotFound
    case actionFailed
    case timeout
    case notConnected
    case invalidSelector
    case dryRunBlocked
    case helperUnavailable
}
