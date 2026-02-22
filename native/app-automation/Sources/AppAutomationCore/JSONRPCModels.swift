import Foundation

public struct JSONRPCRequest: Codable {
    public let id: String
    public let method: String
    public let params: [String: AnyCodableValue]?

    public init(id: String, method: String, params: [String: AnyCodableValue]?) {
        self.id = id
        self.method = method
        self.params = params
    }
}

public struct JSONRPCResponse<T: Codable>: Codable {
    public let id: String
    public let result: AutomationResponse<T>

    public init(id: String, result: AutomationResponse<T>) {
        self.id = id
        self.result = result
    }
}

public struct JSONRPCStreamEvent<T: Codable>: Codable {
    public let token: String
    public let event: String
    public let data: T

    public init(token: String, event: String, data: T) {
        self.token = token
        self.event = event
        self.data = data
    }
}
