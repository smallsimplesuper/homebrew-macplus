cask "macplus" do
  version "0.2.51"
  sha256 "83f16694d92d8fd6b66998d85f0ffab2ee12eac40ba995927a46e033cb1ec1b6"

  url "https://github.com/smallsimplesuper/macplus/releases/download/v#{version}/macPlus_#{version}_universal.dmg"
  name "macPlus"
  desc "Fast, native macOS app update manager"
  homepage "https://github.com/smallsimplesuper/macplus"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :ventura"

  # App is not notarized; remove quarantine flag after install to prevent Gatekeeper block
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/macPlus.app"]
  end

  app "macPlus.app"

  caveats <<~EOS
    macPlus is not notarized. If macOS Gatekeeper blocks it, run:
      xattr -cr /Applications/macPlus.app
  EOS

  zap trash: [
    "~/Library/Application Support/com.macplus.app",
    "~/Library/Caches/com.macplus.app",
    "~/Library/Preferences/com.macplus.app.plist",
    "~/Library/Logs/com.macplus.app",
    "~/Library/LaunchAgents/com.macplus.app.plist",
    "~/Library/WebKit/com.macplus.app",
  ]
end
