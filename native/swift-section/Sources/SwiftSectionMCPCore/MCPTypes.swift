import Foundation

struct JSONRPCRequest: Decodable {
    let jsonrpc: String
    let method: String
    let id: JSONRPCID?
    let params: JSONValue?

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let jsonrpc = try container.decodeIfPresent(String.self, forKey: .jsonrpc) ?? "2.0"
        guard jsonrpc == "2.0" else {
            throw JSONRPCStreamError.unsupportedVersion(jsonrpc)
        }
        self.jsonrpc = jsonrpc
        self.method = try container.decode(String.self, forKey: .method)
        self.id = try container.decodeIfPresent(JSONRPCID.self, forKey: .id)
        self.params = try container.decodeIfPresent(JSONValue.self, forKey: .params)
    }

    private enum CodingKeys: String, CodingKey {
        case jsonrpc
        case method
        case id
        case params
    }
}

enum JSONRPCID: Hashable {
    case int(Int)
    case string(String)
}

enum JSONValue: Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])
}

struct JSONRPCSuccessResponse<Result: Encodable>: Encodable {
    let jsonrpc = "2.0"
    let id: JSONRPCID
    let result: Result
}

struct JSONRPCErrorResponse: Encodable {
    let jsonrpc = "2.0"
    let id: JSONRPCID
    let error: JSONRPCError
}

struct JSONRPCError: Codable {
    let code: Int
    let message: String
    let data: JSONValue?
}

struct JSONRPCResponseEnvelope: Decodable {
    let jsonrpc: String
    let id: JSONRPCID?
    let result: JSONValue?
    let error: JSONRPCError?
}

enum JSONRPCErrorCode: Int {
    case parseError = -32700
    case invalidRequest = -32600
    case methodNotFound = -32601
    case invalidParams = -32602
    case internalError = -32603
}

struct InitializeParams: Decodable {
    struct ClientInfo: Decodable {
        let name: String
        let version: String?
    }

    let capabilities: JSONValue?
    let clientInfo: ClientInfo?
    let protocolVersion: String?
}

struct InitializeResult: Encodable {
    let protocolVersion: String
    let capabilities: ServerCapabilities
}

struct ServerCapabilities: Encodable {
    let tools: ToolCapabilities
}

struct ToolCapabilities: Encodable {
    let list: Bool
    let call: Bool
}

struct ToolsListResult: Encodable {
    let tools: [ToolDescription]
}

struct ToolDescription: Encodable {
    let name: String
    let description: String
    let inputSchema: JSONValue
}

struct ToolCallParams: Decodable {
    let name: String
    let arguments: JSONValue?
}

extension JSONRPCID: Codable {
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let intValue = try? container.decode(Int.self) {
            self = .int(intValue)
        } else if let stringValue = try? container.decode(String.self) {
            self = .string(stringValue)
        } else {
            throw DecodingError.typeMismatch(
                JSONRPCID.self,
                .init(codingPath: decoder.codingPath, debugDescription: "Unsupported id type")
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .int(let intValue):
            try container.encode(intValue)
        case .string(let stringValue):
            try container.encode(stringValue)
        }
    }
}

extension JSONValue: Codable {
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let boolValue = try? container.decode(Bool.self) {
            self = .bool(boolValue)
        } else if let stringValue = try? container.decode(String.self) {
            self = .string(stringValue)
        } else if let doubleValue = try? container.decode(Double.self) {
            self = .number(doubleValue)
        } else if let arrayValue = try? container.decode([JSONValue].self) {
            self = .array(arrayValue)
        } else if let objectValue = try? container.decode([String: JSONValue].self) {
            self = .object(objectValue)
        } else {
            throw DecodingError.typeMismatch(
                JSONValue.self,
                .init(codingPath: decoder.codingPath, debugDescription: "Unsupported JSON value")
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null:
            try container.encodeNil()
        case .bool(let value):
            try container.encode(value)
        case .number(let value):
            try container.encode(value)
        case .string(let value):
            try container.encode(value)
        case .array(let value):
            try container.encode(value)
        case .object(let value):
            try container.encode(value)
        }
    }
}

extension JSONValue {
    func decode<T: Decodable>(_ type: T.Type, using decoder: JSONDecoder = JSONDecoder()) throws -> T {
        let encoder = JSONEncoder()
        let data = try encoder.encode(self)
        return try decoder.decode(T.self, from: data)
    }
}
