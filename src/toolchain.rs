//! Toolchain auto-detection for projects
//!
//! Detects project type and required tools by analyzing project files.

use std::path::Path;

/// Detected toolchain information
#[derive(Debug, Clone, Default)]
pub struct Toolchain {
    /// Detected tools and their install commands
    pub tools: Vec<Tool>,
}

/// A single tool that should be available in the container
#[derive(Debug, Clone)]
pub struct Tool {
    /// Tool name (for display)
    pub name: &'static str,
    /// Install command(s) to run in container
    pub install_cmd: &'static str,
    /// Check command to verify installation (reserved for future use)
    #[allow(dead_code)]
    pub check_cmd: &'static str,
}

impl Toolchain {
    /// Detect toolchain from project directory
    pub fn detect(project_path: &Path) -> Self {
        let mut tools = Vec::new();

        // Rust detection
        if let Some(tool) = detect_rust(project_path) {
            tools.push(tool);
        }

        // Node.js / JavaScript detection
        if let Some(tool) = detect_node(project_path) {
            tools.push(tool);
        }

        // Python detection
        if let Some(tool) = detect_python(project_path) {
            tools.push(tool);
        }

        // Go detection
        if let Some(tool) = detect_go(project_path) {
            tools.push(tool);
        }

        // Moon/Proto detection (monorepo tooling)
        if let Some(tool) = detect_moon_proto(project_path) {
            tools.push(tool);
        }

        // Turbo detection (monorepo)
        if let Some(tool) = detect_turbo(project_path) {
            tools.push(tool);
        }

        // Deno detection
        if let Some(tool) = detect_deno(project_path) {
            tools.push(tool);
        }

        // Java/Kotlin detection
        if let Some(tool) = detect_java(project_path) {
            tools.push(tool);
        }

        // Ruby detection
        if let Some(tool) = detect_ruby(project_path) {
            tools.push(tool);
        }

        // PHP detection
        if let Some(tool) = detect_php(project_path) {
            tools.push(tool);
        }

        // Elixir detection
        if let Some(tool) = detect_elixir(project_path) {
            tools.push(tool);
        }

        // Zig detection
        if let Some(tool) = detect_zig(project_path) {
            tools.push(tool);
        }

        Toolchain { tools }
    }

    /// Generate shell commands to install all detected tools
    pub fn install_commands(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.install_cmd).collect()
    }

    /// Check if any tools were detected
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Get tool names for display
    pub fn tool_names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.name).collect()
    }
}

// === Detection functions ===

fn detect_rust(path: &Path) -> Option<Tool> {
    let indicators = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "rust-toolchain",
    ];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "Rust",
            install_cmd: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . $HOME/.cargo/env",
            check_cmd: "rustc --version",
        })
    } else {
        None
    }
}

fn detect_node(path: &Path) -> Option<Tool> {
    // Check for package manager lock files to determine which one to use
    if path.join("bun.lockb").exists() || path.join("bun.lock").exists() {
        return Some(Tool {
            name: "Bun",
            install_cmd:
                "curl -fsSL https://bun.sh/install | bash && export PATH=$HOME/.bun/bin:$PATH",
            check_cmd: "bun --version",
        });
    }

    if path.join("pnpm-lock.yaml").exists() {
        return Some(Tool {
            name: "pnpm",
            install_cmd: "curl -fsSL https://get.pnpm.io/install.sh | sh - && export PNPM_HOME=$HOME/.local/share/pnpm && export PATH=$PNPM_HOME:$PATH",
            check_cmd: "pnpm --version",
        });
    }

    if path.join("yarn.lock").exists() {
        return Some(Tool {
            name: "Yarn",
            install_cmd: "corepack enable && corepack prepare yarn@stable --activate",
            check_cmd: "yarn --version",
        });
    }

    // Default to npm if package.json exists
    let indicators = [
        "package.json",
        "package-lock.json",
        ".nvmrc",
        ".node-version",
    ];
    if indicators.iter().any(|f| path.join(f).exists()) {
        return Some(Tool {
            name: "Node.js",
            install_cmd: "curl -fsSL https://fnm.vercel.app/install | bash && export PATH=$HOME/.local/share/fnm:$PATH && eval \"$(fnm env)\" && fnm install --lts",
            check_cmd: "node --version",
        });
    }

    None
}

