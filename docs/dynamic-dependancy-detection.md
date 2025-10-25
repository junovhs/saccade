### **A Biomimetic, Multi-Layered Heuristic Engine for Dynamic Dependency Manifest Detection**

**Abstract:**
This paper specifies the architecture and implementation of a biomimetic, multi-layered heuristic engine designed to dynamically identify software dependency manifest files within a repository. Traditional methods relying on static, top-down filename lists (e.g., `package.json`, `Cargo.toml`) fail in non-standardized ecosystems such as C/C++. We propose a bottom-up, content-based classification system that emulates the robust, multi-stage pattern recognition and validation mechanisms found in biological systems, specifically the innate immune system, DNA mismatch repair, and bacterial chemotaxis. The engine is designed as a three-layer computational funnel that processes all files to identify manifest candidates with high precision and computational efficiency. This document provides the architectural principles, algorithmic specifications, implementation prerequisites, and risk analysis necessary for its construction.

---

#### **1. Introduction**

##### **1.1. The Architectural Deficit of Static Detection**
Software Composition Analysis (SCA) is critically dependent on the accurate identification of files that declare project dependencies. The prevalent top-down approach—matching filenames against a hardcoded list—is inherently brittle. It offers zero recall for projects that use custom naming conventions or operate in ecosystems without a standardized manifest filename, a problem endemic to C, C++, and bespoke build environments. This architectural deficit results in a silent failure mode, producing incomplete or empty dependency reports and rendering downstream analysis unreliable.

##### **1.2. The Proposed Solution: A Biomimetic Heuristic Funnel**
We propose a bottom-up classification engine that infers a file's purpose from its intrinsic properties. The system architecture is modeled directly on the principles of biological pattern recognition, which have been optimized over billions of years for efficiency, resilience, and adaptability. The proposed engine is a multi-layered computational funnel designed to:
1.  Rapidly filter a large corpus of files using low-cost lexical analysis.
2.  Apply progressively more sophisticated (and computationally expensive) syntactic and contextual validation to a dwindling set of candidates.
3.  Produce a high-confidence list of manifest files, regardless of their names or the project's ecosystem.

---

#### **2. System Architecture and Algorithmic Mechanisms**

The engine is composed of three sequential layers, each serving as a filter for the next. Each layer is a direct analog of a specific biological mechanism.

##### **2.1. Layer 1: Lexical Filtering (The Innate Immune Response)**
*   **Biological Analog:** The innate immune system's Pattern Recognition Receptors (PRRs) perform a fast, stateless scan for Pathogen-Associated Molecular Patterns (PAMPs)—short, conserved, and essential molecular signatures that signify a "non-self" entity.
*   **Mechanism:** This layer performs a rapid lexical scan on all text-based files to identify candidates based on the density of manifest-specific keywords.
*   **Algorithm Specification:**
    1.  **Prerequisite:** A predefined `const` array of high-signal keywords (`PAMP_KEYWORDS`) that are strongly indicative of build or dependency management logic.
    2.  **Entropy Pre-Filter:** For each file, calculate its Shannon entropy. If the entropy is above a certain threshold (e.g., > 6.0), classify it as a binary/compressed file and immediately exclude it from further analysis.
    3.  **Manifest Density Score (MDS) Calculation:** For each remaining file, calculate its MDS:
        ```
        MDS = (count_of_unique_pamp_keywords_found) / (total_lines_in_file + 1)
        ```
        The score is normalized by line count to prevent large source files with incidental keyword mentions from scoring highly.
    4.  **Thresholding:** If a file's `MDS` is below a configurable `MDS_CANDIDACY_THRESHOLD` (e.g., 0.05), it is rejected.
*   **Output:** A small list of `HeuristicCandidate` structs, each containing a file path and its MDS score, to be passed to Layer 2.

##### **2.2. Layer 2: Syntactic Validation (DNA Mismatch Repair & Protein Folding)**
*   **Biological Analog:** DNA Mismatch Repair (MMR) enzymes perform a second, context-aware pass over replicated DNA to find structural errors missed by the initial proofreading. This is analogous to validating that keywords are not just present, but are used in a syntactically correct structure, similar to how molecular chaperones validate a protein's 3D fold.
*   **Mechanism:** This layer applies Abstract Syntax Tree (AST) analysis to the candidates from Layer 1 to validate their structural integrity. It also implements a "rejection cache" inspired by the ERAD (Endoplasmic Reticulum-Associated Degradation) pathway for misfolded proteins.
*   **Algorithm Specification:**
    1.  **Prerequisite:** A set of simple, language-agnostic Tree-sitter queries that identify common manifest structures (e.g., function calls with specific names, key-value pairs with specific keys).
    2.  **Structural Validation Function (`validate_structure`):** For each `HeuristicCandidate`, attempt to parse the file using Tree-sitter. Run a battery of structural queries against the resulting AST. A file is considered valid if it matches at least one structural query.
    3.  **ERAD Rejection Cache:** If `validate_structure` returns `false`, the file's path is added to a `HashSet<PathBuf>` named `erad_cache`. The file is rejected and will be skipped instantly in future analyses within the same run.
