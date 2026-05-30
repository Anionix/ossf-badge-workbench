use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use html_escape::encode_text;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TEMPLATE: &str = "templates/openssf-checksheet.template.yml";
const DEFAULT_CATALOG: &str = "data/openssf/criteria-catalog.yml";
const DEFAULT_EXAMPLE: &str = "examples/openssf-checksheet.example.yml";
const DEFAULT_BASELINE_CRITERIA_URL: &str =
    "https://raw.githubusercontent.com/coreinfrastructure/best-practices-badge/main/criteria/baseline_criteria.yml";
const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Validate {
        #[arg(long, default_value = DEFAULT_TEMPLATE)]
        template: PathBuf,
        #[arg(long, default_value = DEFAULT_CATALOG)]
        catalog: PathBuf,
        #[arg(long)]
        checksheet: Vec<PathBuf>,
    },
    Fill {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        project_json_url: Option<String>,
        #[arg(long)]
        project_json_file: Option<PathBuf>,
        #[arg(long)]
        repo_url: String,
        #[arg(long)]
        homepage_url: String,
        #[arg(long)]
        badge_level: String,
        #[arg(long = "old-ref")]
        old_refs: Vec<String>,
        #[arg(long)]
        signed_release_version: Option<String>,
        #[arg(long, default_value = DEFAULT_TEMPLATE)]
        template: PathBuf,
        #[arg(long, default_value = DEFAULT_CATALOG)]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Proposals {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = ProposalMode::Criterion)]
        mode: ProposalMode,
        #[arg(long)]
        include_unknown: bool,
        #[arg(long)]
        out: PathBuf,
    },
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    Render {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
    CompareReadback {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        project_json_url: Option<String>,
        #[arg(long)]
        project_json_file: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    SyncUpstream {
        #[arg(long, default_value = DEFAULT_CATALOG)]
        catalog: PathBuf,
        #[arg(long)]
        baseline_criteria_url: Option<String>,
        #[arg(long)]
        baseline_criteria_file: Option<PathBuf>,
        #[arg(long, default_value = DEFAULT_CATALOG)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum ProposalMode {
    Criterion,
    Step,
}

#[derive(Subcommand)]
enum ExportCommand {
    BestpracticesJson {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct Checksheet {
    schema_version: u64,
    project: Project,
    #[serde(default)]
    source: Option<Source>,
    choice_sets: ChoiceSets,
    columns: Vec<Column>,
    workflow_steps: Vec<WorkflowStep>,
    final_readback_rows: Vec<FinalReadbackRow>,
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Project {
    project_id: String,
    target_repo: String,
    target_homepage: String,
    expected_badge_level: String,
    old_references: Vec<String>,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Source {
    #[serde(default)]
    project_json_url: String,
    #[serde(default)]
    generated_by: String,
    #[serde(default)]
    generated_at: String,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChoiceSets {
    track: Vec<String>,
    level: Vec<String>,
    category: Vec<String>,
    status: Vec<String>,
    evidence_type: Vec<String>,
    boolean_choice: Vec<String>,
    save_state: Vec<String>,
    review_decision: Vec<String>,
    #[serde(default)]
    autofill_confidence: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Column {
    name: String,
    required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    choices: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct WorkflowStep {
    step: u64,
    section: String,
    rows: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct FinalReadbackRow {
    criterion_key: String,
    category: String,
    pass_value: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Row {
    project_id: String,
    target_repo: String,
    track: String,
    level: String,
    category: String,
    criterion_key: String,
    current_status: String,
    target_status: String,
    current_justification: String,
    target_justification: String,
    evidence_type: String,
    #[serde(default)]
    evidence_url: String,
    stale_reference: String,
    manual_proof_required: String,
    save_state: String,
    review_decision: String,
    #[serde(default)]
    save_url: String,
    #[serde(default)]
    owner: String,
    checked: String,
    #[serde(default)]
    notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    save_step: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    autofill_rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    autofill_confidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    autofill_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Catalog {
    schema_version: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    source: BTreeMap<String, String>,
    criteria: Vec<CatalogCriterion>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CatalogCriterion {
    key: String,
    track: String,
    level: String,
    category: String,
    evidence_type: String,
    manual_proof_required: String,
    safe_default_decision: String,
    edit_path: String,
    autofill_rule_id: String,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Validate {
            template,
            catalog,
            checksheet,
        } => validate_command(&template, &catalog, checksheet),
        Command::Fill {
            project_id,
            project_json_url,
            project_json_file,
            repo_url,
            homepage_url,
            badge_level,
            old_refs,
            signed_release_version,
            template,
            catalog,
            output,
        } => fill_command(FillArgs {
            project_id,
            project_json_url,
            project_json_file,
            repo_url,
            homepage_url,
            badge_level,
            old_refs,
            signed_release_version,
            template,
            catalog,
            output,
        }),
        Command::Proposals {
            input,
            mode,
            include_unknown,
            out,
        } => proposals_command(&input, mode, include_unknown, &out),
        Command::Export { command } => match command {
            ExportCommand::BestpracticesJson { input, out } => {
                export_bestpractices_json_command(&input, &out)
            }
        },
        Command::Render { input, out_dir } => render_command(&input, &out_dir),
        Command::CompareReadback {
            input,
            project_json_url,
            project_json_file,
            out,
        } => compare_readback_command(
            &input,
            project_json_url.as_deref(),
            project_json_file.as_deref(),
            out.as_deref(),
        ),
        Command::SyncUpstream {
            catalog,
            baseline_criteria_url,
            baseline_criteria_file,
            output,
            dry_run,
        } => sync_upstream_command(SyncUpstreamArgs {
            catalog,
            baseline_criteria_url,
            baseline_criteria_file,
            output,
            dry_run,
        }),
    }
}

struct FillArgs {
    project_id: String,
    project_json_url: Option<String>,
    project_json_file: Option<PathBuf>,
    repo_url: String,
    homepage_url: String,
    badge_level: String,
    old_refs: Vec<String>,
    signed_release_version: Option<String>,
    template: PathBuf,
    catalog: PathBuf,
    output: PathBuf,
}

struct SyncUpstreamArgs {
    catalog: PathBuf,
    baseline_criteria_url: Option<String>,
    baseline_criteria_file: Option<PathBuf>,
    output: PathBuf,
    dry_run: bool,
}

fn validate_command(template: &Path, catalog: &Path, checksheets: Vec<PathBuf>) -> Result<()> {
    let template_sheet = read_checksheet(template)?;
    let catalog = read_catalog(catalog)?;
    validate_catalog(&catalog)?;
    validate_checksheet(&template_sheet, Some(&catalog))
        .with_context(|| format!("validating {}", template.display()))?;

    let mut paths = checksheets;
    if paths.is_empty() && Path::new(DEFAULT_EXAMPLE).exists() {
        paths.push(PathBuf::from(DEFAULT_EXAMPLE));
    }

    for path in paths {
        let sheet = read_checksheet(&path)?;
        validate_checksheet(&sheet, Some(&catalog))
            .with_context(|| format!("validating {}", path.display()))?;
    }

    println!("validation passed");
    Ok(())
}

fn fill_command(args: FillArgs) -> Result<()> {
    let mut sheet = read_checksheet(&args.template)?;
    let catalog = read_catalog(&args.catalog)?;
    let catalog_by_key = catalog_by_key(&catalog);
    let json_url = args.project_json_url.clone().unwrap_or_default();
    let project_json = read_project_json(
        args.project_json_url.as_deref(),
        args.project_json_file.as_deref(),
    )?;
    let map = project_json
        .as_object()
        .ok_or_else(|| anyhow!("project JSON must be an object"))?;

    sheet.project.project_id = args.project_id.clone();
    sheet.project.target_repo = args.repo_url.clone();
    sheet.project.target_homepage = args.homepage_url.clone();
    sheet.project.expected_badge_level = args.badge_level.clone();
    sheet.project.old_references = args.old_refs.clone();
    sheet.project.notes =
        "Generated for an OpenSSF Best Practices badge refresh. UI saves remain manual.".into();
    sheet.source = Some(Source {
        project_json_url: json_url,
        generated_by: "ossf-badge-workbench".into(),
        generated_at: generated_at(),
        notes: "Conservative autofill: uncertain, private, and admin-only claims are deferred."
            .into(),
    });

    let mut rows = Vec::new();
    let mut status_keys: Vec<_> = map
        .keys()
        .filter(|key| key.ends_with("_status"))
        .map(|key| osps_keys::readback_key(key.trim_end_matches("_status")))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    status_keys.sort_by_key(|key| catalog_sort_key(&catalog, key));

    for key in status_keys {
        let current_status = json_field_string(map, &key, "status").unwrap_or_else(|| "?".into());
        let current_justification =
            json_field_string(map, &key, "justification").unwrap_or_default();
        let criterion = catalog_by_key
            .get(&key)
            .cloned()
            .unwrap_or_else(|| fallback_criterion(&key));
        let stale = contains_old_ref(&current_justification, &args.old_refs);
        let decision = decision_for(&current_status, stale, &criterion);
        let target_status = if is_gold_social_blocker(&key) {
            "Unmet".to_string()
        } else if criterion.manual_proof_required == "yes" {
            "?".to_string()
        } else {
            current_status.clone()
        };
        let save_state = if target_status != current_status {
            "drafted".into()
        } else {
            save_state_for(stale, &criterion, &decision)
        };
        let signed_release_version = args
            .signed_release_version
            .as_deref()
            .filter(|_| key == "signed_releases");
        let target_justification = if let Some(version) = signed_release_version {
            signed_release_justification(version, &args.repo_url)
        } else if is_gold_social_blocker(&key) {
            gold_blocker_justification(&key, &args.repo_url)
        } else if stale {
            current_justification.replace_old_refs(&args.old_refs, &args.repo_url)
        } else {
            current_justification.clone()
        };
        let evidence_url = if let Some(version) = signed_release_version {
            format!(
                "{}/releases/tag/{}",
                args.repo_url.trim_end_matches('/'),
                version
            )
        } else if is_gold_social_blocker(&key) {
            format!(
                "{}/blob/main/GOVERNANCE.md",
                args.repo_url.trim_end_matches('/')
            )
        } else if stale {
            evidence_url_for(&key, map).replace_old_refs(&args.old_refs, &args.repo_url)
        } else {
            evidence_url_for(&key, map)
        };
        let save_step = save_step_for_level(&criterion.level);
        rows.push(Row {
            project_id: args.project_id.clone(),
            target_repo: args.repo_url.clone(),
            track: criterion.track.clone(),
            level: criterion.level.clone(),
            category: criterion.category.clone(),
            criterion_key: key.clone(),
            current_status,
            target_status,
            current_justification,
            target_justification,
            evidence_type: criterion.evidence_type.clone(),
            evidence_url,
            stale_reference: yes_no(stale),
            manual_proof_required: criterion.manual_proof_required.clone(),
            save_state,
            review_decision: decision,
            save_url: format!(
                "https://www.bestpractices.dev/en/projects/{}/{}",
                args.project_id, criterion.edit_path
            ),
            owner: "maintainer".into(),
            checked: "no".into(),
            notes: notes_for(stale, &criterion),
            save_step: Some(save_step),
            autofill_rule_id: Some(criterion.autofill_rule_id.clone()),
            autofill_confidence: Some(if criterion.manual_proof_required == "yes" {
                "manual".into()
            } else if stale {
                "medium".into()
            } else {
                "low".into()
            }),
            autofill_reason: Some(reason_for(stale, &criterion)),
        });
    }

    rows.extend(readback_rows(&sheet.project));
    sheet.rows = rows;
    validate_checksheet(&sheet, Some(&catalog))?;
    write_yaml(&args.output, &sheet)?;
    println!("wrote {}", args.output.display());
    Ok(())
}

fn proposals_command(
    input: &Path,
    mode: ProposalMode,
    include_unknown: bool,
    out: &Path,
) -> Result<()> {
    let sheet = read_checksheet(input)?;
    validate_checksheet(&sheet, None)?;
    let mut text = String::new();
    text.push_str("# OpenSSF Proposal URLs\n\n");
    text.push_str(
        "Generated by `ossf-badge-workbench`. Review each URL before saving in the OpenSSF UI.\n\n",
    );
    match mode {
        ProposalMode::Criterion => {
            for row in sheet
                .rows
                .iter()
                .filter(|row| is_proposal_row(row, include_unknown))
            {
                let url = proposal_url(&sheet.project.project_id, row, std::slice::from_ref(row));
                text.push_str(&format!("## `{}`\n\n<{}>\n\n", row.criterion_key, url));
            }
        }
        ProposalMode::Step => {
            let mut groups: BTreeMap<u64, Vec<Row>> = BTreeMap::new();
            for row in sheet
                .rows
                .iter()
                .filter(|row| is_proposal_row(row, include_unknown))
            {
                groups
                    .entry(row.save_step.unwrap_or(save_step_for_level(&row.level)))
                    .or_default()
                    .push(row.clone());
            }
            for (step, rows) in groups {
                let section = rows
                    .first()
                    .map(|row| edit_path_for_level(&row.level))
                    .unwrap_or("passing/edit");
                let url = proposal_url(&sheet.project.project_id, &rows[0], &rows);
                text.push_str(&format!("## Step {} `{}`\n\n<{}>\n\n", step, section, url));
            }
        }
    }
    write_text(out, &text)?;
    println!("wrote {}", out.display());
    Ok(())
}

fn export_bestpractices_json_command(input: &Path, out: &Path) -> Result<()> {
    let sheet = read_checksheet(input)?;
    validate_checksheet(&sheet, None)?;
    let exported = bestpractices_json(&sheet);
    write_text(out, &serde_json::to_string_pretty(&exported)?)?;
    println!("wrote {}", out.display());
    Ok(())
}

fn render_command(input: &Path, out_dir: &Path) -> Result<()> {
    let sheet = read_checksheet(input)?;
    validate_checksheet(&sheet, None)?;
    fs::create_dir_all(out_dir)?;
    render_csv(&sheet, &out_dir.join("checksheet.csv"))?;
    render_html(&sheet, &out_dir.join("checksheet.html"))?;
    render_readback(&sheet, &out_dir.join("readback.json"))?;
    println!("wrote {}", out_dir.display());
    Ok(())
}

fn compare_readback_command(
    input: &Path,
    project_json_url: Option<&str>,
    project_json_file: Option<&Path>,
    out: Option<&Path>,
) -> Result<()> {
    let sheet = read_checksheet(input)?;
    validate_checksheet(&sheet, None)?;
    let project_json = read_project_json(project_json_url, project_json_file)?;
    let comparison = compare_readback(&sheet, &project_json)?;
    let raw = serde_json::to_string_pretty(&comparison)?;
    if let Some(out) = out {
        write_text(out, &raw)?;
        println!("wrote {}", out.display());
    } else {
        println!("{raw}");
    }

    if !comparison.missing.is_empty() || !comparison.mismatches.is_empty() {
        bail!(
            "readback comparison failed: {} missing, {} mismatched",
            comparison.missing.len(),
            comparison.mismatches.len()
        );
    }
    Ok(())
}

fn sync_upstream_command(args: SyncUpstreamArgs) -> Result<()> {
    let mut catalog = read_catalog(&args.catalog)?;
    validate_catalog(&catalog)?;
    let baseline_raw = read_upstream_yaml(
        args.baseline_criteria_url.as_deref(),
        args.baseline_criteria_file.as_deref(),
        DEFAULT_BASELINE_CRITERIA_URL,
    )?;
    let baseline_yaml: Value = serde_yaml::from_str(&baseline_raw)?;
    let synced_osps = sync_osps_baseline_criteria(&catalog, &baseline_yaml)?;
    let osps_keys: BTreeSet<_> = synced_osps
        .iter()
        .map(|criterion| osps_keys::readback_key(&criterion.key))
        .collect();

    catalog
        .criteria
        .retain(|criterion| !osps_keys.contains(&osps_keys::readback_key(&criterion.key)));
    catalog.criteria.extend(synced_osps);
    catalog.source.insert(
        "baseline_criteria_url".into(),
        args.baseline_criteria_url
            .unwrap_or_else(|| DEFAULT_BASELINE_CRITERIA_URL.into()),
    );
    catalog.source.insert(
        "generated_by".into(),
        "ossf-badge-workbench sync-upstream".into(),
    );
    catalog.source.insert("generated_at".into(), generated_at());
    catalog.source.insert(
        "notes".into(),
        "OSPS baseline criteria refreshed from upstream while preserving local metadata for existing keys.".into(),
    );
    validate_catalog(&catalog)?;

    if args.dry_run {
        println!(
            "would write {} criteria ({} OSPS) to {}",
            catalog.criteria.len(),
            osps_keys.len(),
            args.output.display()
        );
    } else {
        write_catalog(&args.output, &catalog)?;
        println!(
            "wrote {} criteria ({} OSPS) to {}",
            catalog.criteria.len(),
            osps_keys.len(),
            args.output.display()
        );
    }
    Ok(())
}

fn read_checksheet(path: &Path) -> Result<Checksheet> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn read_catalog(path: &Path) -> Result<Catalog> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn read_project_json(url: Option<&str>, file: Option<&Path>) -> Result<Value> {
    match (url, file) {
        (Some(url), None) => {
            let response = ureq::get(url)
                .call()
                .with_context(|| format!("fetching {url}"))?;
            let raw = response
                .into_string()
                .with_context(|| format!("reading {url}"))?;
            serde_json::from_str(&raw).with_context(|| format!("decoding {url}"))
        }
        (None, Some(file)) => {
            let raw =
                fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", file.display()))
        }
        (Some(_), Some(_)) => {
            bail!("use either --project-json-url or --project-json-file, not both")
        }
        (None, None) => bail!("missing --project-json-url or --project-json-file"),
    }
}

fn validate_catalog(catalog: &Catalog) -> Result<()> {
    if catalog.schema_version != 1 {
        bail!(
            "unsupported catalog schema_version {}",
            catalog.schema_version
        );
    }
    let mut seen = BTreeSet::new();
    for criterion in &catalog.criteria {
        if !seen.insert(osps_keys::readback_key(&criterion.key)) {
            bail!("duplicate catalog key {}", criterion.key);
        }
        ensure_enum("track", &criterion.track, &["metal", "osps"])?;
        ensure_enum(
            "level",
            &criterion.level,
            &[
                "passing",
                "silver",
                "gold",
                "baseline-1",
                "baseline-2",
                "baseline-3",
            ],
        )?;
        ensure_enum(
            "manual_proof_required",
            &criterion.manual_proof_required,
            &["yes", "no"],
        )?;
    }
    Ok(())
}

fn validate_checksheet(sheet: &Checksheet, catalog: Option<&Catalog>) -> Result<()> {
    if sheet.schema_version != 1 {
        bail!(
            "unsupported checksheet schema_version {}",
            sheet.schema_version
        );
    }
    let choices = &sheet.choice_sets;
    let mut seen = BTreeSet::new();
    let catalog_keys: BTreeSet<_> = catalog
        .map(|catalog| {
            catalog
                .criteria
                .iter()
                .map(|criterion| osps_keys::readback_key(&criterion.key))
                .collect()
        })
        .unwrap_or_default();

    for row in &sheet.rows {
        let row_id = format!(
            "{}:{}",
            row.track,
            osps_keys::readback_key(&row.criterion_key)
        );
        if !seen.insert(row_id) {
            bail!(
                "duplicate row track/criterion_key {}:{}",
                row.track,
                row.criterion_key
            );
        }
        validate_choice("track", &row.track, &choices.track)?;
        validate_choice("level", &row.level, &choices.level)?;
        validate_choice("category", &row.category, &choices.category)?;
        validate_choice("current_status", &row.current_status, &choices.status)?;
        validate_choice("target_status", &row.target_status, &choices.status)?;
        validate_choice("evidence_type", &row.evidence_type, &choices.evidence_type)?;
        validate_choice(
            "stale_reference",
            &row.stale_reference,
            &choices.boolean_choice,
        )?;
        validate_choice(
            "manual_proof_required",
            &row.manual_proof_required,
            &choices.boolean_choice,
        )?;
        validate_choice("save_state", &row.save_state, &choices.save_state)?;
        validate_choice(
            "review_decision",
            &row.review_decision,
            &choices.review_decision,
        )?;
        validate_choice("checked", &row.checked, &choices.boolean_choice)?;
        if let Some(confidence) = &row.autofill_confidence {
            validate_choice(
                "autofill_confidence",
                confidence,
                &choices.autofill_confidence,
            )?;
        }
        if catalog.is_some()
            && row.track != "readback"
            && !catalog_keys.contains(&osps_keys::readback_key(&row.criterion_key))
        {
            bail!(
                "row {} is not present in criteria catalog",
                row.criterion_key
            );
        }
    }
    Ok(())
}

fn validate_choice(name: &str, value: &str, choices: &[String]) -> Result<()> {
    if choices.is_empty() || choices.iter().any(|choice| choice == value) {
        Ok(())
    } else {
        bail!("{name} has invalid value {value}");
    }
}

fn ensure_enum(name: &str, value: &str, choices: &[&str]) -> Result<()> {
    if choices.contains(&value) {
        Ok(())
    } else {
        bail!("{name} has invalid value {value}");
    }
}

fn catalog_by_key(catalog: &Catalog) -> HashMap<String, CatalogCriterion> {
    catalog
        .criteria
        .iter()
        .map(|criterion| (osps_keys::readback_key(&criterion.key), criterion.clone()))
        .collect()
}

fn catalog_sort_key(catalog: &Catalog, key: &str) -> (usize, String) {
    let normalized_key = osps_keys::readback_key(key);
    let index = catalog
        .criteria
        .iter()
        .position(|criterion| osps_keys::readback_key(&criterion.key) == normalized_key)
        .unwrap_or(usize::MAX);
    (index, normalized_key)
}

fn fallback_criterion(key: &str) -> CatalogCriterion {
    let is_osps = osps_keys::is_osps_key(key);
    CatalogCriterion {
        key: osps_keys::readback_key(key),
        track: if is_osps { "osps" } else { "metal" }.into(),
        level: if is_osps { "baseline-3" } else { "silver" }.into(),
        category: if is_osps { "osps_quality" } else { "identity" }.into(),
        evidence_type: "none".into(),
        manual_proof_required: "no".into(),
        safe_default_decision: "keep_unknown".into(),
        edit_path: if is_osps {
            "baseline-3/edit"
        } else {
            "silver/edit"
        }
        .into(),
        autofill_rule_id: "fallback-conservative".into(),
    }
}

fn json_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    })
}

fn json_field_string(
    map: &serde_json::Map<String, Value>,
    criterion_key: &str,
    suffix: &str,
) -> Option<String> {
    json_string(map.get(&osps_keys::readback_field(criterion_key, suffix)))
        .or_else(|| json_string(map.get(&osps_keys::proposal_field(criterion_key, suffix))))
}

fn contains_old_ref(value: &str, old_refs: &[String]) -> bool {
    old_refs
        .iter()
        .any(|old_ref| !old_ref.is_empty() && value.contains(old_ref))
}

trait ReplaceOldRefs {
    fn replace_old_refs(&self, old_refs: &[String], new_ref: &str) -> String;
}

impl ReplaceOldRefs for str {
    fn replace_old_refs(&self, old_refs: &[String], new_ref: &str) -> String {
        let mut value = self.to_string();
        let mut sorted_refs = old_refs.to_vec();
        sorted_refs.sort_by_key(|old_ref| std::cmp::Reverse(old_ref.len()));
        for old_ref in sorted_refs {
            let replacement = replacement_for_old_ref(&old_ref, new_ref);
            value = value.replace(&old_ref, &replacement);
        }
        value
    }
}

fn replacement_for_old_ref(old_ref: &str, target_repo_url: &str) -> String {
    if old_ref.starts_with("https://") || old_ref.starts_with("http://") {
        target_repo_url.to_string()
    } else if old_ref.contains('/') {
        target_repo_url
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/")
            .to_string()
    } else {
        target_repo_url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(target_repo_url)
            .to_string()
    }
}

fn decision_for(status: &str, stale: bool, criterion: &CatalogCriterion) -> String {
    if is_gold_social_blocker(&criterion.key) {
        "keep_unmet".into()
    } else if criterion.manual_proof_required == "yes" {
        "defer".into()
    } else if stale {
        "revise".into()
    } else if status == "N/A" {
        "keep_na".into()
    } else if status == "Unmet" {
        "keep_unmet".into()
    } else if status == "?" {
        "keep_unknown".into()
    } else {
        criterion.safe_default_decision.clone()
    }
}

fn is_gold_social_blocker(key: &str) -> bool {
    matches!(
        key,
        "bus_factor" | "contributors_unassociated" | "code_review_standards" | "two_person_review"
    )
}

fn gold_blocker_justification(key: &str, target_repo_url: &str) -> String {
    let governance_url = format!(
        "{}/blob/main/GOVERNANCE.md",
        target_repo_url.trim_end_matches('/')
    );
    let maintainer_url = format!(
        "{}/blob/main/MAINTAINERS.md",
        target_repo_url.trim_end_matches('/')
    );
    match key {
        "bus_factor" => format!(
            "Current bus factor is one. Private recovery supports continuity but is not claimed as a bus factor of two: {maintainer_url} {governance_url}"
        ),
        "contributors_unassociated" => format!(
            "The project does not yet have two significant unaffiliated contributors. This remains a Gold blocker: {governance_url}"
        ),
        "code_review_standards" => format!(
            "Solo-maintainer hybrid mode is documented, but the project does not yet claim full code-review standards covering review conduct, checked criteria, acceptance criteria, and enforcement evidence. This remains a Gold blocker: {governance_url}"
        ),
        "two_person_review" => format!(
            "Current operation is solo-maintainer hybrid mode. Pull requests and required checks are used, but normal changes do not require separate human approval. This remains a Gold blocker: {governance_url} {maintainer_url}"
        ),
        _ => "Gold social/governance criterion is not claimed as satisfied by the conservative autofill.".into(),
    }
}

fn signed_release_justification(version: &str, target_repo_url: &str) -> String {
    let repo_url = target_repo_url.trim_end_matches('/');
    format!(
        "Release artifacts for {version} and later are signed with Sigstore keyless Cosign bundles and documented verification instructions: {repo_url}/blob/main/docs/release-signing.md {repo_url}/blob/main/.github/workflows/release.yml {repo_url}/releases/tag/{version}"
    )
}

fn save_state_for(stale: bool, criterion: &CatalogCriterion, decision: &str) -> String {
    if criterion.manual_proof_required == "yes" || decision == "defer" {
        "deferred".into()
    } else if stale || decision == "revise" {
        "drafted".into()
    } else {
        "readback_passed".into()
    }
}

fn yes_no(value: bool) -> String {
    if value { "yes" } else { "no" }.into()
}

fn evidence_url_for(key: &str, map: &serde_json::Map<String, Value>) -> String {
    let justification = json_field_string(map, key, "justification").unwrap_or_default();
    justification
        .split_whitespace()
        .find(|part| part.starts_with("https://"))
        .unwrap_or("")
        .trim_matches(|ch| ch == '<' || ch == '>' || ch == ',' || ch == '.')
        .to_string()
}

fn save_step_for_level(level: &str) -> u64 {
    match level {
        "passing" => 1,
        "silver" => 2,
        "gold" => 5,
        "baseline-1" => 6,
        "baseline-2" => 7,
        "baseline-3" => 8,
        _ => 9,
    }
}

fn edit_path_for_level(level: &str) -> &'static str {
    match level {
        "passing" => "passing/edit",
        "silver" => "silver/edit",
        "gold" => "gold/edit",
        "baseline-1" => "baseline-1/edit",
        "baseline-2" => "baseline-2/edit",
        "baseline-3" => "baseline-3/edit",
        _ => "passing/edit",
    }
}

fn notes_for(stale: bool, criterion: &CatalogCriterion) -> String {
    if criterion.manual_proof_required == "yes" {
        "Needs maintainer/admin proof before marking Met.".into()
    } else if stale {
        "Current justification references an old repository or domain; review replacement before saving.".into()
    } else {
        String::new()
    }
}

fn reason_for(stale: bool, criterion: &CatalogCriterion) -> String {
    if criterion.manual_proof_required == "yes" {
        "Private or admin-only proof is required.".into()
    } else if stale {
        "Old reference detected in current justification.".into()
    } else {
        "Conservative carry-forward from current OpenSSF readback.".into()
    }
}

fn readback_rows(project: &Project) -> Vec<Row> {
    let readback = [
        ("repo_url", "identity", "Target repository URL"),
        ("homepage_url", "identity", "Target homepage URL"),
        ("badge_level", "identity", "Expected badge level"),
        ("stale_reference_count", "readback", "0"),
        ("overclaimed_gold_count", "gold_blockers", "0"),
        (
            "manual_proof_pending_count",
            "readback",
            "Known and documented",
        ),
    ];
    readback
        .into_iter()
        .map(|(key, category, pass_value)| Row {
            project_id: project.project_id.clone(),
            target_repo: project.target_repo.clone(),
            track: "readback".into(),
            level: "readback".into(),
            category: category.into(),
            criterion_key: key.into(),
            current_status: "?".into(),
            target_status: "Met".into(),
            current_justification: String::new(),
            target_justification: pass_value.into(),
            evidence_type: "none".into(),
            evidence_url: String::new(),
            stale_reference: "no".into(),
            manual_proof_required: "no".into(),
            save_state: "not_started".into(),
            review_decision: "accept".into(),
            save_url: String::new(),
            owner: "maintainer".into(),
            checked: "no".into(),
            notes: String::new(),
            save_step: Some(9),
            autofill_rule_id: Some("readback".into()),
            autofill_confidence: Some("manual".into()),
            autofill_reason: Some("Final readback guard row.".into()),
        })
        .collect()
}

fn proposal_url(project_id: &str, anchor_row: &Row, rows: &[Row]) -> String {
    let mut url = format!(
        "https://www.bestpractices.dev/en/projects/{}/{}?",
        project_id,
        edit_path_for_level(&anchor_row.level)
    );
    let mut params = Vec::new();
    let mut overrides = Vec::new();
    for row in rows {
        let param_key = osps_keys::proposal_key(&row.criterion_key);
        params.push((format!("{param_key}_status"), row.target_status.clone()));
        params.push((
            format!("{param_key}_justification"),
            row.target_justification.clone(),
        ));
        overrides.push(format!("{param_key}_status"));
    }
    params.push(("overrides".into(), overrides.join(",")));
    url.push_str(
        &params
            .into_iter()
            .map(|(key, value)| format!("{}={}", encode_query(&key), encode_query(&value)))
            .collect::<Vec<_>>()
            .join("&"),
    );
    url
}

fn encode_query(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

fn render_csv(sheet: &Checksheet, path: &Path) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    for row in &sheet.rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

fn render_html(sheet: &Checksheet, path: &Path) -> Result<()> {
    let mut html = String::new();
    html.push_str("<!doctype html><meta charset=\"utf-8\"><title>OpenSSF Checksheet</title>");
    html.push_str("<style>body{font-family:system-ui,sans-serif;margin:2rem}table{border-collapse:collapse;width:100%}th,td{border:1px solid #ddd;padding:.4rem;text-align:left;vertical-align:top}th{background:#f6f8fa}.yes{color:#9a3412}.Met{color:#166534}.Unmet{color:#991b1b}</style>");
    html.push_str(&format!(
        "<h1>OpenSSF Checksheet {}</h1><p>{}</p><table><thead><tr><th>level</th><th>category</th><th>criterion</th><th>current</th><th>target</th><th>decision</th><th>stale</th><th>manual</th></tr></thead><tbody>",
        encode_text(&sheet.project.project_id),
        encode_text(&sheet.project.target_repo)
    ));
    for row in &sheet.rows {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td class=\"{}\">{}</td><td>{}</td><td class=\"{}\">{}</td><td class=\"{}\">{}</td></tr>",
            encode_text(&row.level),
            encode_text(&row.category),
            encode_text(&row.criterion_key),
            encode_text(&row.current_status),
            encode_text(&row.current_status),
            encode_text(&row.target_status),
            encode_text(&row.target_status),
            encode_text(&row.review_decision),
            encode_text(&row.stale_reference),
            encode_text(&row.stale_reference),
            encode_text(&row.manual_proof_required),
            encode_text(&row.manual_proof_required),
        ));
    }
    html.push_str("</tbody></table>");
    write_text(path, &html)
}

fn render_readback(sheet: &Checksheet, path: &Path) -> Result<()> {
    let stale = sheet
        .rows
        .iter()
        .filter(|row| row.stale_reference == "yes")
        .count();
    let manual = sheet
        .rows
        .iter()
        .filter(|row| row.manual_proof_required == "yes")
        .count();
    let overclaimed_gold = sheet
        .rows
        .iter()
        .filter(|row| {
            row.level == "gold" && row.target_status == "Met" && row.review_decision != "accept"
        })
        .count();
    let value = serde_json::json!({
        "project_id": sheet.project.project_id,
        "repo_url": sheet.project.target_repo,
        "homepage_url": sheet.project.target_homepage,
        "badge_level": sheet.project.expected_badge_level,
        "row_count": sheet.rows.len(),
        "stale_reference_count": stale,
        "manual_proof_pending_count": manual,
        "overclaimed_gold_count": overclaimed_gold,
    });
    write_text(path, &serde_json::to_string_pretty(&value)?)
}

fn is_proposal_row(row: &Row, include_unknown: bool) -> bool {
    row.track != "readback" && (include_unknown || row.target_status != "?")
}

fn is_bestpractices_export_row(row: &Row) -> bool {
    row.track != "readback"
        && row.target_status != "?"
        && row.stale_reference == "no"
        && row.manual_proof_required == "no"
        && row.save_state != "deferred"
        && (row.review_decision == "accept"
            || (row.checked == "yes"
                && matches!(row.review_decision.as_str(), "keep_na" | "keep_unmet")))
}

fn bestpractices_json(sheet: &Checksheet) -> BTreeMap<String, Value> {
    let mut value = BTreeMap::new();
    for row in sheet
        .rows
        .iter()
        .filter(|row| is_bestpractices_export_row(row))
    {
        let key = osps_keys::proposal_key(&row.criterion_key);
        value.insert(
            format!("{key}_status"),
            Value::String(row.target_status.clone()),
        );
        value.insert(
            format!("{key}_justification"),
            Value::String(row.target_justification.clone()),
        );
    }
    value
}

#[derive(Debug, Serialize)]
struct ReadbackComparison {
    project_id: String,
    checked_rows: usize,
    matched: Vec<ReadbackMatch>,
    missing: Vec<ReadbackMissing>,
    mismatches: Vec<ReadbackMismatch>,
}

#[derive(Debug, Serialize)]
struct ReadbackMatch {
    criterion_key: String,
    field: String,
}

#[derive(Debug, Serialize)]
struct ReadbackMissing {
    criterion_key: String,
    field: String,
}

#[derive(Debug, Serialize)]
struct ReadbackMismatch {
    criterion_key: String,
    field: String,
    expected: String,
    actual: String,
}

fn compare_readback(sheet: &Checksheet, project_json: &Value) -> Result<ReadbackComparison> {
    let map = project_json
        .as_object()
        .ok_or_else(|| anyhow!("project JSON must be an object"))?;
    let mut comparison = ReadbackComparison {
        project_id: sheet.project.project_id.clone(),
        checked_rows: 0,
        matched: Vec::new(),
        missing: Vec::new(),
        mismatches: Vec::new(),
    };

    for row in sheet.rows.iter().filter(|row| row.track != "readback") {
        if row.target_status == "?" {
            continue;
        }
        comparison.checked_rows += 1;
        compare_readback_field(
            &mut comparison,
            map,
            &row.criterion_key,
            "status",
            &row.target_status,
        );
        if !row.target_justification.is_empty() {
            compare_readback_field(
                &mut comparison,
                map,
                &row.criterion_key,
                "justification",
                &row.target_justification,
            );
        }
    }

    Ok(comparison)
}

fn compare_readback_field(
    comparison: &mut ReadbackComparison,
    map: &serde_json::Map<String, Value>,
    criterion_key: &str,
    suffix: &str,
    expected: &str,
) {
    let field = osps_keys::readback_field(criterion_key, suffix);
    match json_field_string(map, criterion_key, suffix) {
        Some(actual) if actual == expected => comparison.matched.push(ReadbackMatch {
            criterion_key: osps_keys::readback_key(criterion_key),
            field,
        }),
        Some(actual) => comparison.mismatches.push(ReadbackMismatch {
            criterion_key: osps_keys::readback_key(criterion_key),
            field,
            expected: expected.into(),
            actual,
        }),
        None => comparison.missing.push(ReadbackMissing {
            criterion_key: osps_keys::readback_key(criterion_key),
            field,
        }),
    }
}

fn read_upstream_yaml(url: Option<&str>, file: Option<&Path>, default_url: &str) -> Result<String> {
    match (url, file) {
        (Some(_), Some(_)) => {
            bail!("use either --baseline-criteria-url or --baseline-criteria-file, not both")
        }
        (None, Some(file)) => {
            fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))
        }
        (Some(url), None) => read_url(url),
        (None, None) => read_url(default_url),
    }
}

fn read_url(url: &str) -> Result<String> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("fetching {url}"))?;
    response
        .into_string()
        .with_context(|| format!("reading {url}"))
}

fn sync_osps_baseline_criteria(
    catalog: &Catalog,
    baseline_yaml: &Value,
) -> Result<Vec<CatalogCriterion>> {
    let existing = catalog_by_key(catalog);
    let mut controls = Vec::new();
    collect_osps_controls(baseline_yaml, None, &mut controls);
    if controls.is_empty() {
        bail!("no OSPS baseline controls found in upstream criteria YAML");
    }

    let mut criteria = Vec::new();
    for control in controls {
        let readback_key = osps_keys::readback_key(&control.original_id);
        let mut criterion = existing
            .get(&readback_key)
            .cloned()
            .unwrap_or_else(|| fallback_criterion(&readback_key));
        criterion.key = readback_key;
        criterion.track = "osps".into();
        criterion.level = control.level;
        criterion.category = osps_category(&criterion.key).into();
        criterion.edit_path = format!("{}/edit", criterion.level);
        if criterion.autofill_rule_id == "fallback-conservative" {
            criterion.autofill_rule_id = "osps-upstream-conservative".into();
        }
        criteria.push(criterion);
    }
    Ok(criteria)
}

#[derive(Debug)]
struct UpstreamOspsControl {
    level: String,
    original_id: String,
}

fn collect_osps_controls(
    value: &Value,
    level: Option<&str>,
    controls: &mut Vec<UpstreamOspsControl>,
) {
    let Some(map) = value.as_object() else {
        return;
    };

    if let Some(original_id) = map.get("original_id").and_then(Value::as_str) {
        if osps_keys::is_osps_key(original_id) {
            controls.push(UpstreamOspsControl {
                level: level.unwrap_or("baseline-3").into(),
                original_id: original_id.into(),
            });
        }
        return;
    }

    for (key, value) in map {
        let nested_level = if key.starts_with("baseline-") {
            Some(key.as_str())
        } else {
            level
        };
        collect_osps_controls(value, nested_level, controls);
    }
}

fn osps_category(key: &str) -> &'static str {
    match osps_keys::proposal_key(key)
        .split('_')
        .nth(1)
        .unwrap_or_default()
    {
        "ac" => "osps_access_control",
        "br" => "osps_build_release",
        "do" => "osps_documentation",
        "gv" => "osps_governance",
        "le" => "osps_legal",
        "qa" => "osps_quality",
        "sa" => "osps_security_assessment",
        "vm" => "osps_vulnerability_management",
        _ => "osps_quality",
    }
}

fn write_yaml(path: &Path, sheet: &Checksheet) -> Result<()> {
    let raw = annotate_scanner_false_positives(serde_yaml::to_string(sheet)?);
    write_text(path, &raw)
}

fn write_catalog(path: &Path, catalog: &Catalog) -> Result<()> {
    let raw = annotate_scanner_false_positives(serde_yaml::to_string(catalog)?);
    write_text(path, &raw)
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file =
        fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

fn generated_at() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}

fn annotate_scanner_false_positives(raw: String) -> String {
    let raw = raw
        .replace(": yes\n", ": 'yes'\n")
        .replace(": no\n", ": 'no'\n")
        .replace("- yes\n", "- 'yes'\n")
        .replace("- no\n", "- 'no'\n");
    raw.replace(
        "criterion_key: vulnerabilities_fixed_60_days\n",
        "criterion_key: vulnerabilities_fixed_60_days # gitleaks:allow\n",
    )
}

mod osps_keys {
    pub fn is_osps_key(key: &str) -> bool {
        let key = key.trim();
        key.to_ascii_uppercase().starts_with("OSPS-")
            || key.to_ascii_lowercase().starts_with("osps_")
    }

    pub fn readback_key(key: &str) -> String {
        let base = strip_suffix(key);
        if !is_osps_key(base) {
            return base.to_string();
        }

        let parts = osps_parts(base);
        if parts.len() == 4 {
            format!(
                "OSPS-{}-{}.{}",
                parts[1].to_ascii_uppercase(),
                parts[2],
                parts[3]
            )
        } else {
            base.to_ascii_uppercase().replace('_', "-")
        }
    }

    pub fn proposal_key(key: &str) -> String {
        let base = strip_suffix(key);
        if is_osps_key(base) {
            osps_parts(base).join("_")
        } else {
            base.to_string()
        }
    }

    pub fn readback_field(key: &str, suffix: &str) -> String {
        format!("{}_{}", readback_key(key), suffix)
    }

    pub fn proposal_field(key: &str, suffix: &str) -> String {
        format!("{}_{}", proposal_key(key), suffix)
    }

    fn strip_suffix(key: &str) -> &str {
        key.trim()
            .strip_suffix("_status")
            .or_else(|| key.trim().strip_suffix("_justification"))
            .unwrap_or_else(|| key.trim())
    }

    fn osps_parts(key: &str) -> Vec<String> {
        strip_suffix(key)
            .to_ascii_lowercase()
            .replace(['-', '.'], "_")
            .split('_')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_osps_keys_to_form_params() {
        assert_eq!(osps_keys::proposal_key("OSPS-AC-01.01"), "osps_ac_01_01");
        assert_eq!(
            osps_keys::proposal_key("OSPS-AC-03.01_status"),
            "osps_ac_03_01"
        );
        assert_eq!(
            osps_keys::proposal_field("OSPS-AC-03.01_status", "status"),
            "osps_ac_03_01_status"
        );
        assert_eq!(
            osps_keys::readback_field("osps_ac_03_01_status", "status"),
            "OSPS-AC-03.01_status"
        );
        assert_eq!(osps_keys::proposal_key("repo_public"), "repo_public");
    }

    #[test]
    fn proposals_skip_unknown_targets_by_default() {
        let row = test_row("OSPS-AC-03.01", "?", "no", "defer");
        assert!(!is_proposal_row(&row, false));
        assert!(is_proposal_row(&row, true));
    }

    #[test]
    fn proposal_url_uses_internal_osps_keys_and_status_overrides() {
        let mut row = test_row("OSPS-AC-03.01", "Met", "yes", "accept");
        row.target_justification = "Branch protection enabled on main.".into();
        let url = proposal_url("42", &row, std::slice::from_ref(&row));
        assert!(url.contains("osps_ac_03_01_status=Met"));
        assert!(url.contains("osps_ac_03_01_justification=Branch"));
        assert!(url.contains("overrides=osps_ac_03_01_status"));
        assert!(!url.contains("overrides=osps_ac_03_01_status%2Cosps_ac_03_01_justification"));
    }

    #[test]
    fn bestpractices_export_uses_internal_keys_and_omits_unknown() {
        let rows = vec![
            test_row("OSPS-AC-03.01", "Met", "yes", "accept"),
            test_row("OSPS-AC-03.02", "?", "yes", "accept"),
            test_row("OSPS-BR-01.01", "Met", "no", "defer"),
            test_row("repo_public", "Met", "no", "accept"),
        ];
        let sheet = test_sheet(rows);
        let exported = bestpractices_json(&sheet);

        assert_eq!(
            exported.get("osps_ac_03_01_status"),
            Some(&Value::String("Met".into()))
        );
        assert_eq!(
            exported.get("repo_public_status"),
            Some(&Value::String("Met".into()))
        );
        assert!(!exported.contains_key("osps_ac_03_02_status"));
        assert!(!exported.contains_key("osps_br_01_01_status"));
    }

    #[test]
    fn bestpractices_export_omits_stale_deferred_and_manual_rows() {
        let mut stale = test_row("OSPS-AC-03.01", "Met", "yes", "accept");
        stale.stale_reference = "yes".into();
        let mut manual = test_row("OSPS-AC-03.02", "Met", "yes", "accept");
        manual.manual_proof_required = "yes".into();
        let mut deferred = test_row("OSPS-BR-01.01", "Met", "yes", "keep_unmet");
        deferred.save_state = "deferred".into();
        let keep_unmet = test_row("repo_public", "Unmet", "yes", "keep_unmet");

        let sheet = test_sheet(vec![stale, manual, deferred, keep_unmet]);
        let exported = bestpractices_json(&sheet);

        assert!(!exported.contains_key("osps_ac_03_01_status"));
        assert!(!exported.contains_key("osps_ac_03_02_status"));
        assert!(!exported.contains_key("osps_br_01_01_status"));
        assert_eq!(
            exported.get("repo_public_status"),
            Some(&Value::String("Unmet".into()))
        );
    }

    #[test]
    fn compare_readback_accepts_readback_and_internal_osps_keys() {
        let sheet = test_sheet(vec![test_row("OSPS-AC-03.01", "Met", "yes", "accept")]);
        let readback = serde_json::json!({
            "osps_ac_03_01_status": "Met",
            "osps_ac_03_01_justification": "Test justification"
        });
        let comparison = compare_readback(&sheet, &readback).expect("comparison should run");
        assert_eq!(comparison.checked_rows, 1);
        assert!(comparison.missing.is_empty());
        assert!(comparison.mismatches.is_empty());
        assert_eq!(comparison.matched.len(), 2);
    }

    #[test]
    fn sync_upstream_preserves_existing_osps_metadata() {
        let catalog = Catalog {
            schema_version: 1,
            source: BTreeMap::new(),
            criteria: vec![CatalogCriterion {
                key: "OSPS-AC-03.01".into(),
                track: "osps".into(),
                level: "baseline-1".into(),
                category: "osps_access_control".into(),
                evidence_type: "repo_file".into(),
                manual_proof_required: "no".into(),
                safe_default_decision: "accept".into(),
                edit_path: "baseline-1/edit".into(),
                autofill_rule_id: "osps-conservative".into(),
            }],
        };
        let upstream = serde_json::json!({
            "baseline-1": {
                "General": {
                    "Controls": {
                        "osps_ac_03_01": {
                            "original_id": "OSPS-AC-03.01"
                        }
                    }
                }
            }
        });

        let synced =
            sync_osps_baseline_criteria(&catalog, &upstream).expect("sync should find control");
        assert_eq!(synced.len(), 1);
        assert_eq!(synced[0].key, "OSPS-AC-03.01");
        assert_eq!(synced[0].level, "baseline-1");
        assert_eq!(synced[0].category, "osps_access_control");
        assert_eq!(synced[0].evidence_type, "repo_file");
        assert_eq!(synced[0].safe_default_decision, "accept");
    }

    #[test]
    fn detects_stale_references() {
        assert!(contains_old_ref(
            "see Anionix/old-repo",
            &["old-repo".into()]
        ));
        assert!(!contains_old_ref(
            "see Anionix/new-repo",
            &["old-repo".into()]
        ));
    }

    #[test]
    fn validates_fixture_catalog() {
        let catalog = read_catalog(Path::new(DEFAULT_CATALOG)).expect("catalog should parse");
        validate_catalog(&catalog).expect("catalog should validate");
    }

    #[test]
    fn annotates_known_criterion_false_positive() {
        let raw = "criterion_key: vulnerabilities_fixed_60_days\n";
        assert!(annotate_scanner_false_positives(raw.into()).contains("gitleaks:allow"));
    }

    #[test]
    fn replaces_old_refs_without_malformed_absolute_urls() {
        let current =
            "https://github.com/Anionix/agent-ready-repo-template-spec/blob/main/README.md";
        let old_refs = vec![
            "agent-ready-repo-template-spec".into(),
            "Anionix/agent-ready-repo-template-spec".into(),
            "https://github.com/Anionix/agent-ready-repo-template-spec".into(),
            "agent-ready-repo-template".into(),
            "agent-ready-repoーtemplate".into(),
        ];
        let replaced =
            current.replace_old_refs(&old_refs, "https://github.com/Anionix/agent-ready-template");
        assert_eq!(
            replaced,
            "https://github.com/Anionix/agent-ready-template/blob/main/README.md"
        );
    }

    #[test]
    fn builds_signed_release_override_evidence() {
        let justification =
            signed_release_justification("v0.1.2", "https://github.com/Anionix/example/");
        assert!(justification.contains("Release artifacts for v0.1.2 and later"));
        assert!(justification.contains("https://github.com/Anionix/example/releases/tag/v0.1.2"));
        assert!(!justification.contains("v0.1.1"));
    }

    fn test_sheet(rows: Vec<Row>) -> Checksheet {
        Checksheet {
            schema_version: 1,
            project: Project {
                project_id: "42".into(),
                target_repo: "https://github.com/example/project".into(),
                target_homepage: "https://github.com/example/project".into(),
                expected_badge_level: "silver".into(),
                old_references: Vec::new(),
                notes: String::new(),
            },
            source: None,
            choice_sets: ChoiceSets {
                track: Vec::new(),
                level: Vec::new(),
                category: Vec::new(),
                status: Vec::new(),
                evidence_type: Vec::new(),
                boolean_choice: Vec::new(),
                save_state: Vec::new(),
                review_decision: Vec::new(),
                autofill_confidence: Vec::new(),
            },
            columns: Vec::new(),
            workflow_steps: Vec::new(),
            final_readback_rows: Vec::new(),
            rows,
        }
    }

    fn test_row(key: &str, target_status: &str, checked: &str, review_decision: &str) -> Row {
        Row {
            project_id: "42".into(),
            target_repo: "https://github.com/example/project".into(),
            track: if osps_keys::is_osps_key(key) {
                "osps".into()
            } else {
                "metal".into()
            },
            level: if osps_keys::is_osps_key(key) {
                "baseline-1".into()
            } else {
                "passing".into()
            },
            category: if osps_keys::is_osps_key(key) {
                "osps_access_control".into()
            } else {
                "identity".into()
            },
            criterion_key: key.into(),
            current_status: "?".into(),
            target_status: target_status.into(),
            current_justification: String::new(),
            target_justification: "Test justification".into(),
            evidence_type: "repo_file".into(),
            evidence_url: String::new(),
            stale_reference: "no".into(),
            manual_proof_required: "no".into(),
            save_state: "drafted".into(),
            review_decision: review_decision.into(),
            save_url: String::new(),
            owner: "maintainer".into(),
            checked: checked.into(),
            notes: String::new(),
            save_step: Some(1),
            autofill_rule_id: None,
            autofill_confidence: None,
            autofill_reason: None,
        }
    }
}
