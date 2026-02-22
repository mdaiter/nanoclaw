#!/usr/bin/env swift

import Foundation
import NaturalLanguage
import Intents
import AppKit

class TextAnalyzerDemo {

    func analyzeText(_ text: String) -> [String: Any] {
        var results: [String: Any] = [:]

        // 1. Language Detection
        let languageRecognizer = NLLanguageRecognizer()
        languageRecognizer.processString(text)
        if let language = languageRecognizer.dominantLanguage {
            results["language"] = language.rawValue
        }

        // 2. Sentiment Analysis
        let sentimentTagger = NLTagger(tagSchemes: [.sentimentScore])
        sentimentTagger.string = text
        let (sentiment, _) = sentimentTagger.tag(at: text.startIndex, unit: .paragraph, scheme: .sentimentScore)
        if let sentimentScore = sentiment {
            results["sentiment"] = Double(sentimentScore.rawValue) ?? 0.0
        }

        // 3. Named Entity Recognition
        var entities: [String] = []
        let tagger = NLTagger(tagSchemes: [.nameType])
        tagger.string = text
        let options: NLTagger.Options = [.omitWhitespace, .omitPunctuation, .joinNames]
        tagger.enumerateTags(in: text.startIndex..<text.endIndex, unit: .word,
                            scheme: .nameType, options: options) { tag, range in
            if let tag = tag {
                let entity = String(text[range])
                entities.append("\(entity) (\(tag.rawValue))")
            }
            return true
        }
        results["entities"] = entities

        // 4. Tokenization
        let tokenizer = NLTokenizer(unit: .word)
        tokenizer.string = text
        var wordCount = 0
        tokenizer.enumerateTokens(in: text.startIndex..<text.endIndex) { _, _ in
            wordCount += 1
            return true
        }
        results["wordCount"] = wordCount

        // 5. Parts of Speech Tagging
        var pos: [String] = []
        let posTagger = NLTagger(tagSchemes: [.lexicalClass])
        posTagger.string = text
        posTagger.enumerateTags(in: text.startIndex..<text.endIndex, unit: .word,
                               scheme: .lexicalClass, options: options) { tag, range in
            if let tag = tag {
                let word = String(text[range])
                pos.append("\(word): \(tag.rawValue)")
            }
            return true
        }
        results["partsOfSpeech"] = pos

        return results
    }

    func createShortcutActivity(for text: String) {
        let activity = NSUserActivity(activityType: "com.demo.analyzeText")
        activity.title = "Analyze Text Sentiment"
        activity.isEligibleForSearch = true
        activity.isEligibleForHandoff = true
        activity.suggestedInvocationPhrase = "Analyze my text"

        // Add user info
        activity.userInfo = ["text": text]

        // Donate to system
        activity.becomeCurrent()

        print("\n✅ Shortcut activity created and donated to Shortcuts app")
        print("   - Activity Type: \(activity.activityType)")
        print("   - Title: \(activity.title ?? "N/A")")
        print("   - Suggested Phrase: '\(activity.suggestedInvocationPhrase ?? "N/A")'")
    }

    func runDemo() {
        print("=== NaturalLanguage + Intents Demo ===\n")

        let testTexts = [
            "This is a neutral statement about macOS frameworks.",
            "I absolutely love working with these amazing APIs! The framework discovery is incredible!",
            "This is terrible and frustrating. Nothing works as expected."
        ]

        for (index, text) in testTexts.enumerated() {
            print("📝 Sample \(index + 1): \"\(text)\"")
            print("─────────────────────────────────────────────────────")

            let results = analyzeText(text)

            if let language = results["language"] as? String {
                print("🌍 Language: \(language)")
            }

            if let sentiment = results["sentiment"] as? Double {
                let emoji = sentiment > 0.3 ? "😊" : (sentiment < -0.3 ? "😞" : "😐")
                print("\(emoji) Sentiment Score: \(String(format: "%.2f", sentiment))")
            }

            if let entities = results["entities"] as? [String], !entities.isEmpty {
                print("🏷️  Named Entities:")
                for entity in entities {
                    print("   - \(entity)")
                }
            }

            if let wordCount = results["wordCount"] as? Int {
                print("📊 Word Count: \(wordCount)")
            }

            if let pos = results["partsOfSpeech"] as? [String], !pos.isEmpty {
                print("🔤 Parts of Speech (first 5):")
                for item in pos.prefix(5) {
                    print("   - \(item)")
                }
            }

            print("\n")
        }

        // Create Shortcuts integration
        createShortcutActivity(for: testTexts[1])

        print("\n=== Demo Complete ===")
        print("Framework APIs used:")
        print("  • NLLanguageRecognizer - language detection")
        print("  • NLTagger - sentiment analysis, NER, POS tagging")
        print("  • NLTokenizer - word tokenization")
        print("  • NSUserActivity - Shortcuts integration")
    }
}

// Run the demo
let demo = TextAnalyzerDemo()
demo.runDemo()
