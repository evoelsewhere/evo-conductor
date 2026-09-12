//! Static, non-executing validation for governed Agent, Skill and Plugin drafts.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::str::FromStr;

use conductor_domain::{
    DiagnosticSeverity, DraftFile, FileManifestEntry, ResourceBundle, ResourceBundleKind,
    ResourceDiagnostic, ResourceKind, ResourceTargetMode, ResourceValidation, SemanticVersion,
    TeamManifest, TEAM_AGENT_DIR,
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
        let path = zip_entry_path(&enclosed)?;
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
        ResourceKind::AgentTeam => {
            let lead = slug;
            let member = format!("{slug}-specialist");
            vec![
                DraftFile {
                    path: TeamManifest::FILENAME.into(),
                    content: serde_json::to_string_pretty(&serde_json::json!({
                        "lead": lead,
                        "members": [&member],
                    }))
                    .unwrap_or_default()
                        + "\n",
                },
                DraftFile {
                    path: format!("{TEAM_AGENT_DIR}{lead}.md"),
                    content: format!(
                        "---\nname: {lead}\nrole: lead\ndescription: {name} lead\n---\n\nYou lead \"{name}\".\n\n## Responsibilities\n\n- Decide what the team delivers and what stays out of scope.\n- Delegate to the member best suited to each subtask.\n- Verify member evidence before reporting the result.\n"
                    ),
                },
                DraftFile {
                    path: format!("{TEAM_AGENT_DIR}{member}.md"),
                    content: format!(
                        "---\nname: {member}\nrole: member\nlead: {lead}\ndescription: Focused specialist for {name}\n---\n\nYou are a focused specialist on \"{name}\".\n\n## Responsibilities\n\n- Define the work this member owns.\n- State its boundaries and hand-off conditions.\n"
                    ),
                },
                target_mode_file(&ResourceTargetMode::ALL),
            ]
        }
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
pub fn resource_bundle(
    kind: ResourceKind,
    slug: &str,
    version: &str,
    artifact_sha256: &str,
    artifact_size: u64,
    artifact_media_type: &str,
    files: &[DraftFile],
) -> Option<ResourceBundle> {
    let kind = ResourceBundleKind::from_resource_kind(kind)?;
    let manifest = file_manifest(files);

    Some(ResourceBundle {
        schema_version: ResourceBundle::SCHEMA_VERSION,
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

pub fn file_manifest(files: &[DraftFile]) -> Vec<FileManifestEntry> {
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
    manifest
}

pub fn resource_archive_media_type(kind: ResourceKind) -> &'static str {
    if kind == ResourceKind::Plugin {
        "application/vnd.evoflux.plugin+zip"
    } else {
        "application/vnd.evoflux.resource+zip"
    }
}

/// Metadata persisted in SQL for a file bundle. It contains no authored file
/// bytes; the complete source lives at `artifact_key` in object storage.
pub struct ResourceStorageArtifact<'a> {
    pub key: &'a str,
    pub sha256: &'a str,
    pub size: u64,
    pub media_type: &'a str,
}

pub fn resource_storage_payload(
    kind: ResourceKind,
    slug: &str,
    version: &str,
    artifact: ResourceStorageArtifact<'_>,
    files: &[DraftFile],
) -> serde_json::Value {
    let bundle = resource_bundle(
        kind,
        slug,
        version,
        artifact.sha256,
        artifact.size,
        artifact.media_type,
        files,
    );
    let mut payload = serde_json::json!({
        "storage_schema_version": 1,
        "artifact": {
            "key": artifact.key,
            "sha256": artifact.sha256,
            "size": artifact.size,
            "media_type": artifact.media_type,
        },
        "files": file_manifest(files),
        "bundle": bundle,
    });
    // An Agent references an MCP server by the server's own name, never by the
    // plugin's slug, so the catalog has to carry the names a package declares
    // or nothing can offer a reference that will actually resolve.
    if kind == ResourceKind::Plugin {
        let servers = plugin_mcp_server_names(files);
        if !servers.is_empty() {
            payload["mcp_servers"] = serde_json::json!(servers);
        }
    }
    payload
}

/// Names declared by a plugin package's `mcp.json` (`mcpServers` keys).
pub fn plugin_mcp_server_names(files: &[DraftFile]) -> Vec<String> {
    let Some(file) = files.iter().find(|file| file.path == "mcp.json") else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&file.content) else {
        return Vec::new();
    };
    let Some(servers) = value.get("mcpServers").and_then(|item| item.as_object()) else {
        return Vec::new();
    };
    let mut names = servers.keys().cloned().collect::<Vec<_>>();
    names.sort();
    names
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
        ResourceKind::AgentTeam => team_source_metadata(files),
        ResourceKind::Skill => files
            .iter()
            .find(|file| file.path == "SKILL.md")
            .map(|file| markdown_source_metadata(file, None))
            .unwrap_or_default(),
        ResourceKind::Plugin => plugin_source_metadata(files),
        ResourceKind::Workflow | ResourceKind::Command => ArchiveSourceMetadata::default(),
    }
}

