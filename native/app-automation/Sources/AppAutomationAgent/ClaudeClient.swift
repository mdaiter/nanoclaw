import Foundation

actor ClaudeClient {
    private let apiKey: String
    private let model: String
    private let baseURL = URL(string: "https://api.anthropic.com/v1/messages")!
    
    init(apiKey: String, model: String = "claude-sonnet-4-20250514") {
        self.apiKey = apiKey
        self.model = model
    }
    
    func chat(
        system: String,
        messages: [ClaudeMessage],
        tools: [ClaudeTool]
    ) async throws -> ClaudeResponse {
        let request = ClaudeRequest(
            model: model,
            max_tokens: 4096,
            system: system,
            messages: messages,
            tools: tools
        )
        
        var urlRequest = URLRequest(url: baseURL)
        urlRequest.httpMethod = "POST"
        urlRequest.addValue("application/json", forHTTPHeaderField: "Content-Type")
        urlRequest.addValue(apiKey, forHTTPHeaderField: "x-api-key")
        urlRequest.addValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
        
        let encoder = JSONEncoder()
        urlRequest.httpBody = try encoder.encode(request)
        
        let (data, response) = try await URLSession.shared.data(for: urlRequest)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw ClaudeClientError.invalidResponse
        }
        
        if httpResponse.statusCode != 200 {
            if let errorResponse = try? JSONDecoder().decode(ClaudeError.self, from: data) {
                throw ClaudeClientError.apiError(
                    status: httpResponse.statusCode,
                    message: errorResponse.error.message
                )
            }
            throw ClaudeClientError.httpError(status: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        return try decoder.decode(ClaudeResponse.self, from: data)
    }
}

enum ClaudeClientError: Error, LocalizedError {
    case invalidResponse
    case httpError(status: Int)
    case apiError(status: Int, message: String)
    case missingAPIKey
    
    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from Claude API"
        case .httpError(let status):
            return "HTTP error: \(status)"
        case .apiError(let status, let message):
            return "API error (\(status)): \(message)"
        case .missingAPIKey:
            return "ANTHROPIC_API_KEY environment variable not set"
        }
    }
}
