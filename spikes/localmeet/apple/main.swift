// Apple on-device SpeechAnalyzer/SpeechTranscriber benchmark.
// usage: applestt <locale> <wav>...   prints one JSON line per file
import AVFoundation
import Foundation
import Speech

@main
struct Main {
    static func main() async throws {
        let args = CommandLine.arguments
        let locale = Locale(identifier: args[1])
        let probe = SpeechTranscriber(locale: locale, preset: .transcription)
        if let req = try await AssetInventory.assetInstallationRequest(supporting: [probe]) {
            FileHandle.standardError.write("installing assets for \(args[1])\n".data(using: .utf8)!)
            try await req.downloadAndInstall()
        }
        for path in args.dropFirst(2) {
            let url = URL(fileURLWithPath: path)
            let file = try AVAudioFile(forReading: url)
            let secs = Double(file.length) / file.processingFormat.sampleRate
            let t0 = Date()
            let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
            let analyzer = SpeechAnalyzer(modules: [transcriber])
            let collect = Task {
                var s = ""
                for try await r in transcriber.results { s += String(r.text.characters) }
                return s
            }
            if let last = try await analyzer.analyzeSequence(from: file) {
                try await analyzer.finalizeAndFinish(through: last)
            } else {
                await analyzer.cancelAndFinishNow()
            }
            let text = try await collect.value
            let dt = Date().timeIntervalSince(t0)
            let obj: [String: Any] = ["file": url.lastPathComponent, "secs": secs, "time": dt, "text": text]
            let data = try JSONSerialization.data(withJSONObject: obj)
            print(String(data: data, encoding: .utf8)!)
            fflush(stdout)
        }
    }
}
