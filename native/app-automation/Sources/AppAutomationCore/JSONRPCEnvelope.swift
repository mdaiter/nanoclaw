import Foundation

public struct JSONRPCEnvelope: Codable {
    public let jsonrpc: String
    public let id: String
    public let method: String
    public let params: [String: AnyCodableValue]?

    public init(jsonrpc: String = "2.0", id: String, method: String, params: [String: AnyCodableValue]?) {
        self.jsonrpc = jsonrpc
        self.id = id
        self.method = method
        self.params = params
    }
}
