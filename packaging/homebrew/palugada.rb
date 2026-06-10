# Homebrew formula template. The Release workflow renders the @TOKENS@ (version
# + per-archive sha256) and pushes the result to the homebrew-tap repo.
# Users: `brew install yudistirosaputro/tap/palugada`.
class Palugada < Formula
  desc "Project-agnostic developer knowledge & connector CLI"
  homepage "https://github.com/yudistirosaputro/palugada-cli"
  license "MIT"
  version "@VERSION@"

  on_macos do
    on_arm do
      url "https://github.com/yudistirosaputro/palugada-cli/releases/download/v@VERSION@/palugada-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA_DARWIN_ARM64@"
    end
    on_intel do
      url "https://github.com/yudistirosaputro/palugada-cli/releases/download/v@VERSION@/palugada-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA_DARWIN_X64@"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/yudistirosaputro/palugada-cli/releases/download/v@VERSION@/palugada-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA_LINUX_X64@"
    end
  end

  def install
    # Keep the binary next to its knowledge/ profiles; symlink onto PATH.
    # palugada canonicalizes its own path, so the symlink resolves correctly.
    libexec.install "palugada"
    libexec.install "knowledge"
    libexec.install "examples" if File.exist?("examples")
    bin.install_symlink libexec/"palugada"
  end

  test do
    assert_match "palugada", shell_output("#{bin}/palugada --help")
  end
end
