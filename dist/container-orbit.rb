# Homebrew formula for orbit (container-orbit).
#
# This is a reference template. The release workflow (.github/workflows/release.yml)
# regenerates it in the slothlabsorg/homebrew-tap repo on each tagged release with
# the real version + sha256 values. Users install with:
#
#     brew install slothlabsorg/tap/container-orbit
#
class ContainerOrbit < Formula
  desc "Delegate Docker to a beefier LAN machine over SSH, with automatic port forwarding"
  homepage "https://slothlabs.org/container-orbit"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/slothlabsorg/container-orbit/releases/download/v0.1.0/orbit-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACED_BY_CI"
    end
    on_intel do
      url "https://github.com/slothlabsorg/container-orbit/releases/download/v0.1.0/orbit-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACED_BY_CI"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/slothlabsorg/container-orbit/releases/download/v0.1.0/orbit-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACED_BY_CI"
    end
    on_intel do
      url "https://github.com/slothlabsorg/container-orbit/releases/download/v0.1.0/orbit-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACED_BY_CI"
    end
  end

  def install
    bin.install "orbit"
  end

  test do
    assert_match "orbit", shell_output("#{bin}/orbit --version")
  end
end
