//! Static, non-executing validation for governed Agent, Skill and Plugin drafts.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::str::FromStr;

use conductor_domain::{
    DiagnosticSeverity, DraftFile, FileManifestEntry, ResourceBundleKind, ResourceBundleV2,
    ResourceDiagnostic, ResourceKind, ResourceTargetMode, ResourceValidation, SemanticVersion,
};
use sha2::{Digest, Sha256};

use crate::core::constants::resource::RESOURCE_MODE_SCOPE_FILENAME;

pub const MAX_DRAFT_FILES: usize = 2_000;
pub const MAX_EDITABLE_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_DRAFT_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_IMPORT_ARCHIVE_BYTES: usize = 20 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 2_500;
const MAX_COMPRESSION_RATIO: u64 = 100;
const MAX_SKILL_MARKDOWN_BYTES: usize = 512 * 1024;
const MAX_SKILL_DESCRIPTION_CHARS: usize = 1_024;

#[derive(Debug, Clone, Default)]
pub struct ArchiveSourceMetadata {
    pub slug: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub primary_source: Option<String>,
}

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
    let mut files = normalize_archive_root(files);
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
        ResourceKind::Agent => vec![
            DraftFile {
                path: format!("{slug}.md"),
                content: format!(
                    "---\nname: {slug}\nrole: member\ndescription: {name}\n---\n\nYou are \"{name}\" — a focused EvoFlux team member.\n\n## Responsibilities\n\n- Define the work this Agent owns.\n- State its boundaries and hand-off conditions.\n"
                ),
            },
            target_mode_file(&ResourceTargetMode::ALL),
        ],
        ResourceKind::Skill => {
            let yaml_display_name =
                serde_json::to_string(name).unwrap_or_else(|_| format!("\"{slug}\""));
            vec![DraftFile {
                path: "SKILL.md".into(),
                content: format!(
                    "---\nname: {slug}\ndescription: Describe what this skill does, when it should activate, and the nearby requests it must not handle.\n---\n\n# {name}\n\n## Use this skill when\n\n- Define the positive activation conditions.\n- Define important near-misses that should not activate it.\n\n## Workflow\n\n1. Define the required inputs and intended output.\n2. Perform the smallest reliable workflow for this specialty.\n3. Load bundled references only when a step needs them.\n4. Verify the result with observable checks.\n\n## Output contract\n\n- Specify the artifact, answer, or code change this skill produces.\n- Report evidence, uncertainty, and remaining risks.\n"
                ),
            }, DraftFile {
                path: "agents/evoflux.yaml".into(),
                content: format!(
                    "interface:\n  display_name: {yaml_display_name}\n  short_description: A focused reusable workflow for EvoFlux\n  default_prompt: Use ${slug} for this task.\npolicy:\n  allow_implicit_invocation: true\n"
                ),
            },
            DraftFile {
                path: "evals/trigger-cases.json".into(),
                content: serde_json::to_string_pretty(&serde_json::json!({
                    "skill": slug,
                    "cases": [
                        {
                            "prompt": "A realistic request that should activate this workflow.",
                            "should_trigger": true,
                            "reason": "Replace with the distinguishing activation signal."
                        },
                        {
                            "prompt": "A nearby request the base agent can handle without this workflow.",
                            "should_trigger": false,
                            "reason": "Replace with the boundary that prevents over-triggering."
                        }
                    ]
                }))
                .unwrap_or_default()
                    + "\n",
            },
            target_mode_file(&ResourceTargetMode::ALL)]
        },
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

