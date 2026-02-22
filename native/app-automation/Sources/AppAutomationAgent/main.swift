import Foundation

@main
struct AppAgentMain {
    static func main() async {
        let args = CommandLine.arguments
        
        // Parse arguments
        guard args.count >= 3 else {
            printUsage()
            exit(1)
        }
        
        let appName = args[1]
        let command = args[2..<args.count].joined(separator: " ")
        
        // Get API key
        guard let apiKey = ProcessInfo.processInfo.environment["ANTHROPIC_API_KEY"] else {
            printError("ANTHROPIC_API_KEY environment variable not set")
            printError("Set it with: export ANTHROPIC_API_KEY=sk-ant-...")
            exit(1)
        }
        
        // Check for help
        if appName == "--help" || appName == "-h" || appName == "help" {
            printUsage()
            exit(0)
        }
        
        // Run agent
        let agent = AgentLoop(appName: appName, apiKey: apiKey)
        
        do {
            let result = try await agent.run(command: command)
            
            // Output JSON
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            
            if let data = try? encoder.encode(result),
               let json = String(data: data, encoding: .utf8) {
                print(json)
            }
            
            exit(result.success ? 0 : 1)
        } catch {
            let errorResult = AgentResult(
                success: false,
                iterations: 0,
                steps: [],
                summary: "Agent failed",
                error: error.localizedDescription
            )
            
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            
            if let data = try? encoder.encode(errorResult),
               let json = String(data: data, encoding: .utf8) {
                print(json)
            }
            
            exit(1)
        }
    }
    
    static func printUsage() {
        let usage = """
        app-agent - LLM-powered macOS app automation
        
        Usage:
          app-agent <app-name> "<natural language command>"
        
        Examples:
          app-agent Safari "open a new tab and go to github.com"
          app-agent Messages "send 'hello' to John"
          app-agent Finder "go to Downloads folder"
          app-agent Notes "create a new note titled 'Meeting Notes'"
        
        Environment:
          ANTHROPIC_API_KEY    Your Anthropic API key (required)
        
        Output:
          JSON with success status, steps taken, and summary
        
        Supported Apps:
          Any macOS app with Accessibility support. Best results with:
          - Safari, Chrome, Firefox (browsers)
          - Messages, Mail, Slack (communication)
          - Finder, Notes, TextEdit (productivity)
          - Terminal, Preview, Calendar
        
        Tips:
          - Make sure the app is running before executing commands
          - Grant Accessibility permissions to Terminal in System Preferences
          - Commands work best when they're specific and actionable
        """
        print(usage)
    }
    
    static func printError(_ message: String) {
        fputs("Error: \(message)\n", stderr)
    }
}
