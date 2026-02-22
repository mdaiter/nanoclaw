import Foundation

protocol MCPTool {
    var name: String { get }
    var description: String { get }
    var inputSchema: JSONValue { get }

    func call(arguments: JSONValue?, context: ToolContext) async throws -> ToolResponse
}

extension MCPTool {
    var toolDescription: ToolDescription {
        ToolDescription(
            name: name,
            description: description,
            inputSchema: inputSchema
        )
    }
}

struct ToolContext {
    let logger: MCPLogger
    let encoder: JSONEncoder
    let decoder: JSONDecoder

    func decodeArguments<T: Decodable>(_ type: T.Type, from value: JSONValue?) throws -> T {
        guard let value else {
            return try decoder.decode(T.self, from: Data("{}".utf8))
        }
        return try value.decode(T.self, using: decoder)
    }
}

struct ToolResponse: Codable {
    let content: [ToolContent]

    static func text(_ text: String) -> ToolResponse {
        ToolResponse(content: [.text(text)])
    }
}

struct ToolContent: Codable {
    let type: String
    let text: String?
    let mimeType: String?
    let data: String?

    static func text(_ text: String) -> ToolContent {
        ToolContent(type: "text", text: text, mimeType: nil, data: nil)
    }
}

enum MCPToolError: LocalizedError {
    case encodingFailed

    var errorDescription: String? {
        switch self {
        case .encodingFailed:
            return "Failed to encode tool output."
        }
    }
}