/// A Team's importable identity is its lead: the manifest names it and the
/// slug must match it, so the lead's own Markdown supplies the description.
fn team_source_metadata(files: &[DraftFile]) -> ArchiveSourceMetadata {
    let Some(lead) = files
        .iter()
        .find(|file| file.path == TeamManifest::FILENAME)
        .and_then(|file| serde_json::from_str::<TeamManifest>(&file.content).ok())
        .map(|manifest| manifest.lead)
    else {
        return ArchiveSourceMetadata::default();
    };
    let path = format!("{TEAM_AGENT_DIR}{lead}.md");
    let mut metadata = files
        .iter()
        .find(|file| file.path == path)
        .map(|file| markdown_source_metadata(file, None))
        .unwrap_or_default();
    metadata.slug = Some(lead);
    metadata.primary_source = Some(path);
    metadata
}

pub fn validate_draft(
    kind: ResourceKind,
    slug: &str,
    revision: u64,
    files: &[DraftFile],
) -> ResourceValidation {
    let mut diagnostics = validate_file_set(files);
    match kind {
        ResourceKind::AgentTeam => validate_agent_team(slug, files, &mut diagnostics),
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

/// Spell a ZIP entry's path the way the archive declared it.
///
/// `enclosed_name` returns a platform `PathBuf` — its traversal check is what
/// we want, but on Windows it hands the components back joined with
/// backslashes, which [`safe_relative_path`] rejects. Every nested entry would
/// then fail to import on a Windows host. ZIP paths are always `/`-separated,
/// so rejoin the components the archive actually declared. On Unix this is a
/// no-op: a name containing a literal backslash stays one component and is
/// still refused.
fn zip_entry_path(enclosed: &std::path::Path) -> Result<String, String> {
    let mut path = String::new();
    for component in enclosed.components() {
        let part = component
            .as_os_str()
            .to_str()
            .ok_or_else(|| "ZIP paths must use UTF-8 names.".to_string())?;
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(part);
    }
    Ok(path)
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

/// Validates a Team release: one lead Agent, its members, and a manifest that
/// agrees with both.
///
/// EvoFlux reconstructs a team from Agent frontmatter alone, and a member that
/// omits `lead:` silently joins whichever lead that installation defaults to
/// rather than the one shipped beside it. That failure is invisible on the
/// client, so every member must name its lead here, before the release exists.
fn validate_agent_team(slug: &str, files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    let agent_files = files
        .iter()
        .filter(|file| {
            file.path.starts_with(TEAM_AGENT_DIR)
                && file.path.ends_with(".md")
                && file.path[TEAM_AGENT_DIR.len()..].split('/').count() == 1
        })
        .collect::<Vec<_>>();
    let supported = agent_files.len()
        + files
            .iter()
            .filter(|file| {
                file.path == TeamManifest::FILENAME || file.path == RESOURCE_MODE_SCOPE_FILENAME
            })
            .count();
    if supported != files.len() {
        diagnostics.push(diagnostic(
            "team_source_layout_invalid",
            "An EvoFlux Team archive may contain only team.json, .evoflux.json deployment metadata and agents/<name>.md definitions.",
            "",
        ));
    }
    validate_target_modes(files, diagnostics);

    if agent_files.is_empty() {
        diagnostics.push(diagnostic(
            "team_agents_missing",
            "A Team must define at least one Agent under agents/.",
            TEAM_AGENT_DIR,
        ));
    }

    let mut lead_names: Vec<String> = Vec::new();
    let mut member_names: Vec<String> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for file in &agent_files {
        let stem = file.path[TEAM_AGENT_DIR.len()..].trim_end_matches(".md");
        if !valid_team_agent_name(stem) {
            diagnostics.push(diagnostic(
                "team_agent_name_invalid",
                "Agent filenames may use only letters, digits, dot, underscore and hyphen.",
                &file.path,
            ));
            continue;
        }
        if !seen_names.insert(stem.to_string()) {
            diagnostics.push(diagnostic(
                "team_agent_name_duplicate",
                "Two Agents in this Team share a name.",
                &file.path,
            ));
        }
        let Some(document) = parse_markdown_document(file, diagnostics) else {
            continue;
        };
        validate_frontmatter_name(stem, file, &document.fields, diagnostics);
        let role = document.fields.get("role").map(String::as_str);
        let declared_lead = document.fields.get("lead").map(String::as_str);
        match role {
            Some("lead") => {
                lead_names.push(stem.to_string());
                if declared_lead.is_some() {
                    diagnostics.push(diagnostic(
                        "team_lead_declares_lead",
                        "The lead Agent must not declare a 'lead' field.",
                        &file.path,
                    ));
                }
            }
            Some("member") => {
                member_names.push(stem.to_string());
                if declared_lead.is_none_or(str::is_empty) {
                    diagnostics.push(diagnostic(
                        "team_member_lead_missing",
                        "Each member Agent must declare 'lead: <lead name>'; without it EvoFlux attaches the member to its own default lead instead of this Team.",
                        &file.path,
                    ));
                }
            }
            _ => diagnostics.push(diagnostic(
                "agent_role_invalid",
                "Agent frontmatter role must be either 'lead' or 'member'.",
                &file.path,
            )),
        }
        if document
            .fields
            .get("description")
            .is_some_and(|value| value.len() > MAX_SKILL_DESCRIPTION_CHARS)
        {
            diagnostics.push(diagnostic(
                "agent_description_too_long",
                "Agent frontmatter description must be at most 1024 characters.",
                &file.path,
            ));
        }
        // Which model runs, and how hard it thinks, is the installation's call:
        // EvoFlux resolves both against the providers that member actually has,
        // and it rewrites a placeholder in place — which would break the managed
        // copy's integrity check. A Team that ships either cannot be honoured.
        for field in ["model", "fallback_model", "thinking_level"] {
            if document
                .fields
                .get(field)
                .is_some_and(|value| !value.is_empty())
            {
                diagnostics.push(diagnostic(
                    "team_model_is_client_owned",
                    &format!(
                        "Remove '{field}': each EvoFlux installation chooses the model and thinking level for a governed Agent."
                    ),
                    &file.path,
                ));
            }
        }
    }

    if lead_names.len() != 1 {
        diagnostics.push(diagnostic(
            "team_lead_count_invalid",
            "A Team must define exactly one Agent with role 'lead'.",
            TEAM_AGENT_DIR,
        ));
    }
    let lead_name = lead_names.first().cloned();

    // The slug is the catalog's unique key, and a team's runtime identity in
    // EvoFlux is its lead's name. Binding them keeps one published team per
    // lead, so two teams cannot claim the same lead file on an installation.
    if let Some(lead) = lead_name.as_deref() {
        if lead != slug {
            diagnostics.push(diagnostic(
                "team_lead_slug_mismatch",
                "The lead Agent's name must match the Team slug.",
                &format!("{TEAM_AGENT_DIR}{lead}.md"),
            ));
        }
    }

    for file in &agent_files {
        let stem = file.path[TEAM_AGENT_DIR.len()..].trim_end_matches(".md");
        let Some(document) = parse_markdown_document(file, &mut Vec::new()) else {
            continue;
        };
        if document.fields.get("role").map(String::as_str) != Some("member") {
            continue;
        }
        let Some(declared) = document.fields.get("lead").filter(|value| !value.is_empty()) else {
            continue;
        };
        if lead_name.as_deref().is_some_and(|lead| declared != lead) {
            diagnostics.push(diagnostic(
                "team_member_lead_mismatch",
                "A member Agent names a lead that this Team does not define.",
                &format!("{TEAM_AGENT_DIR}{stem}.md"),
            ));
        }
    }

    validate_team_manifest(files, lead_name.as_deref(), &member_names, diagnostics);
}

fn validate_team_manifest(
    files: &[DraftFile],
    lead_name: Option<&str>,
    member_names: &[String],
    diagnostics: &mut Vec<ResourceDiagnostic>,
) {
    let Some(file) = files
        .iter()
        .find(|file| file.path == TeamManifest::FILENAME)
    else {
        diagnostics.push(diagnostic(
            "team_manifest_missing",
            "team.json is required.",
            TeamManifest::FILENAME,
        ));
        return;
    };
    let manifest = match serde_json::from_str::<TeamManifest>(&file.content) {
        Ok(manifest) => manifest,
        Err(_) => {
            diagnostics.push(diagnostic(
                "team_manifest_invalid",
                "team.json must be a JSON object with a 'lead' string and a 'members' array of strings.",
                TeamManifest::FILENAME,
            ));
            return;
        }
    };
    if lead_name.is_some_and(|lead| manifest.lead != lead) {
        diagnostics.push(diagnostic(
            "team_manifest_lead_mismatch",
            "team.json names a different lead than the Agent definitions do.",
            TeamManifest::FILENAME,
        ));
    }
    let declared = manifest.members.iter().collect::<HashSet<_>>();
    let actual = member_names.iter().collect::<HashSet<_>>();
    if declared != actual {
        diagnostics.push(diagnostic(
            "team_manifest_members_mismatch",
            "team.json members must list exactly the member Agents defined under agents/.",
            TeamManifest::FILENAME,
        ));
    }
    if member_names.is_empty() {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "team_has_no_members".into(),
            message: "This Team publishes a lead with no members.".into(),
            path: Some(TeamManifest::FILENAME.into()),
            line: None,
        });
    }
}

fn valid_team_agent_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value
            .chars()
            .all(|item| item.is_ascii_alphanumeric() || matches!(item, '.' | '_' | '-'))
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
                .is_none_or(|mode| ResourceTargetMode::parse(mode).is_none())
        })
        || modes
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|mode| !selected.insert(mode))
    {
        diagnostics.push(diagnostic(
            "resource_modes_invalid",
            "modes must contain work and/or coding exactly once.",
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
    validate_plugin_mcp(files, diagnostics);
}

/// The MCP schema identifier Agent Plugins 1.0 packages must declare.
const MCP_SCHEMA_ID: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";

/// Validate `mcp.json` against the contract EvoFlux enforces on install.
///
/// Without this a plugin publishes cleanly and then fails on the installation:
/// a malformed `mcp.json` yields no server names at all, so `payload.mcp_servers`
/// advertises nothing and every Agent that references one of those servers finds
/// it missing — with no diagnostic anywhere between the two systems.
fn validate_plugin_mcp(files: &[DraftFile], diagnostics: &mut Vec<ResourceDiagnostic>) {
    const PATH: &str = "mcp.json";
    let Some(file) = files.iter().find(|file| file.path == PATH) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&file.content) else {
        diagnostics.push(diagnostic(
            "mcp_json_invalid",
            "mcp.json is not valid JSON.",
            PATH,
        ));
        return;
    };
    let Some(object) = value.as_object() else {
        diagnostics.push(diagnostic(
            "mcp_not_object",
            "mcp.json must be a JSON object.",
            PATH,
        ));
        return;
    };
    if object.len() != 2 || !object.contains_key("$schema") || !object.contains_key("mcpServers") {
        diagnostics.push(diagnostic(
            "mcp_top_level_invalid",
            "mcp.json must contain exactly $schema and mcpServers.",
            PATH,
        ));
        return;
    }
    if object.get("$schema").and_then(serde_json::Value::as_str) != Some(MCP_SCHEMA_ID) {
        diagnostics.push(diagnostic(
            "mcp_schema_invalid",
            "Use the Portable Agent Plugins 1.0 MCP schema identifier.",
            PATH,
        ));
        return;
    }
    let Some(servers) = object.get("mcpServers").and_then(serde_json::Value::as_object) else {
        diagnostics.push(diagnostic(
            "mcp_servers_invalid",
            "mcpServers must be an object.",
            PATH,
        ));
        return;
    };
    if servers.is_empty() {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "mcp_servers_empty".into(),
            message: "mcp.json declares no servers. Remove the file or add one.".into(),
            path: Some(PATH.into()),
            line: None,
        });
        return;
    }
    for (name, server) in servers {
        if !valid_mcp_server_name(name) {
            diagnostics.push(diagnostic(
                "mcp_server_name_invalid",
                "Server names must be 1–80 lowercase letters, numbers, dots, or hyphens.",
                PATH,
            ));
            continue;
        }
        let Some(entry) = server.as_object() else {
            diagnostics.push(diagnostic(
                "mcp_server_not_object",
                &format!("Server '{name}' must be a JSON object."),
                PATH,
            ));
            continue;
        };
        match entry.get("type").and_then(serde_json::Value::as_str) {
            Some("stdio") => {
                let command = entry.get("command").and_then(serde_json::Value::as_str);
                match command {
                    None | Some("") => diagnostics.push(diagnostic(
                        "mcp_server_command_missing",
                        &format!("stdio server '{name}' requires a non-empty command."),
                        PATH,
                    )),
                    // The installation runs this command inside the plugin
                    // sandbox and rejects anything that is not a bare
                    // executable or a path under the package. Catching it
                    // here keeps a publish from shipping a server the
                    // installer will refuse to start.
                    Some(value) if !valid_mcp_stdio_command(value) => {
                        diagnostics.push(diagnostic(
                            "mcp_server_command_invalid",
                            &format!(
                                "stdio server '{name}' command must be a bare                                  executable name or begin with './'."
                            ),
                            PATH,
                        ))
                    }
                    Some(_) => {}
                }
                if let Some(env) = entry.get("env").and_then(serde_json::Value::as_object) {
                    if env.contains_key("PLUGIN_ROOT") || env.contains_key("PLUGIN_DATA") {
                        diagnostics.push(diagnostic(
                            "mcp_server_env_reserved",
                            &format!(
                                "Server '{name}' env must not define PLUGIN_ROOT or PLUGIN_DATA."
                            ),
                            PATH,
                        ));
                    }
                }
                if let Some(cwd) = entry.get("cwd").and_then(serde_json::Value::as_str) {
                    if !valid_mcp_cwd(cwd) {
                        diagnostics.push(diagnostic(
                            "mcp_server_cwd_invalid",
                            &format!(
                                "Server '{name}' cwd must be plugin-relative,                                  PLUGIN_ROOT-rooted, or PLUGIN_DATA-rooted."
                            ),
                            PATH,
                        ));
                    }
                }
            }
            Some("streamable-http") | Some("sse") => {
                let url = entry.get("url").and_then(serde_json::Value::as_str);
                if url.is_none_or(str::is_empty) {
                    diagnostics.push(diagnostic(
                        "mcp_server_url_missing",
                        &format!("HTTP server '{name}' requires a non-empty url."),
                        PATH,
                    ));
                }
            }
            _ => diagnostics.push(diagnostic(
                "mcp_server_transport_invalid",
                &format!(
                    "Server '{name}' needs type stdio, streamable-http, or sse."
                ),
                PATH,
            )),
        }
    }
}

