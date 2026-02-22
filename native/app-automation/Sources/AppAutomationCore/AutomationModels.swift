import Foundation

public struct AutomationResponse<T: Codable>: Codable {
    public let success: Bool
    public let errorCode: AutomationError?
    public let message: String
    public let retryHint: String?
    public let signals: [String]
    public let changed: Bool?
    public let data: T?

    public init(
        success: Bool,
        errorCode: AutomationError? = nil,
        message: String,
        retryHint: String? = nil,
        signals: [String] = [],
        changed: Bool? = nil,
        data: T? = nil
    ) {
        self.success = success
        self.errorCode = errorCode
        self.message = message
        self.retryHint = retryHint
        self.signals = signals
        self.changed = changed
        self.data = data
    }
}

public struct AnyCodableValue: Codable, Equatable {
    public let value: Any

    public init(_ value: Any) {
        self.value = value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let str = try? container.decode(String.self) {
            value = str
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let dict = try? container.decode([String: AnyCodableValue].self) {
            value = dict
        } else if let arr = try? container.decode([AnyCodableValue].self) {
            value = arr
        } else {
            value = "null"
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch value {
        case let str as String:
            try container.encode(str)
        case let int as Int:
            try container.encode(int)
        case let bool as Bool:
            try container.encode(bool)
        case let double as Double:
            try container.encode(double)
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodableValue($0) })
        case let dict as [String: AnyCodableValue]:
            try container.encode(dict)
        case let arr as [Any]:
            try container.encode(arr.map { AnyCodableValue($0) })
        case let arr as [AnyCodableValue]:
            try container.encode(arr)
        case let snapshot as AutomationSnapshot:
            try container.encode(snapshot)
        case let diff as AutomationDiff:
            try container.encode(diff)
        case let element as AutomationElement:
            try container.encode(element)
        case let elements as [AutomationElement]:
            try container.encode(elements)
        case let profile as CapabilityProfile:
            try container.encode(profile)
        default:
            try container.encode(String(describing: value))
        }
    }

    public static func == (lhs: AnyCodableValue, rhs: AnyCodableValue) -> Bool {
        String(describing: lhs.value) == String(describing: rhs.value)
    }
}