fn detect_python(path: &Path) -> Option<Tool> {
    // Check for uv first (modern Python package manager)
    if path.join("uv.lock").exists() || path.join("uv.toml").exists() {
        return Some(Tool {
            name: "uv",
            install_cmd: "curl -LsSf https://astral.sh/uv/install.sh | sh && export PATH=$HOME/.local/bin:$PATH",
            check_cmd: "uv --version",
        });
    }

    // Check for poetry
    if path.join("poetry.lock").exists() || path.join("poetry.toml").exists() {
        return Some(Tool {
            name: "Poetry",
            install_cmd: "curl -sSL https://install.python-poetry.org | python3 - && export PATH=$HOME/.local/bin:$PATH",
            check_cmd: "poetry --version",
        });
    }

    // Check for pipenv
    if path.join("Pipfile").exists() || path.join("Pipfile.lock").exists() {
        return Some(Tool {
            name: "Pipenv",
            install_cmd: "pip install --user pipenv && export PATH=$HOME/.local/bin:$PATH",
            check_cmd: "pipenv --version",
        });
    }

    // Default Python detection
    let indicators = [
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "requirements.txt",
        "requirements-dev.txt",
        ".python-version",
        "tox.ini",
        "Makefile", // Many Python projects use Makefiles
    ];

    // Check for .py files in root
    let has_py_files = path
        .read_dir()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().map(|ext| ext == "py").unwrap_or(false))
        })
        .unwrap_or(false);

    if indicators.iter().any(|f| path.join(f).exists()) || has_py_files {
        // Prefer uv for new projects as it's faster
        return Some(Tool {
            name: "Python (uv)",
            install_cmd: "curl -LsSf https://astral.sh/uv/install.sh | sh && export PATH=$HOME/.local/bin:$PATH",
            check_cmd: "uv --version && python3 --version",
        });
    }

    None
}

fn detect_go(path: &Path) -> Option<Tool> {
    let indicators = ["go.mod", "go.sum", "go.work"];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "Go",
            install_cmd: "curl -fsSL https://go.dev/dl/go1.22.0.linux-amd64.tar.gz | tar -C /usr/local -xzf - && export PATH=$PATH:/usr/local/go/bin:$HOME/go/bin",
            check_cmd: "go version",
        })
    } else {
        None
    }
}

fn detect_moon_proto(path: &Path) -> Option<Tool> {
    // Moon workspace detection
    let moon_indicators = [".moon/workspace.yml", ".moon/toolchain.yml", "moon.yml"];

    // Proto toolchain detection
    let proto_indicators = [".prototools", ".proto/config.toml"];

    if moon_indicators.iter().any(|f| path.join(f).exists()) {
        return Some(Tool {
            name: "moon",
            install_cmd: "curl -fsSL https://moonrepo.dev/install/moon.sh | bash && export PATH=$HOME/.moon/bin:$PATH",
            check_cmd: "moon --version",
        });
    }

    if proto_indicators.iter().any(|f| path.join(f).exists()) {
        return Some(Tool {
            name: "proto",
            install_cmd: "curl -fsSL https://moonrepo.dev/install/proto.sh | bash && export PATH=$HOME/.proto/bin:$PATH",
            check_cmd: "proto --version",
        });
    }

    None
}

fn detect_turbo(path: &Path) -> Option<Tool> {
    let indicators = ["turbo.json", ".turbo"];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "Turborepo",
            // Turbo is typically installed via npm, but we can also install globally
            install_cmd: "npm install -g turbo",
            check_cmd: "turbo --version",
        })
    } else {
        None
    }
}

fn detect_deno(path: &Path) -> Option<Tool> {
    let indicators = ["deno.json", "deno.jsonc", "deno.lock", "mod.ts", "deps.ts"];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "Deno",
            install_cmd:
                "curl -fsSL https://deno.land/install.sh | sh && export PATH=$HOME/.deno/bin:$PATH",
            check_cmd: "deno --version",
        })
    } else {
        None
    }
}

