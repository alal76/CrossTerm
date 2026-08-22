cask "crossterm" do
  version "1.0.0"
  sha256 "PLACEHOLDER_SHA256"

  url "https://github.com/alal76/CrossTerm/releases/download/v#{version}/CrossTerm_#{version}_aarch64.dmg"
  name "CrossTerm"
  desc "Cross-platform terminal emulator and remote access suite"
  homepage "https://github.com/alal76/CrossTerm"

  livecheck do
    url :url
    strategy :github_latest
  end

  app "CrossTerm.app"

  # Must match src-tauri/tauri.conf.json's "identifier" (com.crossterm.desktop),
  # not the cask's own token ("crossterm") or app name — macOS keys support/
  # preference/cache paths by bundle identifier, and this previously pointed
  # at "com.crossterm.app", which was never the app's real identifier.
  zap trash: [
    "~/Library/Application Support/com.crossterm.desktop",
    "~/Library/Preferences/com.crossterm.desktop.plist",
    "~/Library/Caches/com.crossterm.desktop",
  ]
end
