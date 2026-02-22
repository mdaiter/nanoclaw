import Foundation

package final class MCPServer {
    private let reader = JSONRPCMessageReader()
    private let writer = JSONRPCMessageWriter()
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    private let logger = MCPLogger()
    private let tools: [MCPTool]
    private let toolsByName: [String: MCPTool]
    private var isInitialized = false
    private var isShuttingDown = false
    private var shouldTerminate = false

    init(tools: [MCPTool] = [SwiftInterfaceTool(), ListTypesTool(), SearchSymbolsTool()]) {
        self.tools = tools
        self.toolsByName = Dictionary(uniqueKeysWithValues: tools.map { ($0.name, $0) })

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        self.decoder = decoder

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.withoutEscapingSlashes]
        self.encoder = encoder
    }

    package convenience init() {
        self.init(tools: [SwiftInterfaceTool(), ListTypesTool(), SearchSymbolsTool()])
    }

    package func run() async {
        logger.info("Swift Section MCP server booting...")
        do {
            while !shouldTerminate {
                guard let messageData = try reader.nextMessage() else {
                    break
                }
                do {
                    try await handleMessage(data: messageData)
                } catch {
                    logger.error("Failed to handle request: \(error.localizedDescription)")
                }
            }
        } catch {
            logger.error("Stream failure: \(error.localizedDescription)")
        }
        logger.info("Swift Section MCP server stopped.")
    }

    private func handleMessage(data: Data) async throws {
        let request = try decoder.decode(JSONRPCRequest.self, from: data)
        switch request.method {
        case "initialize":
            try handleInitialize(request)
        case "initialized":
            logger.debug("Client reported initialized.")
        case "shutdown":
            try handleShutdown(request)
        case "exit":
            logger.info("Received exit notification.")
            shouldTerminate = true
        case "tools/list":
            try handleToolsList(request)
        case "tools/call":
            try await handleToolsCall(request)
        default:
            try respondError(
                id: request.id,
                code: JSONRPCErrorCode.methodNotFound.rawValue,
                message: "Method \(request.method) is not implemented."
            )
        }
    }

    private func handleInitialize(_ request: JSONRPCRequest) throws {
        guard let id = request.id else {
            logger.error("initialize request is missing an id.")
            return
        }

        _ = try decode(InitializeParams.self, from: request.params)

        isInitialized = true
        let result = InitializeResult(
            protocolVersion: "2024-11-05",
            capabilities: ServerCapabilities(
                tools: ToolCapabilities(list: true, call: true)
            )
        )
        try respondSuccess(id: id, result: result)
    }

    private func handleShutdown(_ request: JSONRPCRequest) throws {
        guard let id = request.id else { return }
        isShuttingDown = true
        let result: JSONValue = .null
        try respondSuccess(id: id, result: result)
    }

    private func handleToolsList(_ request: JSONRPCRequest) throws {
        guard let id = request.id else { return }
        guard isInitialized else {
            try respondError(
                id: id,
                code: JSONRPCErrorCode.invalidRequest.rawValue,
                message: "initialize must be called before tools/list."
            )
            return
        }

        let definitions = tools.map { $0.toolDescription }
        let result = ToolsListResult(tools: definitions)
        try respondSuccess(id: id, result: result)
    }

    private func handleToolsCall(_ request: JSONRPCRequest) async throws {
        guard let id = request.id else { return }
        guard isInitialized else {
            try respondError(
                id: id,
                code: JSONRPCErrorCode.invalidRequest.rawValue,
                message: "initialize must be called before tools/call."
            )
            return
        }

        let params = try decode(ToolCallParams.self, from: request.params)
        guard let tool = toolsByName[params.name] else {
            try respondError(
                id: id,
                code: JSONRPCErrorCode.methodNotFound.rawValue,
                message: "Unknown tool named \(params.name)."
            )
            return
        }

        let context = ToolContext(logger: logger, encoder: encoder, decoder: decoder)
        do {
            let result = try await tool.call(arguments: params.arguments, context: context)
            try respondSuccess(id: id, result: result)
        } catch {
            try respondError(
                id: id,
                code: JSONRPCErrorCode.internalError.rawValue,
                message: error.localizedDescription
            )
        }
    }

    private func decode<T: Decodable>(_ type: T.Type, from value: JSONValue?) throws -> T {
        guard let value else {
            return try decoder.decode(T.self, from: Data("{}".utf8))
        }
        return try value.decode(T.self, using: decoder)
    }

    private func respondSuccess<Result: Encodable>(id: JSONRPCID, result: Result) throws {
        let response = JSONRPCSuccessResponse(id: id, result: result)
        try writer.send(response, encoder: encoder)
    }

    private func respondError(id: JSONRPCID?, code: Int, message: String, data: JSONValue? = nil) throws {
        guard let id else {
            logger.error("Unable to send error response without id: \(message)")
            return
        }
        let error = JSONRPCError(code: code, message: message, data: data)
        let response = JSONRPCErrorResponse(id: id, error: error)
        try writer.send(response, encoder: encoder)
    }
}
