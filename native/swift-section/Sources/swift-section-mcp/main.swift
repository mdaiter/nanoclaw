import SwiftSectionMCPCore

@main
struct SwiftSectionMCPApp {
    static func main() async {
        await MCPServer().run()
    }
}
