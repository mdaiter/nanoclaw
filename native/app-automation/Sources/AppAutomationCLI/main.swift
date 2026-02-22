import Foundation
import AppAutomationCore
import AppAutomationAX

// MARK: - CLI Entry Point

@main
struct AppAutomationCLI {
    static func main() async {
        let args = CommandLine.arguments
        guard args.count >= 2 else {
            printUsage()
            exit(1)
        }
        
        let command = args[1]
        
        switch command {
        case "health":
            print(formatJSON(["status": "ok", "version": "1.0"]))
            
        case "list-apps":
            listApps()
            
        case "observe":
            guard args.count >= 3 else {
                printError("Missing app name")
                exit(1)
            }
            observe(appName: args[2])
            
        case "query":
            guard args.count >= 4 else {
                printError("Usage: query <app> <selector-json>")
                exit(1)
            }
            query(appName: args[2], selectorJSON: args[3])
            
        case "click":
            guard args.count >= 4 else {
                printError("Usage: click <app> <selector-json>")
                exit(1)
            }
            click(appName: args[2], selectorJSON: args[3])
            
        case "type":
            guard args.count >= 5 else {
                printError("Usage: type <app> <selector-json> <text>")
                exit(1)
            }
            typeText(appName: args[2], selectorJSON: args[3], text: args[4])
            
        case "run-script":
            guard args.count >= 3 else {
                printError("Usage: run-script <script-file.json>")
                exit(1)
            }
            await runScript(path: args[2])
            
        case "help", "--help", "-h":
            printUsage()
            
        default:
            printError("Unknown command: \(command)")
            printUsage()
            exit(1)
        }
    }
    
    static func printUsage() {
        print("""
        AppAutomation CLI - macOS App Automation Tool
        
        Usage:
          app-automation <command> [arguments]
        
        Commands:
          health                          Check daemon health
          list-apps                       List supported apps
          observe <app>                   Get UI snapshot of app
          query <app> <selector>          Query elements matching selector
          click <app> <selector>          Click first matching element
          type <app> <selector> <text>    Type text into matching element
          run-script <file>               Run automation script from JSON file
          help                            Show this help
        
        Selector JSON Format:
          {"role": "AXButton", "title": {"value": "Submit", "match": "contains"}}
        
        Script File Format:
          {
            "name": "Test Flow",
            "app": "Safari",
            "steps": [
              {"action": "observe"},
              {"action": "click", "selector": {"role": "AXButton"}},
              {"action": "wait", "ms": 1000}
            ]
          }
        
        Examples:
          app-automation observe Safari
          app-automation query Safari '{"role": "AXButton"}'
          app-automation click Safari '{"title": {"value": "Submit", "match": "exact"}}'
          app-automation run-script test-flow.json
        """)
    }
    
    static func printError(_ message: String) {
        fputs("Error: \(message)\n", stderr)
    }
    
    static func formatJSON(_ dict: [String: Any]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: dict, options: [.prettyPrinted, .sortedKeys]),
              let str = String(data: data, encoding: .utf8) else {
            return "{}"
        }
        return str
    }
    
    static func listApps() {
        let apps = [
            ["name": "Safari", "bundleId": "com.apple.Safari", "supported": true],
            ["name": "Messages", "bundleId": "com.apple.MobileSMS", "supported": true]
        ]
        print(formatJSON(["apps": apps]))
    }
    
    static func observe(appName: String) {
        let driver = AXDriver()
        guard let (appElement, pid) = driver.connect(appName: appName) else {
            printError("App '\(appName)' not running or not accessible")
            exit(1)
        }
        
        let snapshot = driver.observe(appName: appName, appElement: appElement, pid: pid)
        if let data = try? JSONEncoder().encode(snapshot),
           let json = String(data: data, encoding: .utf8) {
            print(json)
        }
    }
    
    static func query(appName: String, selectorJSON: String) {
        guard let selector = parseSelector(selectorJSON) else {
            printError("Invalid selector JSON")
            exit(1)
        }
        
        let driver = AXDriver()
        guard let (appElement, pid) = driver.connect(appName: appName) else {
            printError("App '\(appName)' not running")
            exit(1)
        }
        
        let matches = driver.find(appName: appName, appElement: appElement, pid: pid, selector: selector)
        if let data = try? JSONEncoder().encode(matches),
           let json = String(data: data, encoding: .utf8) {
            print(json)
        }
    }
    
    static func click(appName: String, selectorJSON: String) {
        guard let selector = parseSelector(selectorJSON) else {
            printError("Invalid selector JSON")
            exit(1)
        }
        
        let driver = AXDriver()
        guard let (appElement, pid) = driver.connect(appName: appName) else {
            printError("App '\(appName)' not running")
            exit(1)
        }
        
        let action = AutomationAction(action: .click, selector: selector)
        let result = driver.perform(appName: appName, appElement: appElement, pid: pid, action: action)
        
        if let data = try? JSONEncoder().encode(result),
           let json = String(data: data, encoding: .utf8) {
            print(json)
        }
        
        if !result.success {
            exit(1)
        }
    }
    
    static func typeText(appName: String, selectorJSON: String, text: String) {
        guard let selector = parseSelector(selectorJSON) else {
            printError("Invalid selector JSON")
            exit(1)
        }
        
        let driver = AXDriver()
        guard let (appElement, pid) = driver.connect(appName: appName) else {
            printError("App '\(appName)' not running")
            exit(1)
        }
        
        let action = AutomationAction(
            action: .setValue,
            selector: selector,
            params: ["text": AnyCodableValue(text)]
        )
        let result = driver.perform(appName: appName, appElement: appElement, pid: pid, action: action)
        
        if let data = try? JSONEncoder().encode(result),
           let json = String(data: data, encoding: .utf8) {
            print(json)
        }
        
        if !result.success {
            exit(1)
        }
    }
    
    static func runScript(path: String) async {
        guard let data = FileManager.default.contents(atPath: path) else {
            printError("Cannot read script file: \(path)")
            exit(1)
        }
        
        guard let script = try? JSONDecoder().decode(AutomationScript.self, from: data) else {
            printError("Invalid script JSON format")
            exit(1)
        }
        
        let runner = ScriptRunner()
        let results = await runner.run(script: script)
        
        if let outputData = try? JSONEncoder().encode(results),
           let json = String(data: outputData, encoding: .utf8) {
            print(json)
        }
        
        let failed = results.contains { !$0.success }
        exit(failed ? 1 : 0)
    }
    
    static func parseSelector(_ json: String) -> AutomationSelector? {
        guard let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(AutomationSelector.self, from: data)
    }
}

