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

  zap trash: [
    "~/Library/Application Support/com.crossterm.app",
    "~/Library/Preferences/com.crossterm.app.plist",
    "~/Library/Caches/com.crossterm.app",
  ]
end
