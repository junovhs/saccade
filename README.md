# 👁️ Saccade

**Give your AI eyes. The sensory organ for your codebase.**

Saccade is a command-line tool that intelligently scans your repository and creates a hyper-efficient, multi-layered "context pack" for Large Language Models. It enables AIs to understand, debug, and modify complex codebases with a fraction of the token cost of traditional methods.

---

*[Planned: Cool animated GIF here showing `saccade` running, the staged output printing, and the final summary table.]*

---

### The Problem: LLMs Are Blind

Large Language Models are incredibly powerful, but they operate with a fundamental handicap: they have no "vision." When you paste an entire codebase into their context window, they are forced to "read" every single word, from the most critical architectural interface to the last byte of a PNG file. This is slow, expensive, and deeply inefficient.

### The Solution: A Vision-Inspired Workflow

Saccade mimics the way human vision works. It doesn't just dump data; it performs an intelligent, multi-stage scan to build a rich, layered understanding of your project.

1.  **[SACCADE] The Initial Scan:** The moment you run it, `saccade` performs a quick, low-cost scan to identify the project's key features—what languages are used, where the source code lives, and what the dependency structure looks like.

2.  **[STAGE 0] Peripheral Vision:** It first generates a low-resolution, global overview of your project.
    *   `STRUCTURE.txt`: A map of your key directories and files.
    *   `TOKENS.txt`: A "heat map" to show where the code density is highest.
    *   **Result:** The AI gets a "feel" for the project landscape in seconds, using only a handful of tokens.

3.  **[STAGE 1] Feature Detection:** Next, it identifies the "edges and contours" of your code.
    *   `DEPS.txt`: The dependency graph—the project's structural skeleton.
    *   `API.txt`: The public API surfaces—the boundaries and contracts of your code.
    *   **Result:** The AI understands how components connect and interact, without needing to see inside them.

4.  **[STAGE 2] Focused Gaze:** Finally, it uses the power of Tree-sitter to generate a compressed "code skeleton."
    *   `PACK_STAGE2_COMPRESSED.xml`: An architectural blueprint of your most important code, preserving imports, signatures, and types, while completely removing the token-heavy implementation details.
    *   **Result:** The AI gets a high-resolution model of your project's architecture, enabling it to reason about logic and write new code that fits perfectly.

The entire process is governed by the **Ask-for-Files Protocol**, which teaches the AI to request full source code on demand, just like a human developer would look up a specific file.

### Features

-   **Intelligent Project Detection:** Automatically identifies Rust, JavaScript/TypeScript, Python, Go, and more.
-   **Extreme Token Efficiency:** Creates context packs that are often 10-50x smaller than the raw source.
-   **Multi-Language Support:** The core compression logic is powered by Tree-sitter, supporting dozens of languages out of the box.
-   **Staged Context:** Provides a layered view of your codebase, from a high-level overview to a detailed architectural skeleton.
-   **Zero-Dependency Script:** The core script runs anywhere Bash is present, with no mandatory installs.

### Getting Started

#### Installation

Since `saccade` is a powerful shell script, you can install it by simply cloning this repository and adding the script to your path.

```sh
# Clone the repository
git clone https://github.com/junovhs/saccade.git

# Go into the directory
cd saccade

# Optional: Make it runnable from anywhere
# Add this line to your .bashrc, .zshrc, or equivalent shell profile
export PATH=$PATH:$(pwd)
```

#### First Run

Navigate to any of your project directories and simply run:

```sh
saccade
```

This will create an `ai-pack/` folder in that directory containing the complete, staged context pack for your project.

### The Saccade Pack: What's Inside?

When you run `saccade`, it produces the following artifacts in the `ai-pack/` directory:

-   `OVERVIEW.md`: A high-level, human-written summary of the project. (You should edit this once!)
-   `STRUCTURE.txt`: The directory tree.
-   `TOKENS.txt`: A "heat map" of the largest files.
-   `DEPS.txt`: The project's dependency graph.
-   `API.txt`: The project's public API surface.
-   `PACK_STAGE2_COMPRESSED.xml`: The compressed architectural skeleton (if Repomix is installed).
-   `REQUEST_PROTOCOL.md`: The instructions that teach the AI how to ask for more files.

### The Saccade + ApplyDiff Ecosystem

Saccade is designed to work in a perfect loop with its sister tool, **ApplyDiff**.

1.  **👁️ Saccade (The Eyes):** Scans your codebase and creates the context for the AI.
2.  **🧠 The AI (The Brain):** Analyzes the context and generates a patch.
3.  **🖐️ ApplyDiff (The Hands):** Takes the AI-generated patch and safely applies it to your codebase.

Together, they form a complete, end-to-end workflow for AI-assisted development.

### Contributing

This is a new project, and contributions are welcome! Please see `CONTRIBUTING.md` for details on how to report bugs, suggest features, or submit code.

### License

This project is licensed under the MIT License. See the `LICENSE` file for details.