// MARK: - Script Models

struct AutomationScript: Codable {
    let name: String
    let app: String
    let dryRun: Bool?
    let steps: [ScriptStep]
}

struct ScriptStep: Codable {
    let action: String
    let selector: AutomationSelector?
    let params: [String: AnyCodableValue]?
    let ms: Int?
}

struct StepResult: Codable {
    let step: Int
    let action: String
    let success: Bool
    let message: String
    let durationMs: Int
}

// MARK: - Script Runner

actor ScriptRunner {
    private let driver = AXDriver()
    
    func run(script: AutomationScript) async -> [StepResult] {
        var results: [StepResult] = []
        
        guard let (appElement, pid) = driver.connect(appName: script.app) else {
            return [StepResult(step: 0, action: "connect", success: false, message: "App not running", durationMs: 0)]
        }
        
        for (index, step) in script.steps.enumerated() {
            let start = Date()
            let result: StepResult
            
            switch step.action {
            case "observe":
                let snapshot = driver.observe(appName: script.app, appElement: appElement, pid: pid)
                result = StepResult(
                    step: index,
                    action: "observe",
                    success: true,
                    message: "Found \(snapshot.elements.count) elements",
                    durationMs: Int(Date().timeIntervalSince(start) * 1000)
                )
                
            case "click":
                guard let selector = step.selector else {
                    result = StepResult(step: index, action: "click", success: false, message: "Missing selector", durationMs: 0)
                    results.append(result)
                    continue
                }
                let action = AutomationAction(action: .click, selector: selector)
                let response = driver.perform(appName: script.app, appElement: appElement, pid: pid, action: action)
                result = StepResult(
                    step: index,
                    action: "click",
                    success: response.success,
                    message: response.message,
                    durationMs: Int(Date().timeIntervalSince(start) * 1000)
                )
                
            case "type":
                guard let selector = step.selector,
                      let text = step.params?["text"]?.value as? String else {
                    result = StepResult(step: index, action: "type", success: false, message: "Missing selector or text", durationMs: 0)
                    results.append(result)
                    continue
                }
                let action = AutomationAction(action: .setValue, selector: selector, params: ["text": AnyCodableValue(text)])
                let response = driver.perform(appName: script.app, appElement: appElement, pid: pid, action: action)
                result = StepResult(
                    step: index,
                    action: "type",
                    success: response.success,
                    message: response.message,
                    durationMs: Int(Date().timeIntervalSince(start) * 1000)
                )
                
            case "wait":
                let ms = step.ms ?? 1000
                try? await Task.sleep(nanoseconds: UInt64(ms) * 1_000_000)
                result = StepResult(
                    step: index,
                    action: "wait",
                    success: true,
                    message: "Waited \(ms)ms",
                    durationMs: ms
                )
                
            case "query":
                guard let selector = step.selector else {
                    result = StepResult(step: index, action: "query", success: false, message: "Missing selector", durationMs: 0)
                    results.append(result)
                    continue
                }
                let matches = driver.find(appName: script.app, appElement: appElement, pid: pid, selector: selector)
                result = StepResult(
                    step: index,
                    action: "query",
                    success: !matches.isEmpty,
                    message: "Found \(matches.count) matches",
                    durationMs: Int(Date().timeIntervalSince(start) * 1000)
                )
                
            default:
                result = StepResult(
                    step: index,
                    action: step.action,
                    success: false,
                    message: "Unknown action",
                    durationMs: 0
                )
            }
            
            results.append(result)
            
            // Stop on failure unless continuing is specified
            if !result.success {
                break
            }
        }
        
        return results
    }
}
