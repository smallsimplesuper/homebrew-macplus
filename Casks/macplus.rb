cask "macplus" do
  version "0.2.27"
  sha256 "26dd36b469d92df3c9a2fceede5ca406051fe8a2a4b3a0c9667ac5c9d7f5181b"

  url "https://github.com/smallsimplesuper/homebrew-macplus/releases/download/v#{version}/macPlus_#{version}_universal.dmg"
  name "macPlus"
  desc "Fast, native macOS app update manager"
  homepage "https://github.com/smallsimplesuper/homebrew-macplus"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :ventura"

  app "macPlus.app"

  zap trash: [
    "~/Library/Application Support/com.macplus.app",
    "~/Library/Caches/com.macplus.app",
    "~/Library/Preferences/com.macplus.app.plist",
    "~/Library/Logs/com.macplus.app",
    "~/Library/LaunchAgents/com.macplus.app.plist",
    "~/Library/WebKit/com.macplus.app",
  ]
end
