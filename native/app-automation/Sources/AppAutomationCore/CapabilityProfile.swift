import Foundation

public struct CapabilityProfile: Codable {
    public let appName: String
    public let pid: Int32
    public var axScore: Double
    public var supportsScriptingBridge: Bool
    public var supportsAppleScript: Bool
    public var supportsUrlSchemes: Bool
    public var lastUpdated: String

    public init(
        appName: String,
        pid: Int32,
        axScore: Double,
        supportsScriptingBridge: Bool,
        supportsAppleScript: Bool,
        supportsUrlSchemes: Bool,
        lastUpdated: String
    ) {
        self.appName = appName
        self.pid = pid
        self.axScore = axScore
        self.supportsScriptingBridge = supportsScriptingBridge
        self.supportsAppleScript = supportsAppleScript
        self.supportsUrlSchemes = supportsUrlSchemes
        self.lastUpdated = lastUpdated
    }
}
