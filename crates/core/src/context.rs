use std::path::Path;

const CONTEXT_FILES: &[&str] = &[
    "AGENTS.md",
    "AGENTS.local.md",
    "README.md",
    "README.local.md",
    "CRUSH.md",
    "CRUSH.local.md",
    "CLAUDE.md",
    "CLAUDE.local.md",
    "GEMINI.md",
    "GEMINI.local.md",
    ".github/copilot-instructions.md",
    ".cursorrules",
    ".cursor/rules/default.mdc",
    ".windsurfrules",
    ".onicode/instructions.md",
    ".onicode/context.md",
];

pub fn load_project_context(workspace_root: &Path) -> Option<String> {
    let mut contexts = Vec::new();

    for file in CONTEXT_FILES {
        let path = workspace_root.join(file);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                contexts.push(format!("### {file}\n\n{content}"));
            }
        }
    }

    if contexts.is_empty() {
        None
    } else {
        Some(contexts.join("\n\n---\n\n"))
    }
}