fn detect_java(path: &Path) -> Option<Tool> {
    let indicators = [
        "pom.xml",          // Maven
        "build.gradle",     // Gradle
        "build.gradle.kts", // Gradle Kotlin DSL
        "settings.gradle",
        "settings.gradle.kts",
        ".java-version",
        "mvnw",
        "gradlew",
    ];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "Java (SDKMAN)",
            install_cmd: "curl -s https://get.sdkman.io | bash && source $HOME/.sdkman/bin/sdkman-init.sh && sdk install java",
            check_cmd: "java --version",
        })
    } else {
        None
    }
}

fn detect_ruby(path: &Path) -> Option<Tool> {
    let indicators = [
        "Gemfile",
        "Gemfile.lock",
        ".ruby-version",
        ".ruby-gemset",
        "Rakefile",
        "*.gemspec",
    ];

    // Special handling for gemspec pattern
    let has_gemspec = path
        .read_dir()
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "gemspec")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if indicators[..5].iter().any(|f| path.join(f).exists()) || has_gemspec {
        Some(Tool {
            name: "Ruby",
            install_cmd: "curl -fsSL https://github.com/rbenv/rbenv-installer/raw/HEAD/bin/rbenv-installer | bash && export PATH=$HOME/.rbenv/bin:$PATH && eval \"$(rbenv init -)\" && rbenv install -s && rbenv global $(rbenv install -l | grep -v - | tail -1)",
            check_cmd: "ruby --version",
        })
    } else {
        None
    }
}

fn detect_php(path: &Path) -> Option<Tool> {
    let indicators = ["composer.json", "composer.lock", "artisan", ".php-version"];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "PHP",
            install_cmd: "apt-get update && apt-get install -y php php-cli php-mbstring php-xml php-curl && curl -sS https://getcomposer.org/installer | php -- --install-dir=/usr/local/bin --filename=composer",
            check_cmd: "php --version && composer --version",
        })
    } else {
        None
    }
}

fn detect_elixir(path: &Path) -> Option<Tool> {
    let indicators = ["mix.exs", "mix.lock", ".tool-versions"];

    // Check for .tool-versions containing elixir
    let has_elixir_in_tool_versions = path.join(".tool-versions").exists()
        && std::fs::read_to_string(path.join(".tool-versions"))
            .map(|content| content.contains("elixir"))
            .unwrap_or(false);

    if indicators[..2].iter().any(|f| path.join(f).exists()) || has_elixir_in_tool_versions {
        Some(Tool {
            name: "Elixir",
            install_cmd: "apt-get update && apt-get install -y erlang elixir",
            check_cmd: "elixir --version",
        })
    } else {
        None
    }
}

fn detect_zig(path: &Path) -> Option<Tool> {
    let indicators = ["build.zig", "build.zig.zon"];

    if indicators.iter().any(|f| path.join(f).exists()) {
        Some(Tool {
            name: "Zig",
            install_cmd: "curl -fsSL https://ziglang.org/download/0.11.0/zig-linux-x86_64-0.11.0.tar.xz | tar -xJ -C /usr/local && export PATH=$PATH:/usr/local/zig-linux-x86_64-0.11.0",
            check_cmd: "zig version",
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();

        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.tool_names().contains(&"Rust"));
    }

    #[test]
    fn test_detect_node_npm() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();

        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.tool_names().contains(&"Node.js"));
    }

    #[test]
    fn test_detect_node_bun() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("bun.lockb"), "").unwrap();

        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.tool_names().contains(&"Bun"));
    }

    #[test]
    fn test_detect_python_uv() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("uv.lock"), "").unwrap();

        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.tool_names().contains(&"uv"));
    }

    #[test]
    fn test_detect_moon() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".moon")).unwrap();
        fs::write(dir.path().join(".moon/workspace.yml"), "").unwrap();

        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.tool_names().contains(&"moon"));
    }

    #[test]
    fn test_detect_multiple() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();

        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.tools.len() >= 2);
    }

    #[test]
    fn test_empty_detection() {
        let dir = TempDir::new().unwrap();
        let toolchain = Toolchain::detect(dir.path());
        assert!(toolchain.is_empty());
    }
}