pub fn set_target_modes(files: &mut Vec<DraftFile>, modes: &[ResourceTargetMode]) {
    let mode_file = target_mode_file(if modes.is_empty() {
        &ResourceTargetMode::ALL
    } else {
        modes
    });
    if let Some(existing) = files
        .iter_mut()
        .find(|file| file.path == RESOURCE_MODE_SCOPE_FILENAME)
    {
        *existing = mode_file;
    } else {
        files.push(mode_file);
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
}

fn target_mode_file(modes: &[ResourceTargetMode]) -> DraftFile {
    DraftFile {
        path: RESOURCE_MODE_SCOPE_FILENAME.into(),
        content: serde_json::to_string_pretty(&serde_json::json!({
            "modes": modes.iter().map(|mode| mode.as_str()).collect::<Vec<_>>(),
        }))
        .unwrap_or_default()
            + "\n",
    }
}

/// Builds the canonical delivery descriptor for bundle-backed resource kinds.
///
/// File entries are sorted by path so the tree digest is independent from ZIP
/// entry order or editor ordering. The executable bit is intentionally false:
/// today's UTF-8 authoring/import pipeline does not retain source file modes.
pub fn resource_bundle_v2(
    kind: ResourceKind,
    slug: &str,
    version: &str,
    artifact_sha256: &str,
    artifact_size: u64,
    artifact_media_type: &str,
    files: &[DraftFile],
) -> Option<ResourceBundleV2> {
    let kind = ResourceBundleKind::from_resource_kind(kind)?;
    let mut manifest = files
        .iter()
        .map(|file| FileManifestEntry {
            path: file.path.clone(),
            sha256: hex::encode(Sha256::digest(file.content.as_bytes())),
            size: file.content.len().try_into().unwrap_or(u64::MAX),
            media_type: resource_file_media_type(&file.path).to_string(),
            executable: false,
        })
        .collect::<Vec<_>>();
    manifest.sort_by(|left, right| left.path.cmp(&right.path));

    Some(ResourceBundleV2 {
        schema_version: ResourceBundleV2::SCHEMA_VERSION,
        kind,
        slug: slug.to_string(),
        version: version.to_string(),
        artifact_sha256: artifact_sha256.to_string(),
        artifact_size,
        artifact_media_type: artifact_media_type.to_string(),
        tree_sha256: manifest_tree_sha256(&manifest),
        files: manifest,
    })
}

fn manifest_tree_sha256(files: &[FileManifestEntry]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"evoflux-resource-tree-v2\n");
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(file.sha256.as_bytes());
        hasher.update(b"\0");
        hasher.update(file.size.to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(file.media_type.as_bytes());
        hasher.update(b"\0");
        hasher.update(if file.executable { b"1" } else { b"0" });
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn resource_file_media_type(path: &str) -> &'static str {
    let path = path.to_ascii_lowercase();
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("md") => "text/markdown",
        Some("json") => "application/json",
        Some("yaml" | "yml") => "application/yaml",
        Some("toml") => "application/toml",
        Some("xml") => "application/xml",
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("csv") => "text/csv",
        Some("txt") => "text/plain",
        Some("py") => "text/x-python",
        Some("js" | "mjs" | "cjs") => "text/javascript",
        Some("ts" | "tsx") => "text/typescript",
        Some("sh" | "bash" | "zsh") => "text/x-shellscript",
        Some("pdf") => "application/pdf",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

pub fn archive_source_metadata(kind: ResourceKind, files: &[DraftFile]) -> ArchiveSourceMetadata {
    match kind {
        ResourceKind::Agent => files
            .iter()
            .find(|file| !file.path.contains('/') && file.path.ends_with(".md"))
            .map(|file| markdown_source_metadata(file, None))
            .unwrap_or_default(),
        ResourceKind::Skill => files
            .iter()
            .find(|file| file.path == "SKILL.md")
            .map(|file| markdown_source_metadata(file, None))
            .unwrap_or_default(),
        ResourceKind::Plugin => plugin_source_metadata(files),
        ResourceKind::Workflow | ResourceKind::Command => ArchiveSourceMetadata::default(),
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
    let root_markdown = files
        .iter()
        .filter(|file| !file.path.contains('/') && file.path.ends_with(".md"))
        .collect::<Vec<_>>();
    let supported_file_count = files
        .iter()
        .filter(|file| {
            (!file.path.contains('/') && file.path.ends_with(".md"))
                || file.path == RESOURCE_MODE_SCOPE_FILENAME
        })
        .count();
    if root_markdown.len() != 1 || supported_file_count != files.len() {
        diagnostics.push(diagnostic(
            "agent_source_count_invalid",
            "An EvoFlux Agent archive must contain one root Markdown definition and may include only .evoflux.json deployment metadata.",
            "",
        ));
    }
    validate_target_modes(files, diagnostics);
    let Some(markdown) = root_markdown.first().copied() else {
        return;
    };
    let expected_path = format!("{slug}.md");
    if markdown.path != expected_path {
        diagnostics.push(diagnostic(
            "agent_filename_mismatch",
            "The root Agent filename must match the resource slug.",
            &markdown.path,
        ));
    }
    let Some(document) = parse_markdown_document(markdown, diagnostics) else {
        return;
    };
    validate_frontmatter_name(slug, markdown, &document.fields, diagnostics);
    let role = document.fields.get("role").map(String::as_str);
    if !matches!(role, Some("lead" | "member")) {
        diagnostics.push(diagnostic(
            "agent_role_invalid",
            "Agent frontmatter role must be either 'lead' or 'member'.",
            &markdown.path,
        ));
    }
    if document
        .fields
        .get("description")
        .is_some_and(|value| value.len() > MAX_SKILL_DESCRIPTION_CHARS)
    {
        diagnostics.push(diagnostic(
            "agent_description_too_long",
            "Agent frontmatter description must be at most 1024 characters.",
            &markdown.path,
        ));
    }
    if document.fields.get("model").is_some_and(|model| {
        !model.is_empty() && model != "__PROVIDER_MODEL__" && !valid_model_id(model)
    }) {
        diagnostics.push(diagnostic(
            "agent_model_invalid",
            "Agent model must use provider:model syntax when it is set.",
            &markdown.path,
        ));
    }
}

fn validate_skill(slug: &str, files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    validate_target_modes(files, diagnostics);
    let Some(skill) = files.iter().find(|file| file.path == "SKILL.md") else {
        diagnostics.push(diagnostic(
            "skill_manifest_missing",
            "A standalone Skill requires SKILL.md at the draft root.",
            "SKILL.md",
        ));
        return;
    };
    if skill.content.len() > MAX_SKILL_MARKDOWN_BYTES {
        diagnostics.push(diagnostic(
            "skill_markdown_too_large",
            "SKILL.md must not exceed the EvoFlux 512 KiB runtime limit.",
            &skill.path,
        ));
    }
    let Some(document) = parse_markdown_document(skill, diagnostics) else {
        return;
    };
    validate_frontmatter_name(slug, skill, &document.fields, diagnostics);
    if slug.len() > 64 || !valid_portable_skill_name(slug) {
        diagnostics.push(diagnostic(
            "skill_name_invalid",
            "Skill names must use 1–64 lowercase letters or digits joined by single hyphens.",
            &skill.path,
        ));
    }
    let description = document.fields.get("description");
    if description.is_none_or(String::is_empty) {
        diagnostics.push(diagnostic(
            "frontmatter_description_missing",
            "SKILL.md frontmatter requires a non-empty description.",
            &skill.path,
        ));
    } else if description.is_some_and(|value| value.len() > MAX_SKILL_DESCRIPTION_CHARS) {
        diagnostics.push(diagnostic(
            "skill_description_too_long",
            "Skill frontmatter description must be at most 1024 characters.",
            &skill.path,
        ));
    }
    for key in document.fields.keys() {
        if key != "name" && key != "description" {
            diagnostics.push(diagnostic(
                "skill_frontmatter_field_unsupported",
                "Portable SKILL.md frontmatter may contain only 'name' and 'description'.",
                &skill.path,
            ));
            break;
        }
    }
    if document.body.trim().is_empty() {
        diagnostics.push(diagnostic(
            "skill_instructions_missing",
            "SKILL.md must contain non-empty workflow instructions after the frontmatter.",
            &skill.path,
        ));
    }
}

fn validate_target_modes(files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    let Some(scope) = files
        .iter()
        .find(|file| file.path == RESOURCE_MODE_SCOPE_FILENAME)
    else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&scope.content) else {
        diagnostics.push(diagnostic(
            "resource_modes_json_invalid",
            ".evoflux.json must contain valid JSON.",
            RESOURCE_MODE_SCOPE_FILENAME,
        ));
        return;
    };
    let Some(modes) = value.get("modes").and_then(serde_json::Value::as_array) else {
        diagnostics.push(diagnostic(
            "resource_modes_missing",
            ".evoflux.json requires a non-empty modes array.",
            RESOURCE_MODE_SCOPE_FILENAME,
        ));
        return;
    };
    let mut selected = HashSet::new();
    if modes.is_empty()
        || modes.iter().any(|mode| {
            mode.as_str()
                .is_none_or(|mode| !matches!(mode, "work" | "coding" | "aim"))
        })
        || modes
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|mode| !selected.insert(mode))
    {
        diagnostics.push(diagnostic(
            "resource_modes_invalid",
            "modes must contain work, coding and/or aim exactly once.",
            RESOURCE_MODE_SCOPE_FILENAME,
        ));
    }
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
    let manifest_name = object.get("name").and_then(serde_json::Value::as_str);
    if manifest_name.is_none_or(str::is_empty) {
        diagnostics.push(diagnostic(
            "manifest_name_missing",
            "plugin.json requires a non-empty name.",
            "plugin.json",
        ));
    } else if manifest_name.is_some_and(|name| {
        name.len() > 80
            || !name.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
    }) {
        diagnostics.push(diagnostic(
            "manifest_name_invalid",
            "plugin.json name must use 1–80 lowercase letters, numbers, or hyphens.",
            "plugin.json",
        ));
    } else if manifest_name != Some(slug) {
        diagnostics.push(diagnostic(
            "manifest_name_mismatch",
            "plugin.json name must match the Conductor resource slug.",
            "plugin.json",
        ));
    }
    let manifest_version = object.get("version").and_then(serde_json::Value::as_str);
    if manifest_version.is_none_or(|version| SemanticVersion::from_str(version).is_err()) {
        diagnostics.push(diagnostic(
            "manifest_version_invalid",
            "plugin.json version must follow strict SemVer 2.0.",
            "plugin.json",
        ));
    }
    let description = object
        .get("description")
        .and_then(serde_json::Value::as_str);
    if description.is_none_or(str::is_empty) {
        diagnostics.push(diagnostic(
            "manifest_description_missing",
            "plugin.json requires a non-empty description.",
            "plugin.json",
        ));
    } else if description.is_some_and(|value| value.len() > 1_000) {
        diagnostics.push(diagnostic(
            "manifest_description_too_long",
            "plugin.json description must be at most 1000 characters.",
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
        validate_named_markdown(expected, skill, diagnostics);
    }
}

fn validate_named_markdown(
    expected_name: &str,
    file: &DraftFile,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) {
    let Some(document) = parse_markdown_document(file, diagnostics) else {
        return;
    };
    validate_frontmatter_name(expected_name, file, &document.fields, diagnostics);
    if document
        .fields
        .get("description")
        .is_none_or(String::is_empty)
    {
        diagnostics.push(diagnostic(
            "frontmatter_description_missing",
            "Frontmatter requires a non-empty description.",
            &file.path,
        ));
    }
}

fn validate_frontmatter_name(
    expected_name: &str,
    file: &DraftFile,
    fields: &HashMap<String, String>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) {
    if fields.get("name").map(String::as_str) != Some(expected_name) {
        diagnostics.push(diagnostic(
            "frontmatter_name_mismatch",
            "Frontmatter name must match the resource or component directory.",
            &file.path,
        ));
    }
}

struct MarkdownDocument {
    fields: HashMap<String, String>,
    body: String,
}

fn parse_markdown_document(
    file: &DraftFile,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) -> Option<MarkdownDocument> {
    let normalized = file.content.replace("\r\n", "\n");
    let Some(rest) = normalized.trim_start().strip_prefix("---\n") else {
        diagnostics.push(diagnostic(
            "frontmatter_missing",
            "Markdown must start with YAML frontmatter.",
            &file.path,
        ));
        return None;
    };
    let document = rest
        .split_once("\n---\n")
        .map(|(frontmatter, body)| (frontmatter, body.to_string()))
        .or_else(|| {
            rest.strip_suffix("\n---")
                .map(|frontmatter| (frontmatter, String::new()))
        });
    let Some((frontmatter, body)) = document else {
        diagnostics.push(diagnostic(
            "frontmatter_unclosed",
            "YAML frontmatter must end with a standalone --- delimiter.",
            &file.path,
        ));
        return None;
    };
    let mut fields = HashMap::new();
    let mut current_key = None;
    for line in frontmatter.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            if current_key.is_none() {
                diagnostics.push(diagnostic(
                    "frontmatter_yaml_invalid",
                    "Frontmatter must be a YAML mapping with top-level fields.",
                    &file.path,
                ));
                return None;
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            diagnostics.push(diagnostic(
                "frontmatter_yaml_invalid",
                "Frontmatter contains an invalid YAML field.",
                &file.path,
            ));
            return None;
        };
        let key = key.trim();
        if key.is_empty()
            || !key.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
            || fields.contains_key(key)
        {
            diagnostics.push(diagnostic(
                "frontmatter_yaml_invalid",
                "Frontmatter fields must be unique YAML mapping keys.",
                &file.path,
            ));
            return None;
        }
        fields.insert(key.to_string(), unquote_yaml_scalar(value.trim()));
        current_key = Some(key.to_string());
    }
    Some(MarkdownDocument { fields, body })
}

fn unquote_yaml_scalar(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if matches!(
            (bytes[0], bytes[value.len() - 1]),
            (b'"', b'"') | (b'\'', b'\'')
        ) {
            return value[1..value.len() - 1].to_string();
        }
    }
    value
        .split_once(" #")
        .map(|(scalar, _)| scalar)
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn valid_model_id(value: &str) -> bool {
    value.split_once(':').is_some_and(|(provider, model)| {
        !provider.is_empty() && !model.is_empty() && !value.contains(char::is_whitespace)
    })
}

fn valid_portable_skill_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        })
}

