cask "norn" do
  version "0.1.0"
  sha256 :no_check

  url "https://github.com/delaudio/norn/releases/download/v#{version}/Norn-#{version}-universal.dmg"
  name "Norn"
  desc "Local-first pull request review with AI-assisted workflow"
  homepage "https://github.com/delaudio/norn"

  app "Norn.app"

  uninstall quit: "app.norn.desktop"

  auto_updates false

  test do
    assert_predicate "#{appdir}/Norn.app", :exist?
  end
end