/// Mirrors the installation's stdio command rule: a bare executable name, or a
/// `./`-relative path that stays inside the package.
fn valid_mcp_stdio_command(value: &str) -> bool {
    if let Some(relative) = value.strip_prefix("./") {
        return !relative.is_empty() && !path_escapes(relative);
    }
    !value.contains('/') && !value.contains('\\') && !value.starts_with('.')
}

fn valid_mcp_cwd(value: &str) -> bool {
    for prefix in ["./", "${PLUGIN_ROOT}/", "${PLUGIN_DATA}/"] {
        if let Some(relative) = value.strip_prefix(prefix) {
            return !path_escapes(relative);
        }
    }
    value == "${PLUGIN_ROOT}" || value == "${PLUGIN_DATA}"
}

fn path_escapes(relative: &str) -> bool {
    relative.starts_with('/')
        || relative.contains('\\')
        || relative.split('/').any(|segment| segment == "..")
}

fn valid_mcp_server_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && !value.contains("--")
        && !value.contains("..")
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && value
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
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

    fn team_files(member_frontmatter: &str, members_in_manifest: &[&str]) -> Vec<DraftFile> {
        vec![
            DraftFile {
                path: "team.json".into(),
                content: serde_json::json!({
                    "lead": "acme",
                    "members": members_in_manifest,
                })
                .to_string(),
            },
            DraftFile {
                path: "agents/acme.md".into(),
                content: "---\nname: acme\nrole: lead\n---\n\nLead.\n".into(),
            },
            DraftFile {
                path: "agents/acme-specialist.md".into(),
                content: format!("---\nname: acme-specialist\nrole: member\n{member_frontmatter}---\n\nMember.\n"),
            },
            DraftFile {
                path: RESOURCE_MODE_SCOPE_FILENAME.into(),
                content: "{\"modes\":[\"work\"]}".into(),
            },
        ]
    }

    fn codes(validation: &ResourceValidation) -> Vec<&str> {
        validation
            .diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect()
    }

    #[test]
    fn accepts_a_team_whose_members_name_their_lead() {
        let files = team_files("lead: acme\n", &["acme-specialist"]);
        let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
        assert!(validation.valid, "{:?}", validation.diagnostics);
    }

    #[test]
    fn rejects_a_team_member_that_omits_its_lead() {
        let files = team_files("", &["acme-specialist"]);
        let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
        assert!(!validation.valid);
        assert!(codes(&validation).contains(&"team_member_lead_missing"));
    }

    #[test]
    fn rejects_a_team_member_that_names_a_foreign_lead() {
        let files = team_files("lead: evoflux\n", &["acme-specialist"]);
        let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
        assert!(!validation.valid);
        assert!(codes(&validation).contains(&"team_member_lead_mismatch"));
    }

    #[test]
    fn rejects_a_team_manifest_that_disagrees_with_its_agents() {
        let files = team_files("lead: acme\n", &["someone-else"]);
        let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
        assert!(!validation.valid);
        assert!(codes(&validation).contains(&"team_manifest_members_mismatch"));
    }

    #[test]
    fn rejects_a_team_whose_lead_does_not_match_the_slug() {
        let files = team_files("lead: acme\n", &["acme-specialist"]);
        let validation = validate_draft(ResourceKind::AgentTeam, "other", 1, &files);
        assert!(!validation.valid);
        assert!(codes(&validation).contains(&"team_lead_slug_mismatch"));
    }

    #[test]
    fn rejects_a_team_that_pins_the_model_or_thinking_level() {
        for field in ["model: openai:gpt-5", "thinking_level: high", "fallback_model: x:y"] {
            let files = team_files(&format!("lead: acme
{field}
"), &["acme-specialist"]);
            let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
            assert!(!validation.valid, "{field} was accepted");
            assert!(
                codes(&validation).contains(&"team_model_is_client_owned"),
                "{field}: {:?}",
                validation.diagnostics
            );
        }
    }

    #[test]
    fn rejects_a_team_without_exactly_one_lead() {
        let mut files = team_files("lead: acme\n", &["acme-specialist"]);
        files[2].content = "---\nname: acme-specialist\nrole: lead\n---\n\nSecond lead.\n".into();
        let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
        assert!(!validation.valid);
        assert!(codes(&validation).contains(&"team_lead_count_invalid"));
    }

    #[test]
    fn team_starter_files_validate_and_scope_members_to_their_lead() {
        let files = starter_files(ResourceKind::AgentTeam, "acme", "Acme");
        let validation = validate_draft(ResourceKind::AgentTeam, "acme", 1, &files);
        assert!(validation.valid, "{:?}", validation.diagnostics);
        let member = files
            .iter()
            .find(|file| file.path == "agents/acme-specialist.md")
            .expect("starter member");
        assert!(member.content.contains("lead: acme\n"));
    }

    fn plugin_draft(mcp: Option<serde_json::Value>) -> Vec<DraftFile> {
        let mut files = vec![
            DraftFile {
                path: "plugin.json".into(),
                content: serde_json::json!({
                    "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
                    "name": "review-tools",
                    "version": "0.1.0",
                    "description": "Review tooling",
                    "extensions": {}
                })
                .to_string(),
            },
            DraftFile {
                path: "skills/review-tools/SKILL.md".into(),
                content: "---
name: review-tools
description: Review
---

Body.
".into(),
            },
        ];
        if let Some(mcp) = mcp {
            files.push(DraftFile {
                path: "mcp.json".into(),
                content: mcp.to_string(),
            });
        }
        files
    }

    fn plugin_codes(mcp: Option<serde_json::Value>) -> Vec<String> {
        validate_draft(ResourceKind::Plugin, "review-tools", 1, &plugin_draft(mcp))
            .diagnostics
            .into_iter()
            .map(|item| item.code)
            .collect()
    }

    #[test]
    fn an_absolute_stdio_command_is_rejected_because_the_installer_refuses_it() {
        // Observed live: EvoFlux answers "command must be a bare executable
        // name or begin with './'", so a publish must not get that far.
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {
                "echo": {
                    "type": "stdio",
                    "command": r"C:\Users\dev\python.exe",
                    "args": ["server.py"]
                }
            }
        })));
        assert!(codes.contains(&"mcp_server_command_invalid".to_string()));
    }

    #[test]
    fn a_package_relative_stdio_command_is_accepted() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"echo": {"type": "stdio", "command": "./bin/echo"}}
        })));
        assert!(codes.is_empty(), "unexpected diagnostics: {codes:?}");
    }

    #[test]
    fn a_stdio_command_that_escapes_the_package_is_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"echo": {"type": "stdio", "command": "./../../evil"}}
        })));
        assert!(codes.contains(&"mcp_server_command_invalid".to_string()));
    }

    #[test]
    fn reserved_plugin_env_names_are_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {
                "echo": {"type": "stdio", "command": "node", "env": {"PLUGIN_ROOT": "/tmp"}}
            }
        })));
        assert!(codes.contains(&"mcp_server_env_reserved".to_string()));
    }

    #[test]
    fn a_cwd_outside_the_plugin_roots_is_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"echo": {"type": "stdio", "command": "node", "cwd": "/etc"}}
        })));
        assert!(codes.contains(&"mcp_server_cwd_invalid".to_string()));
    }

    #[test]
    fn the_plugin_data_and_plugin_root_cwd_forms_are_accepted() {
        for cwd in ["./work", "${PLUGIN_ROOT}", "${PLUGIN_DATA}/cache"] {
            let codes = plugin_codes(Some(serde_json::json!({
                "$schema": MCP_SCHEMA_ID,
                "mcpServers": {"echo": {"type": "stdio", "command": "node", "cwd": cwd}}
            })));
            assert!(codes.is_empty(), "{cwd} produced {codes:?}");
        }
    }

    #[test]
    fn a_plugin_without_mcp_json_is_still_valid() {
        assert!(validate_draft(ResourceKind::Plugin, "review-tools", 1, &plugin_draft(None)).valid);
    }

    #[test]
    fn a_well_formed_mcp_package_validates() {
        let mcp = serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {
                "review-linter": {"type": "stdio", "command": "node", "args": ["lint.js"]},
                "release-notes": {"type": "streamable-http", "url": "https://example.test/mcp"}
            }
        });
        assert!(validate_draft(ResourceKind::Plugin, "review-tools", 1, &plugin_draft(Some(mcp))).valid);
    }

    #[test]
    fn malformed_mcp_json_is_rejected_instead_of_publishing_zero_servers() {
        // The consumer-side failure this prevents: a broken mcp.json parses to
        // no server names at all, so the release advertises none and every
        // Agent referencing one finds it missing.
        let files = {
            let mut files = plugin_draft(None);
            files.push(DraftFile {
                path: "mcp.json".into(),
                content: "{ not json".into(),
            });
            files
        };
        let validation = validate_draft(ResourceKind::Plugin, "review-tools", 1, &files);
        assert!(!validation.valid);
        assert!(validation
            .diagnostics
            .iter()
            .any(|item| item.code == "mcp_json_invalid"));
        assert!(plugin_mcp_server_names(&files).is_empty());
    }

    #[test]
    fn an_mcp_server_without_a_command_is_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"review-linter": {"type": "stdio"}}
        })));
        assert!(codes.contains(&"mcp_server_command_missing".to_string()));
    }

    #[test]
    fn an_mcp_server_without_a_url_is_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"release-notes": {"type": "streamable-http"}}
        })));
        assert!(codes.contains(&"mcp_server_url_missing".to_string()));
    }

    #[test]
    fn an_unknown_mcp_transport_is_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"review-linter": {"command": "node"}}
        })));
        assert!(codes.contains(&"mcp_server_transport_invalid".to_string()));
    }

    #[test]
    fn the_mcp_schema_identifier_must_match_what_evoflux_accepts() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": "https://example.test/mcp.json",
            "mcpServers": {"review-linter": {"type": "stdio", "command": "node"}}
        })));
        assert!(codes.contains(&"mcp_schema_invalid".to_string()));
    }

    #[test]
    fn extra_top_level_mcp_keys_are_rejected_the_way_the_installer_rejects_them() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"review-linter": {"type": "stdio", "command": "node"}},
            "extra": true
        })));
        assert!(codes.contains(&"mcp_top_level_invalid".to_string()));
    }

    #[test]
    fn an_empty_mcp_server_map_warns_without_blocking_publish() {
        let validation = validate_draft(
            ResourceKind::Plugin,
            "review-tools",
            1,
            &plugin_draft(Some(serde_json::json!({
                "$schema": MCP_SCHEMA_ID,
                "mcpServers": {}
            }))),
        );
        assert!(validation.valid);
        assert!(validation
            .diagnostics
            .iter()
            .any(|item| item.code == "mcp_servers_empty"));
    }

    #[test]
    fn an_invalid_mcp_server_name_is_rejected() {
        let codes = plugin_codes(Some(serde_json::json!({
            "$schema": MCP_SCHEMA_ID,
            "mcpServers": {"Review Linter": {"type": "stdio", "command": "node"}}
        })));
        assert!(codes.contains(&"mcp_server_name_invalid".to_string()));
    }

    #[test]
    fn plugin_payload_carries_the_server_names_agents_must_reference() {
        let files = vec![
            DraftFile {
                path: "plugin.json".into(),
                content: "{}".into(),
            },
            DraftFile {
                path: "mcp.json".into(),
                content: serde_json::json!({
                    "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
                    "mcpServers": {"review-bot": {}, "audit-bot": {}}
                })
                .to_string(),
            },
        ];
        assert_eq!(
            plugin_mcp_server_names(&files),
            vec!["audit-bot".to_string(), "review-bot".to_string()]
        );

        let payload = resource_storage_payload(
            ResourceKind::Plugin,
            "review",
            "1.0.0",
            ResourceStorageArtifact {
                key: "k",
                sha256: &"a".repeat(64),
                size: 1,
                media_type: "application/vnd.evoflux.plugin+zip",
            },
            &files,
        );
        assert_eq!(
            payload["mcp_servers"],
            serde_json::json!(["audit-bot", "review-bot"])
        );
    }

    #[test]
    fn a_package_without_mcp_declares_no_servers() {
        let files = vec![DraftFile {
            path: "plugin.json".into(),
            content: "{}".into(),
        }];
        assert!(plugin_mcp_server_names(&files).is_empty());
        let payload = resource_storage_payload(
            ResourceKind::Plugin,
            "review",
            "1.0.0",
            ResourceStorageArtifact {
                key: "k",
                sha256: &"a".repeat(64),
                size: 1,
                media_type: "application/vnd.evoflux.plugin+zip",
            },
            &files,
        );
        assert!(payload.get("mcp_servers").is_none());
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
            ResourceKind::AgentTeam,
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
    fn team_validation_matches_evoflux_role_and_layout_contract() {
        let files = vec![
            DraftFile {
                path: "team.json".into(),
                content: serde_json::json!({"lead": "reviewer", "members": []}).to_string(),
            },
            DraftFile {
                path: "agents/reviewer.md".into(),
                content: "---\nname: reviewer\nrole: worker\ndescription: Review changes.\n---\n\nReview.\n"
                    .into(),
            },
            DraftFile {
                path: "notes.md".into(),
                content: "Not part of an EvoFlux Team definition.\n".into(),
            },
        ];
        let result = validate_draft(ResourceKind::AgentTeam, "reviewer", 0, &files);
        assert!(!result.valid);
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "team_source_layout_invalid"));
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
    fn target_modes_use_the_evoflux_work_and_coding_contract() {
        let mut files = starter_files(ResourceKind::AgentTeam, "reviewer", "Reviewer");
        set_target_modes(&mut files, &[ResourceTargetMode::Coding]);
        let result = validate_draft(ResourceKind::AgentTeam, "reviewer", 0, &files);
        assert!(result.valid, "{:?}", result.diagnostics);
        let scope = files
            .iter()
            .find(|file| file.path == RESOURCE_MODE_SCOPE_FILENAME)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&scope.content).unwrap(),
            serde_json::json!({ "modes": ["coding"] })
        );
    }

    /// A draft still carrying the retired `aim` mode is invalid, and the
    /// diagnostic points at the file the author has to edit.
    #[test]
    fn a_draft_still_naming_the_retired_aim_mode_is_reported() {
        let mut files = starter_files(ResourceKind::AgentTeam, "reviewer", "Reviewer");
        for file in files.iter_mut() {
            if file.path == RESOURCE_MODE_SCOPE_FILENAME {
                file.content = "{\"modes\": [\"work\", \"aim\"]}".into();
            }
        }
        let result = validate_draft(ResourceKind::AgentTeam, "reviewer", 0, &files);
        assert!(!result.valid);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|item| item.code == "resource_modes_invalid")
            .expect("the retired mode is reported");
        assert_eq!(
            diagnostic.path.as_deref(),
            Some(RESOURCE_MODE_SCOPE_FILENAME)
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
    fn bundle_manifest_is_sorted_content_addressed_and_media_typed() {
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
        let bundle = resource_bundle(
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
    fn bundle_excludes_non_portable_resource_kinds() {
        assert!(resource_bundle(
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

    /// A nested entry has to keep the archive's own separator. `PathBuf` joins
    /// with a backslash on Windows, which `safe_relative_path` refuses, so
    /// without this every ZIP with a subdirectory failed to import there.
    #[test]
    fn a_nested_zip_entry_keeps_forward_slashes_on_every_platform() {
        let native = std::path::PathBuf::from("skills")
            .join("manifest-proof")
            .join("SKILL.md");
        let path = zip_entry_path(&native).expect("utf-8 path");
        assert_eq!(path, "skills/manifest-proof/SKILL.md");
        assert!(safe_relative_path(&path));
    }
}