*   **Output:** A list of candidates that have been validated for syntactic correctness.

##### **2.3. Layer 3: Contextual Scoring (Bacterial Chemotaxis)**
*   **Biological Analog:** Bacterial chemotaxis allows a bacterium to navigate by sensing chemical gradients, making decisions based on its spatial and temporal context, not just its immediate state.
*   **Mechanism:** This layer refines a candidate's score based on its location within the repository's filesystem, a powerful contextual clue.
*   **Algorithm Specification:**
    1.  **Prerequisite:** A configurable list of path-based scoring rules (e.g., regex patterns and associated score multipliers).
    2.  **Context Score Function (`get_context_score`):** For each candidate that passes Layer 2, analyze its file path.
        *   Apply a score multiplier > 1.0 for paths matching common build locations (e.g., project root, `/build`, `/cmake`).
        *   Apply a score multiplier < 1.0 for paths matching documentation or asset locations (e.g., `/docs`, `/assets`, `/img`).
    3.  **Final Confidence Score:** Calculate a `FinalScore = HeuristicCandidate.MDS * get_context_score(path)`.
*   **Output:** The final list of manifest files, ranked by their `FinalScore`. Files exceeding a final `CONFIDENCE_THRESHOLD` are embedded in the `DEPS.txt` output.

---

#### **3. Implementation Prerequisites**

*   **3.1. Toolchain and Crates:**
    *   Rust toolchain (latest stable).
    *   `cargo` build system.
    *   Required crates: `tree-sitter`, `regex`, `walkdir`.
*   **3.2. Core Data Structures:**
    ```rust
    struct HeuristicCandidate {
        path: PathBuf,
        mds_score: f32,
    }

    // A central cache for files that fail syntactic validation to avoid re-processing.
    type EradCache = std::collections::HashSet<PathBuf>;
    ```
*   **3.3. Configuration and Tunable Parameters:** These MUST be externalized in the `config.rs` module for tuning without a recompile.
    *   `PAMP_KEYWORDS: &[&str]`
    *   `ENTROPY_THRESHOLD: f64`
    *   `MDS_CANDIDACY_THRESHOLD: f32`
    *   `PATH_CONTEXT_RULES: &[(Regex, f32)]`
    *   `FINAL_CONFIDENCE_THRESHOLD: f32`
    *   `AST_VALIDATION_QUERIES: &[&str]`

---

#### **4. Risk Analysis and Mitigation**

*   **4.1. Risk: False Positives (Type I Error).** A source file or documentation with high keyword density could be misidentified as a manifest.
    *   **Mitigation:** The multi-layer design is the primary mitigation. Layer 2 (Syntactic Validation) is specifically designed to reject files that have the right words but the wrong structure. Fine-tuning the `MDS_CANDIDACY_THRESHOLD` and AST queries will be critical.
*   **4.2. Risk: False Negatives (Type II Error).** A legitimate, non-standard manifest could be missed.
    *   **Mitigation:** The comprehensiveness of the `PAMP_KEYWORDS` list is key. This system will *augment*, not replace, the existing static filename check. The static check provides a high-recall baseline, while the heuristic engine adds precision and extends coverage.
*   **4.3. Risk: Performance Overhead.** Analyzing every file adds computational cost.
    *   **Mitigation:** The funnel architecture. Layer 1 is extremely fast (string matching and line counting). The expensive AST parsing in Layer 2 is only ever performed on a tiny fraction (<1%) of the total files. The `erad_cache` prevents redundant analysis of failed candidates.
*   **4.4. Risk: Heuristic Brittleness.** Build system patterns and keywords evolve over time.
    *   **Mitigation:** All key heuristics (keywords, thresholds, path rules) must be stored in the central `config.rs` module. This allows for rapid updates to the detection logic without altering the core algorithmic pipeline, ensuring long-term maintainability.

---

#### **5. Conclusion**

The proposed biomimetic engine represents a paradigm shift from static, brittle detection to a dynamic, resilient, and context-aware system. By directly mapping the validated strategies of biological pattern recognition onto a computational funnel, we can build a manifest detection engine that is both universally applicable and computationally efficient. The successful implementation of this architecture will significantly enhance the reliability and reach of Saccade's dependency analysis capabilities, particularly in complex and non-standardized software ecosystems.

---

#### **Appendix A: Initial PAMP Keyword Set for Prototyping**

This set provides a balanced starting point, covering generic terms and system-specific commands. It is expected to be refined during implementation.

```rust
const PAMP_KEYWORDS: &[&str] = &[
    // Generic
    "dependency", "dependencies", "require", "version", "package",
    "packages", "project", "include", "source", "library", "libraries",

    // CMake
    "find_package", "add_library", "add_executable", "target_link_libraries",
    "cmake_minimum_required",

    // Make
    "gcc", "g++", "clang", ".PHONY", "target",

    // Conan / Python
    "conanfile", "self.requires",

    // Java
    "groupId", "artifactId", "implementation", "compile",

    // Node.js
    "devDependencies",
];
```