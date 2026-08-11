//! Static, non-executing validation for governed Agent, Skill and Plugin drafts.

use std::collections::HashSet;
use std::io::{Cursor, Read};

use conductor_domain::{
    DiagnosticSeverity, DraftFile, ResourceDiagnostic, ResourceKind, ResourceValidation,
};

pub const MAX_DRAFT_FILES: usize = 2_000;
pub const MAX_EDITABLE_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_DRAFT_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_IMPORT_ARCHIVE_BYTES: usize = 20 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 2_500;
const MAX_COMPRESSION_RATIO: u64 = 100;

pub fn import_zip(bytes: Vec<u8>) -> Result<Vec<DraftFile>, String> {
    if bytes.is_empty() || bytes.len() > MAX_IMPORT_ARCHIVE_BYTES {
        return Err("ZIP archives must be between 1 byte and 20 MiB.".into());
    }
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| "The uploaded file is not a readable ZIP archive.".to_string())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("The ZIP archive contains too many entries.".into());
    }
    let mut files = Vec::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| "A ZIP entry could not be read.".to_string())?;
        if entry.is_dir() {
            continue;
        }
        if entry.encrypted() {
            return Err("Encrypted ZIP entries are not supported.".into());
        }
        if entry.is_symlink() || !entry.is_file() {
            return Err("ZIP archives may contain regular files and directories only.".into());
        }
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| "ZIP paths must not escape the package root.".to_string())?;
        let path = enclosed
            .to_str()
            .ok_or_else(|| "ZIP paths must use UTF-8 names.".to_string())?
            .to_string();
        if !safe_relative_path(&path) {
            return Err(format!("Unsafe ZIP path: {path}"));
        }
        let size = entry.size();
        let compressed = entry.compressed_size();
        if size > MAX_EDITABLE_FILE_BYTES as u64 {
            return Err(format!(
                "ZIP entry exceeds the 1 MiB editable-file limit: {path}"
            ));
        }
        if size > 0 && (compressed == 0 || size > compressed.saturating_mul(MAX_COMPRESSION_RATIO))
        {
            return Err(format!(
                "ZIP entry exceeds the safe compression ratio: {path}"
            ));
        }
        total = total.saturating_add(size);
        if total > MAX_DRAFT_BYTES as u64 {
            return Err("The extracted ZIP archive exceeds 50 MiB.".into());
        }
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|_| format!("ZIP entry must contain editable UTF-8 text: {path}"))?;
        files.push(DraftFile { path, content });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let diagnostics = validate_file_set(&files);
    if let Some(item) = diagnostics
        .iter()
        .find(|item| item.code == "unsafe_path" || item.code == "duplicate_path")
    {
        return Err(item.message.clone());
    }
    Ok(files)
}

pub fn starter_files(kind: ResourceKind, slug: &str, name: &str) -> Vec<DraftFile> {
    match kind {
        ResourceKind::Agent => vec![DraftFile {
            path: format!("{slug}.md"),
            content: format!(
                "---\nname: {slug}\ndescription: {name}\nrole: worker\n---\n\nYou are {name}.\n"
            ),
        }],
        ResourceKind::Skill => vec![DraftFile {
            path: "SKILL.md".into(),
            content: format!(
                "---\nname: {slug}\ndescription: {name}\n---\n\n# {name}\n\nDescribe when and how EvoFlux should use this skill.\n"
            ),
        }],
        ResourceKind::Plugin => vec![
            DraftFile {
                path: "plugin.json".into(),
                content: serde_json::to_string_pretty(&serde_json::json!({
                    "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
                    "name": slug,
                    "version": "0.1.0",
                    "description": name,
                    "extensions": {}
                }))
                .unwrap_or_default()
                    + "\n",
            },
            DraftFile {
                path: format!("skills/{slug}/SKILL.md"),
                content: format!(
                    "---\nname: {slug}\ndescription: {name}\n---\n\n# {name}\n\nPlugin-provided skill instructions.\n"
                ),
            },
        ],
        ResourceKind::Workflow | ResourceKind::Command => vec![DraftFile {
            path: format!("{slug}.json"),
            content: "{}\n".into(),
        }],
    }
}

