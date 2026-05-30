use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use html_escape::encode_text;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
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
        out: PathBuf,
    },
    Render {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
}

#[derive(Clone, ValueEnum)]
enum ProposalMode {
    Criterion,
    Step,
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

#[derive(Debug, Deserialize)]
struct Catalog {
    schema_version: u64,
    criteria: Vec<CatalogCriterion>,
}

#[derive(Debug, Deserialize, Clone)]
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
            template,
            catalog,
            output,
        }),
        Command::Proposals { input, mode, out } => proposals_command(&input, mode, &out),
        Command::Render { input, out_dir } => render_command(&input, &out_dir),
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
    template: PathBuf,
    catalog: PathBuf,
    output: PathBuf,
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
        .map(|key| key.trim_end_matches("_status").to_string())
        .collect();
    status_keys.sort_by_key(|key| catalog_sort_key(&catalog, key));

    for key in status_keys {
        let current_status =
            json_string(map.get(&format!("{key}_status"))).unwrap_or_else(|| "?".into());
        let current_justification =
            json_string(map.get(&format!("{key}_justification"))).unwrap_or_default();
        let criterion = catalog_by_key
            .get(&key)
            .cloned()
            .unwrap_or_else(|| fallback_criterion(&key));
        let stale = contains_old_ref(&current_justification, &args.old_refs);
        let decision = decision_for(&current_status, stale, &criterion);
        let save_state = save_state_for(stale, &criterion, &decision);
        let target_status = if criterion.manual_proof_required == "yes" {
            "?".to_string()
        } else {
            current_status.clone()
        };
        let target_justification = if stale {
            current_justification.replace_old_refs(&args.old_refs, &args.repo_url)
        } else {
            current_justification.clone()
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
            evidence_url: evidence_url_for(&key, map),
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

fn proposals_command(input: &Path, mode: ProposalMode, out: &Path) -> Result<()> {
    let sheet = read_checksheet(input)?;
    validate_checksheet(&sheet, None)?;
    let mut text = String::new();
    text.push_str("# OpenSSF Proposal URLs\n\n");
    text.push_str(
        "Generated by `ossf-badge-workbench`. Review each URL before saving in the OpenSSF UI.\n\n",
    );
    match mode {
        ProposalMode::Criterion => {
            for row in sheet.rows.iter().filter(|row| row.track != "readback") {
                let url = proposal_url(&sheet.project.project_id, row, std::slice::from_ref(row));
                text.push_str(&format!("## `{}`\n\n<{}>\n\n", row.criterion_key, url));
            }
        }
        ProposalMode::Step => {
            let mut groups: BTreeMap<u64, Vec<Row>> = BTreeMap::new();
            for row in sheet.rows.iter().filter(|row| row.track != "readback") {
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
        if !seen.insert(&criterion.key) {
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
                .map(|criterion| criterion.key.as_str())
                .collect()
        })
        .unwrap_or_default();

    for row in &sheet.rows {
        let row_id = format!("{}:{}", row.track, row.criterion_key);
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
            && !catalog_keys.contains(row.criterion_key.as_str())
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
        .map(|criterion| (criterion.key.clone(), criterion.clone()))
        .collect()
}

fn catalog_sort_key(catalog: &Catalog, key: &str) -> (usize, String) {
    let index = catalog
        .criteria
        .iter()
        .position(|criterion| criterion.key == key)
        .unwrap_or(usize::MAX);
    (index, key.to_string())
}

fn fallback_criterion(key: &str) -> CatalogCriterion {
    CatalogCriterion {
        key: key.into(),
        track: if key.starts_with("OSPS-") {
            "osps"
        } else {
            "metal"
        }
        .into(),
        level: if key.starts_with("OSPS-") {
            "baseline-3"
        } else {
            "silver"
        }
        .into(),
        category: if key.starts_with("OSPS-") {
            "osps_quality"
        } else {
            "identity"
        }
        .into(),
        evidence_type: "none".into(),
        manual_proof_required: "no".into(),
        safe_default_decision: "keep_unknown".into(),
        edit_path: if key.starts_with("OSPS-") {
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
        for old_ref in old_refs {
            value = value.replace(old_ref, new_ref);
        }
        value
    }
}

fn decision_for(status: &str, stale: bool, criterion: &CatalogCriterion) -> String {
    if criterion.manual_proof_required == "yes" {
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
    let justification = json_string(map.get(&format!("{key}_justification"))).unwrap_or_default();
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
        let param_key = criterion_param_key(&row.criterion_key);
        params.push((format!("{param_key}_status"), row.target_status.clone()));
        params.push((
            format!("{param_key}_justification"),
            row.target_justification.clone(),
        ));
        overrides.push(format!("{param_key}_status"));
        overrides.push(format!("{param_key}_justification"));
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

fn criterion_param_key(key: &str) -> String {
    if key.starts_with("OSPS-") {
        key.to_ascii_lowercase().replace(['-', '.'], "_")
    } else {
        key.to_string()
    }
}

fn encode_query(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
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

fn write_yaml(path: &Path, sheet: &Checksheet) -> Result<()> {
    let raw = annotate_scanner_false_positives(serde_yaml::to_string(sheet)?);
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
    raw.replace(
        "criterion_key: vulnerabilities_fixed_60_days\n",
        "criterion_key: vulnerabilities_fixed_60_days # gitleaks:allow\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_osps_keys_to_form_params() {
        assert_eq!(criterion_param_key("OSPS-AC-01.01"), "osps_ac_01_01");
        assert_eq!(criterion_param_key("repo_public"), "repo_public");
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
}
