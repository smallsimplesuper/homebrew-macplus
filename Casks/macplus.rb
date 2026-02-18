cask "macplus" do
  version "0.2.45"
  sha256 "d6a044c09b238ae77abefac6f4d9415eea7293f3100364d1312bc55cb5e03f53"

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