pub fn validate_draft(
    kind: ResourceKind,
    slug: &str,
    revision: u64,
    files: &[DraftFile],
) -> ResourceValidation {
    let mut diagnostics = validate_file_set(files);
    match kind {
        ResourceKind::Agent => validate_agent(slug, files, &mut diagnostics),
        ResourceKind::Skill => validate_skill(slug, files, &mut diagnostics),
        ResourceKind::Plugin => validate_plugin(slug, files, &mut diagnostics),
        ResourceKind::Workflow | ResourceKind::Command => {}
    }
    ResourceValidation {
        valid: diagnostics
            .iter()
            .all(|item| item.severity != DiagnosticSeverity::Error),
        revision,
        diagnostics,
    }
}

pub fn versioned_plugin_files(
    files: &[DraftFile],
    version: &str,
) -> Result<Vec<DraftFile>, ResourceDiagnostic> {
    let mut updated = files.to_vec();
    let manifest = updated
        .iter_mut()
        .find(|file| file.path == "plugin.json")
        .ok_or_else(|| {
            diagnostic(
                "manifest_missing",
                "plugin.json is required.",
                "plugin.json",
            )
        })?;
    let mut value: serde_json::Value = serde_json::from_str(&manifest.content).map_err(|_| {
        diagnostic(
            "manifest_json_invalid",
            "plugin.json must contain valid JSON before release.",
            "plugin.json",
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        diagnostic(
            "manifest_not_object",
            "plugin.json must be a JSON object.",
            "plugin.json",
        )
    })?;
    object.insert("version".into(), serde_json::Value::String(version.into()));
    manifest.content = serde_json::to_string_pretty(&value).unwrap_or_default() + "\n";
    Ok(updated)
}

pub fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn validate_file_set(files: &[DraftFile]) -> Vec<ResourceDiagnostic> {
    let mut diagnostics = Vec::new();
    if files.is_empty() {
        diagnostics.push(diagnostic(
            "draft_empty",
            "Add at least one source file before release.",
            "",
        ));
    }
    if files.len() > MAX_DRAFT_FILES {
        diagnostics.push(diagnostic(
            "file_count_exceeded",
            "The draft contains more than 2,000 entries.",
            "",
        ));
    }
    let mut seen = HashSet::new();
    let mut folded = HashSet::new();
    let mut total = 0_usize;
    for file in files {
        if !safe_relative_path(&file.path) {
            diagnostics.push(diagnostic(
                "unsafe_path",
                "Paths must be relative and cannot contain traversal, backslashes or drive prefixes.",
                &file.path,
            ));
        }
        if !seen.insert(file.path.clone()) || !folded.insert(file.path.to_lowercase()) {
            diagnostics.push(diagnostic(
                "duplicate_path",
                "Duplicate and case-fold-colliding paths are not allowed.",
                &file.path,
            ));
        }
        let bytes = file.content.len();
        total = total.saturating_add(bytes);
        if bytes > MAX_EDITABLE_FILE_BYTES {
            diagnostics.push(diagnostic(
                "file_size_exceeded",
                "Editable text files are limited to 1 MiB.",
                &file.path,
            ));
        }
    }
    if total > MAX_DRAFT_BYTES {
        diagnostics.push(diagnostic(
            "draft_size_exceeded",
            "The extracted draft exceeds the 50 MiB limit.",
            "",
        ));
    }
    diagnostics
}

fn validate_agent(slug: &str, files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    let markdown = files.iter().find(|file| file.path.ends_with(".md"));
    let Some(markdown) = markdown else {
        diagnostics.push(diagnostic(
            "agent_markdown_missing",
            "An Agent draft requires one Markdown definition.",
            "",
        ));
        return;
    };
    validate_frontmatter(slug, markdown, diagnostics);
}

fn validate_skill(slug: &str, files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    let Some(skill) = files.iter().find(|file| file.path == "SKILL.md") else {
        diagnostics.push(diagnostic(
            "skill_manifest_missing",
            "A standalone Skill requires SKILL.md at the draft root.",
            "SKILL.md",
        ));
        return;
    };
    validate_frontmatter(slug, skill, diagnostics);
}

fn validate_plugin(slug: &str, files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    let Some(manifest) = files.iter().find(|file| file.path == "plugin.json") else {
        diagnostics.push(diagnostic(
            "manifest_missing",
            "A Portable Agent Plugin requires plugin.json at the package root.",
            "plugin.json",
        ));
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&manifest.content) else {
        diagnostics.push(diagnostic(
            "manifest_json_invalid",
            "plugin.json is not valid JSON.",
            "plugin.json",
        ));
        return;
    };
    let Some(object) = value.as_object() else {
        diagnostics.push(diagnostic(
            "manifest_not_object",
            "plugin.json must be a JSON object.",
            "plugin.json",
        ));
        return;
    };
    if object.get("$schema").and_then(serde_json::Value::as_str)
        != Some("https://agent-plugins.org/schemas/1.0.0/plugin.schema.json")
    {
        diagnostics.push(diagnostic(
            "manifest_schema_invalid",
            "Use the Portable Agent Plugins 1.0 schema identifier.",
            "plugin.json",
        ));
    }
    if object.get("name").and_then(serde_json::Value::as_str) != Some(slug) {
        diagnostics.push(diagnostic(
            "manifest_name_mismatch",
            "plugin.json name must match the Conductor resource slug.",
            "plugin.json",
        ));
    }
    if object
        .get("extensions")
        .is_some_and(|value| !value.is_object())
    {
        diagnostics.push(diagnostic(
            "manifest_extensions_invalid",
            "plugin.json extensions must be an object.",
            "plugin.json",
        ));
    }
    for skill in files
        .iter()
        .filter(|file| file.path.starts_with("skills/") && file.path.ends_with("/SKILL.md"))
    {
        let expected = skill.path.split('/').nth(1).unwrap_or_default();
        validate_frontmatter(expected, skill, diagnostics);
    }
}

fn validate_frontmatter(
    expected_name: &str,
    file: &DraftFile,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) {
    if !file.content.starts_with("---\n") {
        diagnostics.push(diagnostic(
            "frontmatter_missing",
            "Markdown must start with YAML frontmatter.",
            &file.path,
        ));
        return;
    }
    let Some(end) = file.content[4..].find("\n---") else {
        diagnostics.push(diagnostic(
            "frontmatter_unclosed",
            "YAML frontmatter must end with ---. ",
            &file.path,
        ));
        return;
    };
    let frontmatter = &file.content[4..4 + end];
    let name = frontmatter.lines().find_map(|line| {
        line.strip_prefix("name:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
    });
    if name != Some(expected_name) {
        diagnostics.push(diagnostic(
            "frontmatter_name_mismatch",
            "Frontmatter name must match the resource or component directory.",
            &file.path,
        ));
    }
    if !frontmatter.lines().any(|line| {
        line.strip_prefix("description:")
            .is_some_and(|value| !value.trim().is_empty())
    }) {
        diagnostics.push(diagnostic(
            "frontmatter_description_missing",
            "Frontmatter requires a non-empty description.",
            &file.path,
        ));
    }
}

fn diagnostic(code: &str, message: &str, path: &str) -> ResourceDiagnostic {
    ResourceDiagnostic {
        severity: DiagnosticSeverity::Error,
        code: code.into(),
        message: message.into(),
        path: (!path.is_empty()).then(|| path.into()),
        line: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (path, content) in entries {
            writer.start_file(path, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn rejects_traversal_and_case_collisions() {
        let files = vec![
            DraftFile {
                path: "../plugin.json".into(),
                content: "{}".into(),
            },
            DraftFile {
                path: "README.md".into(),
                content: "one".into(),
            },
            DraftFile {
                path: "readme.md".into(),
                content: "two".into(),
            },
        ];
        let validation = validate_draft(ResourceKind::Plugin, "demo", 1, &files);
        assert!(!validation.valid);
        assert!(validation
            .diagnostics
            .iter()
            .any(|item| item.code == "unsafe_path"));
        assert!(validation
            .diagnostics
            .iter()
            .any(|item| item.code == "duplicate_path"));
    }

    #[test]
    fn every_starter_passes_static_validation() {
        for kind in [
            ResourceKind::Agent,
            ResourceKind::Skill,
            ResourceKind::Plugin,
        ] {
            let files = starter_files(kind, "release-audit", "Release audit");
            let result = validate_draft(kind, "release-audit", 0, &files);
            assert!(result.valid, "{kind:?}: {:?}", result.diagnostics);
        }
    }

    #[test]
    fn imports_editable_zip_files() {
        let files = import_zip(archive(&[
            ("plugin.json", "{}"),
            ("skills/a/SKILL.md", "ok"),
        ]))
        .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "plugin.json");
    }

    #[test]
    fn rejects_traversal_in_zip() {
        let result = import_zip(archive(&[("../escape.md", "no")]));
        assert!(result.is_err());
    }
}
