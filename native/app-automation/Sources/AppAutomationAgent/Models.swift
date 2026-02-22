import Foundation

// MARK: - Claude API Request/Response Types

struct ClaudeRequest: Encodable {
    let model: String
    let max_tokens: Int
    let system: String
    let messages: [ClaudeMessage]
    let tools: [ClaudeTool]
}

struct ClaudeMessage: Codable {
    let role: String
    let content: ClaudeContent
    
    static func user(_ text: String) -> ClaudeMessage {
        ClaudeMessage(role: "user", content: .text(text))
    }
    
    static func assistant(_ text: String) -> ClaudeMessage {
        ClaudeMessage(role: "assistant", content: .text(text))
    }
    
    static func assistantToolUse(_ toolUses: [ClaudeToolUse]) -> ClaudeMessage {
        ClaudeMessage(role: "assistant", content: .blocks(toolUses.map { .toolUse($0) }))
    }
    
    static func toolResult(_ results: [ClaudeToolResult]) -> ClaudeMessage {
        ClaudeMessage(role: "user", content: .blocks(results.map { .toolResult($0) }))
    }
}

enum ClaudeContent: Codable {
    case text(String)
    case blocks([ClaudeContentBlock])
    
    func encode(to encoder: Encoder) throws {
        switch self {
        case .text(let text):
            var container = encoder.singleValueContainer()
            try container.encode(text)
        case .blocks(let blocks):
            var container = encoder.singleValueContainer()
            try container.encode(blocks)
        }
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let text = try? container.decode(String.self) {
            self = .text(text)
        } else if let blocks = try? container.decode([ClaudeContentBlock].self) {
            self = .blocks(blocks)
        } else {
            self = .text("")
        }
    }
}

enum ClaudeContentBlock: Codable {
    case text(String)
    case toolUse(ClaudeToolUse)
    case toolResult(ClaudeToolResult)
    
    enum CodingKeys: String, CodingKey {
        case type, text, id, name, input, tool_use_id, content
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .text(let text):
            try container.encode("text", forKey: .type)
            try container.encode(text, forKey: .text)
        case .toolUse(let toolUse):
            try container.encode("tool_use", forKey: .type)
            try container.encode(toolUse.id, forKey: .id)
            try container.encode(toolUse.name, forKey: .name)
            try container.encode(toolUse.input, forKey: .input)
        case .toolResult(let result):
            try container.encode("tool_result", forKey: .type)
            try container.encode(result.tool_use_id, forKey: .tool_use_id)
            try container.encode(result.content, forKey: .content)
        }
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        
        switch type {
        case "text":
            let text = try container.decode(String.self, forKey: .text)
            self = .text(text)
        case "tool_use":
            let id = try container.decode(String.self, forKey: .id)
            let name = try container.decode(String.self, forKey: .name)
            let input = try container.decode([String: JSONValue].self, forKey: .input)
            self = .toolUse(ClaudeToolUse(id: id, name: name, input: input))
        case "tool_result":
            let toolUseId = try container.decode(String.self, forKey: .tool_use_id)
            let content = try container.decode(String.self, forKey: .content)
            self = .toolResult(ClaudeToolResult(tool_use_id: toolUseId, content: content))
        default:
            self = .text("")
        }
    }
}

struct ClaudeToolUse: Codable, Sendable {
    let id: String
    let name: String
    let input: [String: JSONValue]
}

struct ClaudeToolResult: Codable {
    let tool_use_id: String
    let content: String
}

struct ClaudeTool: Encodable {
    let name: String
    let description: String
    let input_schema: JSONSchema
}

struct JSONSchema: Encodable {
    let type: String
    let properties: [String: PropertySchema]
    let required: [String]
    
    static func object(properties: [String: PropertySchema], required: [String] = []) -> JSONSchema {
        JSONSchema(type: "object", properties: properties, required: required)
    }
}

