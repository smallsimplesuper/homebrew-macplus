cask "macplus" do
  version "0.2.26"
  sha256 :no_check

  url "https://github.com/smallsimplesuper/macplus/releases/download/v#{version}/macPlus_#{version}_universal.dmg"
  name "macPlus"
  desc "Fast, native macOS app update manager"
  homepage "https://github.com/smallsimplesuper/macplus"

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
