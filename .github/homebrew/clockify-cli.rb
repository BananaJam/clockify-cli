class ClockifyCli < Formula
  desc "Fast terminal client for Clockify time tracking with CLI, TUI, and prompts"
  homepage "https://github.com/BananaJam/clockify-cli"
  version "{{VERSION}}"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/BananaJam/clockify-cli/releases/download/v#{version}/clockify-cli-aarch64-apple-darwin.tar.xz"
      sha256 "{{SHA_AARCH64_APPLE_DARWIN}}"
    end
    on_intel do
      url "https://github.com/BananaJam/clockify-cli/releases/download/v#{version}/clockify-cli-x86_64-apple-darwin.tar.xz"
      sha256 "{{SHA_X86_64_APPLE_DARWIN}}"
    end
  end
  on_linux do
    on_arm do
      url "https://github.com/BananaJam/clockify-cli/releases/download/v#{version}/clockify-cli-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "{{SHA_AARCH64_UNKNOWN_LINUX_GNU}}"
    end
    on_intel do
      url "https://github.com/BananaJam/clockify-cli/releases/download/v#{version}/clockify-cli-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "{{SHA_X86_64_UNKNOWN_LINUX_GNU}}"
    end
  end

  def install
    bin.install "clockify"

    mandir = buildpath/"man"
    system bin/"clockify", "man", "--dir", mandir
    man1.install Dir[mandir/"*.1"]

    generate_completions_from_executable(bin/"clockify", "completions")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/clockify --version")
  end
end