struct PropertySchema: Encodable {
    let type: String
    let description: String
    let enumValues: [String]?
    
    enum CodingKeys: String, CodingKey {
        case type, description
        case enumValues = "enum"
    }
    
    static func string(_ description: String) -> PropertySchema {
        PropertySchema(type: "string", description: description, enumValues: nil)
    }
    
    static func integer(_ description: String) -> PropertySchema {
        PropertySchema(type: "integer", description: description, enumValues: nil)
    }
    
    static func array(_ description: String) -> PropertySchema {
        PropertySchema(type: "array", description: description, enumValues: nil)
    }
    
    static func stringEnum(_ description: String, values: [String]) -> PropertySchema {
        PropertySchema(type: "string", description: description, enumValues: values)
    }
}

struct ClaudeResponse: Decodable {
    let id: String
    let type: String
    let role: String
    let content: [ClaudeContentBlock]
    let model: String
    let stop_reason: String?
    let usage: ClaudeUsage
    
    var textContent: String {
        content.compactMap { block in
            if case .text(let text) = block { return text }
            return nil
        }.joined(separator: "\n")
    }
    
    var toolUses: [ClaudeToolUse] {
        content.compactMap { block in
            if case .toolUse(let toolUse) = block { return toolUse }
            return nil
        }
    }
}

struct ClaudeUsage: Decodable {
    let input_tokens: Int
    let output_tokens: Int
}

struct ClaudeError: Decodable {
    let type: String
    let error: ClaudeErrorDetail
}

struct ClaudeErrorDetail: Decodable {
    let type: String
    let message: String
}

// MARK: - JSON Value for Dynamic Content

enum JSONValue: Codable, CustomStringConvertible, Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case array([JSONValue])
    case object([String: JSONValue])
    case null
    
    var description: String {
        switch self {
        case .string(let s): return s
        case .int(let i): return String(i)
        case .double(let d): return String(d)
        case .bool(let b): return String(b)
        case .array(let a): return a.description
        case .object(let o): return o.description
        case .null: return "null"
        }
    }
    
    var stringValue: String? {
        if case .string(let s) = self { return s }
        return nil
    }
    
    var intValue: Int? {
        if case .int(let i) = self { return i }
        return nil
    }
    
    var arrayValue: [JSONValue]? {
        if case .array(let a) = self { return a }
        return nil
    }
    
    var stringArrayValue: [String]? {
        arrayValue?.compactMap { $0.stringValue }
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let str = try? container.decode(String.self) {
            self = .string(str)
        } else if let int = try? container.decode(Int.self) {
            self = .int(int)
        } else if let double = try? container.decode(Double.self) {
            self = .double(double)
        } else if let bool = try? container.decode(Bool.self) {
            self = .bool(bool)
        } else if let array = try? container.decode([JSONValue].self) {
            self = .array(array)
        } else if let object = try? container.decode([String: JSONValue].self) {
            self = .object(object)
        } else {
            self = .null
        }
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let s): try container.encode(s)
        case .int(let i): try container.encode(i)
        case .double(let d): try container.encode(d)
        case .bool(let b): try container.encode(b)
        case .array(let a): try container.encode(a)
        case .object(let o): try container.encode(o)
        case .null: try container.encodeNil()
        }
    }
}

// MARK: - Agent Result Types

struct AgentResult: Codable {
    let success: Bool
    let iterations: Int
    let steps: [StepResult]
    let summary: String
    let error: String?
    
    init(success: Bool, iterations: Int, steps: [StepResult], summary: String, error: String? = nil) {
        self.success = success
        self.iterations = iterations
        self.steps = steps
        self.summary = summary
        self.error = error
    }
}

struct StepResult: Codable {
    let tool: String
    let success: Bool
    let message: String
    let details: [String: String]?
    
    init(tool: String, success: Bool, message: String, details: [String: String]? = nil) {
        self.tool = tool
        self.success = success
        self.message = message
        self.details = details
    }
}
