import Foundation

/// Main agent loop that orchestrates LLM and tool execution
actor AgentLoop {
    private let appName: String
    private let client: ClaudeClient
    private let executor: ToolExecutor
    private let maxIterations: Int
    
    init(appName: String, apiKey: String, maxIterations: Int = 15) {
        self.appName = appName
        self.client = ClaudeClient(apiKey: apiKey)
        self.executor = ToolExecutor(appName: appName)
        self.maxIterations = maxIterations
    }
    
    func run(command: String) async throws -> AgentResult {
        // Connect to app
        guard await executor.connect() else {
            return AgentResult(
                success: false,
                iterations: 0,
                steps: [],
                summary: "Failed to connect to app '\(appName)'. Is it running?",
                error: "App not running or not accessible"
            )
        }
        
        // Get initial observation
        guard let initialUI = await executor.observe() else {
            return AgentResult(
                success: false,
                iterations: 0,
                steps: [],
                summary: "Failed to observe app UI",
                error: "Could not get UI snapshot"
            )
        }
        
        // Build system prompt
        let systemPrompt = buildSystemPrompt()
        
        // Build initial messages
        var messages: [ClaudeMessage] = [
            .user("Task: \(command)\n\nCurrent UI state:\n\(initialUI.asText())")
        ]
        
        var steps: [StepResult] = []
        var iterations = 0
        
        // Main loop
        for iteration in 0..<maxIterations {
            iterations = iteration + 1
            
            // Call Claude
            let response: ClaudeResponse
            do {
                response = try await client.chat(
                    system: systemPrompt,
                    messages: messages,
                    tools: AgentTools.allTools
                )
            } catch {
                return AgentResult(
                    success: false,
                    iterations: iterations,
                    steps: steps,
                    summary: "API error",
                    error: error.localizedDescription
                )
            }
            
            // Check if done (no tool calls = task complete)
            let toolUses = response.toolUses
            if toolUses.isEmpty {
                let summary = response.textContent.isEmpty ? "Task completed" : response.textContent
                return AgentResult(
                    success: true,
                    iterations: iterations,
                    steps: steps,
                    summary: summary
                )
            }
            
            // Execute each tool call
            var toolResults: [ClaudeToolResult] = []
            
            for toolUse in toolUses {
                // Execute with retry
                var result = await executor.execute(toolUse: toolUse)
                
                // Retry once on failure
                if !result.success {
                    try? await Task.sleep(nanoseconds: 200_000_000) // 200ms
                    result = await executor.execute(toolUse: toolUse)
                }
                
                steps.append(StepResult(
                    tool: toolUse.name,
                    success: result.success,
                    message: result.message,
                    details: formatToolInput(toolUse.input)
                ))
                
                // Build tool result message
                var resultMessage = result.message
                
                // If it was an action (not observe), append new UI state
                if toolUse.name != "observe_ui" {
                    if let newUI = await executor.observe() {
                        resultMessage += "\n\nUpdated UI state:\n\(newUI.asText())"
                    }
                }
                
                toolResults.append(ClaudeToolResult(
                    tool_use_id: toolUse.id,
                    content: resultMessage
                ))
            }
            
            // Add assistant response and tool results to conversation
            messages.append(.assistantToolUse(toolUses))
            messages.append(.toolResult(toolResults))
        }
        
        // Max iterations reached
        return AgentResult(
            success: false,
            iterations: iterations,
            steps: steps,
            summary: "Max iterations (\(maxIterations)) reached without completing task",
            error: "iteration_limit"
        )
    }
    
    private func buildSystemPrompt() -> String {
        let hints = AppHints.hints(for: appName)
        
        return """
        You are an automation agent controlling macOS apps via Accessibility APIs.
        
        Current app: \(appName)
        
        \(hints)
        
        IMPORTANT RULES:
        1. Start by analyzing the current UI state provided
        2. Execute ONE action at a time, then analyze the updated UI
        3. Use keyboard shortcuts when available (faster and more reliable than clicking)
        4. If an action fails, try an alternative approach
        5. When the task is complete, respond with a summary message (no tool call)
        
        ELEMENT IDS:
        - Elements are identified by IDs like 'e1', 'e42', etc.
        - Use these IDs with the click, focus, or type_text tools
        - IDs may change after actions, so always check the updated UI
        
        KEYBOARD SHORTCUTS:
        - Use press_key for keyboard shortcuts (e.g., cmd+t for new tab)
        - Modifiers: 'command', 'shift', 'option', 'control'
        - Common keys: 'return', 'escape', 'tab', 'delete', 'up', 'down', 'left', 'right'
        
        Current UI state will be provided after each action so you can verify success.
        """
    }
    
    private func formatToolInput(_ input: [String: JSONValue]) -> [String: String]? {
        if input.isEmpty { return nil }
        var result: [String: String] = [:]
        for (key, value) in input {
            result[key] = value.description
        }
        return result
    }
}