fn markdown_source_metadata(file: &DraftFile, version: Option<String>) -> ArchiveSourceMetadata {
    let mut ignored = Vec::new();
    let document = parse_markdown_document(file, &mut ignored);
    ArchiveSourceMetadata {
        slug: document
            .as_ref()
            .and_then(|item| item.fields.get("name").cloned()),
        version,
        description: document
            .as_ref()
            .and_then(|item| item.fields.get("description").cloned()),
        primary_source: Some(file.path.clone()),
    }
}

fn plugin_source_metadata(files: &[DraftFile]) -> ArchiveSourceMetadata {
    let manifest = files
        .iter()
        .find(|file| file.path == "plugin.json")
        .and_then(|file| serde_json::from_str::<serde_json::Value>(&file.content).ok());
    ArchiveSourceMetadata {
        slug: manifest
            .as_ref()
            .and_then(|value| value.get("name"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        version: manifest
            .as_ref()
            .and_then(|value| value.get("version"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        description: manifest
            .as_ref()
            .and_then(|value| value.get("description"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        primary_source: manifest.map(|_| "plugin.json".to_string()),
    }
}

fn normalize_archive_root(files: Vec<DraftFile>) -> Vec<DraftFile> {
    let wrapper = files
        .iter()
        .filter_map(|file| file.path.split_once('/').map(|(root, _)| root))
        .next()
        .map(str::to_string);
    let Some(wrapper) = wrapper else {
        return files;
    };
    if files
        .iter()
        .any(|file| !file.path.starts_with(&format!("{wrapper}/")))
    {
        return files;
    }
    files
        .into_iter()
        .map(|mut file| {
            file.path = file.path[wrapper.len() + 1..].to_string();
            file
        })
        .collect()
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
    fn agent_validation_matches_evoflux_role_and_single_file_contract() {
        let files = vec![
            DraftFile {
                path: "reviewer.md".into(),
                content: "---\nname: reviewer\nrole: worker\ndescription: Review changes.\n---\n\nReview.\n"
                    .into(),
            },
            DraftFile {
                path: "notes.md".into(),
                content: "Not part of an EvoFlux Agent definition.\n".into(),
            },
        ];
        let result = validate_draft(ResourceKind::Agent, "reviewer", 0, &files);
        assert!(!result.valid);
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "agent_source_count_invalid"));
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "agent_role_invalid"));
    }

    #[test]
    fn skill_validation_rejects_non_portable_frontmatter() {
        let files = vec![DraftFile {
            path: "SKILL.md".into(),
            content: "---\nname: release-audit\ndescription: Audit a release.\nmode: coding\n---\n\nAudit it.\n"
                .into(),
        }];
        let result = validate_draft(ResourceKind::Skill, "release-audit", 0, &files);
        assert!(!result.valid);
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "skill_frontmatter_field_unsupported"));
    }

    #[test]
    fn target_modes_use_evoflux_work_coding_and_aim_contract() {
        let mut files = starter_files(ResourceKind::Agent, "reviewer", "Reviewer");
        set_target_modes(
            &mut files,
            &[ResourceTargetMode::Coding, ResourceTargetMode::Aim],
        );
        let result = validate_draft(ResourceKind::Agent, "reviewer", 0, &files);
        assert!(result.valid, "{:?}", result.diagnostics);
        let scope = files
            .iter()
            .find(|file| file.path == RESOURCE_MODE_SCOPE_FILENAME)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&scope.content).unwrap(),
            serde_json::json!({ "modes": ["coding", "aim"] })
        );
    }

    #[test]
    fn target_modes_reject_unknown_or_empty_values() {
        for content in [r#"{"modes":[]}"#, r#"{"modes":["cowork"]}"#] {
            let mut files = starter_files(ResourceKind::Skill, "release-audit", "Audit");
            files
                .iter_mut()
                .find(|file| file.path == RESOURCE_MODE_SCOPE_FILENAME)
                .unwrap()
                .content = content.into();
            let result = validate_draft(ResourceKind::Skill, "release-audit", 0, &files);
            assert!(!result.valid);
            assert!(result
                .diagnostics
                .iter()
                .any(|item| item.code == "resource_modes_invalid"));
        }
    }

    #[test]
    fn bundle_v2_manifest_is_sorted_content_addressed_and_media_typed() {
        let files = vec![
            DraftFile {
                path: "scripts/check.py".into(),
                content: "print('ok')\n".into(),
            },
            DraftFile {
                path: "SKILL.md".into(),
                content: "# Audit\n".into(),
            },
        ];
        let bundle = resource_bundle_v2(
            ResourceKind::Skill,
            "audit",
            "1.2.3",
            &"a".repeat(64),
            123,
            "application/vnd.evoflux.resource+json",
            &files,
        )
        .unwrap();

        assert_eq!(bundle.schema_version, 2);
        assert_eq!(bundle.kind, ResourceBundleKind::Skill);
        assert_eq!(bundle.files[0].path, "SKILL.md");
        assert_eq!(bundle.files[0].media_type, "text/markdown");
        assert_eq!(bundle.files[1].path, "scripts/check.py");
        assert_eq!(bundle.files[1].media_type, "text/x-python");
        assert!(bundle.files.iter().all(|file| !file.executable));
        assert_eq!(
            bundle.tree_sha256,
            "693430d4bd25d3bee52d51f622fb8c7732904d5a820785dc156927cafdfb3703"
        );
    }

    #[test]
    fn bundle_v2_excludes_non_portable_resource_kinds() {
        assert!(resource_bundle_v2(
            ResourceKind::Workflow,
            "deploy",
            "1.0.0",
            &"a".repeat(64),
            1,
            "application/json",
            &[],
        )
        .is_none());
    }

    #[test]
    fn rejects_traversal_in_zip() {
        let result = import_zip(archive(&[("../escape.md", "no")]));
        assert!(result.is_err());
    }
}
