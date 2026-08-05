class Norn < Formula
  desc "Local-first review tooling from command line"
  homepage "https://github.com/lachesi-hq/norn"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/lachesi-hq/norn/releases/download/v#{version}/norn-#{version}-macos-arm64.tar.gz"
      sha256 :no_check
    else
      url "https://github.com/lachesi-hq/norn/releases/download/v#{version}/norn-#{version}-macos-x86_64.tar.gz"
      sha256 :no_check
    end
  end

  def install
    bin.install "norn"
    bin.install "norn-tui"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/norn --version")
    assert_match "norn", shell_output("#{bin}/norn --help")
  end
end
