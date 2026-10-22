//! Bounded syntax and semantic checks; no filesystem access and no authorization output.

use super::model::*;
use super::ReadError;
use radishlex_ime_product_install::{
    ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity, ProgramComponent,
};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[path = "qualification_reader_assertions.rs"]
mod assertions;
use assertions::{assertion_supported, validate_normal_programs};

pub(super) fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ReadError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ReadError::ResourceLimit);
    }
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) || std::str::from_utf8(bytes).is_err() {
        return Err(ReadError::Malformed);
    }
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue { depth: 0 }
        .deserialize(&mut parser)
        .map_err(|error| {
            if error.to_string().contains("evidence depth limit") {
                ReadError::ResourceLimit
            } else {
                ReadError::Malformed
            }
        })?;
    parser.end().map_err(|_| ReadError::Malformed)?;
    let manifest: Manifest = serde_json::from_value(value).map_err(|_| ReadError::Malformed)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

// A pre-pass retains raw object keys until uniqueness has been checked. Parsing into
// Value directly would silently discard duplicate keys, including inside core types.
struct StrictValue {
    depth: usize,
}
impl<'de> DeserializeSeed<'de> for StrictValue {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for StrictValue {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("strict bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Value, E> {
        Err(E::custom("non-integer number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.into()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_DEPTH {
            return Err(de::Error::custom("evidence depth limit"));
        }
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(StrictValue {
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_DEPTH {
            return Err(de::Error::custom("evidence depth limit"));
        }
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate evidence key"));
            }
            let value = map.next_value_seed(StrictValue {
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

pub(super) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn require_digest(value: &str) -> Result<(), ReadError> {
    if valid_digest(value) {
        Ok(())
    } else {
        Err(ReadError::Malformed)
    }
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn require_id(value: &str) -> Result<(), ReadError> {
    if valid_id(value) {
        Ok(())
    } else {
        Err(ReadError::Malformed)
    }
}
// ASCII is intentionally conservative: without a Unicode normalization dependency,
// accepting non-ASCII would leave aliases on normalization-insensitive macOS volumes.
pub(super) fn valid_relative_path(path: &str) -> bool {
    super::filesystem::valid_relative_path(path)
}
fn paths_unique<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), ReadError> {
    let mut spellings = BTreeMap::new();
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    for path in paths {
        if path.len() > 180 {
            return Err(ReadError::ResourceLimit);
        }
        if !valid_relative_path(path) {
            return Err(ReadError::Malformed);
        }
        let mut prefix = String::new();
        let components: Vec<_> = path.split('/').collect();
        for (index, component) in components.iter().enumerate() {
            if index > 0 {
                prefix.push('/');
            }
            prefix.push_str(component);
            let folded = prefix.to_ascii_lowercase();
            if let Some(old) = spellings.insert(folded, prefix.clone()) {
                if old != prefix {
                    return Err(ReadError::Malformed);
                }
            }
            if index + 1 == components.len() {
                if directories.contains(&prefix) || !files.insert(prefix.clone()) {
                    return Err(ReadError::Malformed);
                }
            } else {
                if files.contains(&prefix) {
                    return Err(ReadError::Malformed);
                }
                directories.insert(prefix.clone());
            }
        }
    }
    Ok(())
}
fn reference<'a>(
    files: &'a BTreeMap<&str, &FileEntry>,
    path: &str,
    role: FileRole,
) -> Result<&'a FileEntry, ReadError> {
    let file = files.get(path).ok_or(ReadError::MissingEvidence)?;
    if file.role != role {
        return Err(ReadError::Malformed);
    }
    Ok(file)
}
fn state_valid(state: &LogicalState) -> Result<(), ReadError> {
    if state.schema < 0 {
        return Err(ReadError::SchemaUnsupported);
    }
    for digest in [
        &state.words_sha256,
        &state.tombstones_sha256,
        &state.settings_sha256,
        &state.offline_projection_sha256,
        &state.wal_sha256,
    ] {
        require_digest(digest)?;
    }
    Ok(())
}
fn captured_valid(
    file: &CapturedFile,
    files: &BTreeMap<&str, &FileEntry>,
) -> Result<(), ReadError> {
    if !valid_relative_path(&file.path) || file.inode == 0 {
        return Err(ReadError::Malformed);
    }
    require_digest(&file.sha256)?;
    let material = reference(files, &file.evidence, FileRole::SyntheticData)?;
    if material.bytes != file.bytes || material.sha256 != file.sha256 {
        return Err(ReadError::IdentityMismatch);
    }
    Ok(())
}
fn source_valid(source: &ExactSource) -> Result<(), ReadError> {
    let original = &source.product;
    let release = ProductRelease::new(
        original.release().product_version(),
        original.release().build_number(),
    )
    .map_err(|_| ReadError::IdentityMismatch)?;
    let rebuild = |component| {
        let p = original.program(component);
        ProgramBundleIdentity::new(
            component,
            p.bundle_id(),
            p.bundle_tree_sha256(),
            p.code_identity_sha256(),
        )
        .map_err(|_| ReadError::IdentityMismatch)
    };
    let reconstructed = ProductArtifactIdentity::new(
        original.product_id(),
        release,
        original.product_manifest_sha256(),
        rebuild(ProgramComponent::Manager)?,
        rebuild(ProgramComponent::InputMethod)?,
    )
    .map_err(|_| ReadError::IdentityMismatch)?;
    if reconstructed != *original || source.programs.len() != 2 {
        return Err(ReadError::IdentityMismatch);
    }
    let mut components = BTreeSet::new();
    for program in &source.programs {
        if !components.insert(program.component)
            || program.signatures.is_empty()
            || program.resources.is_empty()
        {
            return Err(ReadError::IdentityMismatch);
        }
        let content = std::iter::once(&program.executable)
            .chain(program.dylibs.iter())
            .chain(program.resources.iter());
        paths_unique(content.clone().map(|item| item.path.as_str()))?;
        for item in content {
            require_digest(&item.sha256)?;
        }
        let mut architectures = BTreeSet::new();
        for signature in &program.signatures {
            if !architectures.insert(signature.architecture) {
                return Err(ReadError::Malformed);
            }
            require_id(&signature.signing_identifier)?;
            match (signature.signature_kind, &signature.team_identifier) {
                (SignatureKind::AdHoc, None) => {}
                (SignatureKind::DeveloperId, Some(team)) => require_id(team)?,
                _ => return Err(ReadError::Malformed),
            }
            for digest in [
                &signature.code_directory_sha256,
                &signature.designated_requirement_sha256,
                &signature.entitlements_sha256,
            ] {
                require_digest(digest)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_manifest(manifest: &Manifest) -> Result<(), ReadError> {
    if manifest.format != FORMAT {
        return Err(ReadError::Malformed);
    }
    if manifest.files.len() > MAX_FILES
        || manifest.rows.len() > MAX_ROWS
        || manifest.samples.len() > MAX_ROWS
    {
        return Err(ReadError::ResourceLimit);
    }
    require_id(&manifest.run_id)?;
    require_id(&manifest.profile)?;
    source_valid(&manifest.source)?;
    paths_unique(
        std::iter::once("manifest.json")
            .chain(manifest.files.iter().map(|file| file.path.as_str())),
    )?;
    let mut total = 0u64;
    let mut files = BTreeMap::new();
    for file in &manifest.files {
        if file.bytes > MAX_FILE_BYTES {
            return Err(ReadError::ResourceLimit);
        }
        total = total
            .checked_add(file.bytes)
            .ok_or(ReadError::ResourceLimit)?;
        if total > MAX_TOTAL_BYTES {
            return Err(ReadError::ResourceLimit);
        }
        require_digest(&file.sha256)?;
        files.insert(file.path.as_str(), file);
    }
    let environment = &manifest.environment;
    require_id(&environment.os_build)?;
    require_id(&environment.filesystem)?;
    match environment.execution_mode {
        ExecutionMode::Native
            if environment.host_architecture != environment.process_architecture =>
        {
            return Err(ReadError::EnvironmentUnsupported)
        }
        ExecutionMode::Translated
            if environment.host_architecture != Architecture::Arm64
                || environment.process_architecture != Architecture::X86_64 =>
        {
            return Err(ReadError::EnvironmentUnsupported)
        }
        _ => {}
    }
    for program in &manifest.source.programs {
        if !program
            .signatures
            .iter()
            .any(|s| s.architecture == environment.process_architecture)
        {
            return Err(ReadError::EnvironmentUnsupported);
        }
    }
    reference(
        &files,
        &environment.isolation_record,
        FileRole::EnvironmentAdmission,
    )?;
    reference(
        &files,
        &environment.admission_record,
        FileRole::EnvironmentAdmission,
    )?;
    let schema = &manifest.schema;
    if schema.source.version < 0 {
        return Err(ReadError::SchemaUnsupported);
    }
    require_digest(&schema.source.semantic_sha256)?;
    require_digest(&schema.source_manifest_sha256)?;
    require_digest(&schema.recipe_sha256)?;
    if schema.source_manifest_sha256 != manifest.source.product.product_manifest_sha256() {
        return Err(ReadError::IdentityMismatch);
    }
    if !files
        .values()
        .any(|f| f.role == FileRole::SyntheticRecipe && f.sha256 == schema.recipe_sha256)
    {
        return Err(ReadError::MissingEvidence);
    }
    reference(&files, &schema.output, FileRole::SchemaMeasurement)?;
    reference(
        &files,
        &schema.independent_observation,
        FileRole::StructuredObservation,
    )?;
    let collector = &manifest.collector;
    if ![40, 64].contains(&collector.source_commit.len())
        || !collector
            .source_commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ReadError::Malformed);
    }
    require_digest(&collector.executable_sha256)?;
    require_digest(&collector.command_list_sha256)?;
    require_id(&collector.protocol)?;
    if reference(&files, &collector.command_list, FileRole::CollectorCommands)?.sha256
        != collector.command_list_sha256
    {
        return Err(ReadError::IdentityMismatch);
    }
    let mut samples = BTreeMap::new();
    for sample in &manifest.samples {
        require_id(&sample.sample_id)?;
        if sample.run_id != manifest.run_id
            || samples.insert(sample.sample_id.as_str(), sample).is_some()
        {
            return Err(ReadError::ContradictoryRun);
        }
        state_valid(&sample.initial_state)?;
        if sample.initial_state.schema != schema.source.version {
            return Err(ReadError::SchemaUnsupported);
        }
        if reference(&files, &sample.recipe, FileRole::SyntheticRecipe)?.sha256
            != sample.recipe_sha256
        {
            return Err(ReadError::IdentityMismatch);
        }
        if sample.initial_files.is_empty() {
            return Err(ReadError::MissingEvidence);
        }
        paths_unique(sample.initial_files.iter().map(|file| file.path.as_str()))?;
        for file in &sample.initial_files {
            captured_valid(file, &files)?;
        }
    }
    let mut attempts = BTreeMap::new();
    let mut case_attempts = BTreeMap::<&str, &str>::new();
    for row in &manifest.rows {
        require_id(&row.case_id)?;
        require_id(&row.attempt_id)?;
        if row.run_id != manifest.run_id
            || !samples.contains_key(row.sample_id.as_str())
            || row.attempt == 0
            || attempts.contains_key(row.attempt_id.as_str())
        {
            return Err(ReadError::ContradictoryRun);
        }
        if case_attempts.get(row.case_id.as_str()).copied() != row.retry_of.as_deref() {
            return Err(ReadError::ContradictoryRun);
        }
        if let Some(prior_id) = &row.retry_of {
            let prior: &&CoverageRow = attempts
                .get(prior_id.as_str())
                .ok_or(ReadError::ContradictoryRun)?;
            if prior.case_id != row.case_id
                || prior.sample_id != row.sample_id
                || prior.attempt.checked_add(1) != Some(row.attempt)
            {
                return Err(ReadError::ContradictoryRun);
            }
        } else if row.attempt != 1 {
            return Err(ReadError::ContradictoryRun);
        }
        case_attempts.insert(row.case_id.as_str(), row.attempt_id.as_str());
        attempts.insert(row.attempt_id.as_str(), row);
        validate_row(row, manifest, &files)?;
    }
    Ok(())
}

fn validate_row(
    row: &CoverageRow,
    manifest: &Manifest,
    files: &BTreeMap<&str, &FileEntry>,
) -> Result<(), ReadError> {
    if row.expected != row.actual {
        return Err(ReadError::AssertionFailed);
    }
    if row.raw_evidence.is_empty()
        || row.independent_evidence.is_empty()
        || row.preconditions.is_empty()
        || row.assertions.is_empty()
        || row.observations.is_empty()
    {
        return Err(ReadError::MissingEvidence);
    }
    for path in &row.raw_evidence {
        reference(files, path, FileRole::RawObservation)?;
    }
    for path in &row.independent_evidence {
        reference(files, path, FileRole::StructuredObservation)?;
    }
    let mut sequences = BTreeSet::new();
    let mut last_sequence = 0;
    let mut processes = BTreeMap::<&str, &ProcessInstance>::new();
    let mut exited = BTreeSet::new();
    for observation in &row.observations {
        if observation.run_id != row.run_id
            || observation.sample_id != row.sample_id
            || observation.sequence <= last_sequence
        {
            return Err(ReadError::ContradictoryRun);
        }
        last_sequence = observation.sequence;
        sequences.insert(observation.sequence);
        reference(
            files,
            &observation.evidence,
            FileRole::StructuredObservation,
        )?;
        if !row.independent_evidence.contains(&observation.evidence) {
            return Err(ReadError::MissingEvidence);
        }
        if let Some(process) = &observation.process {
            require_id(&process.start_id)?;
            if process.pid == 0 || process.start_sequence == 0 {
                return Err(ReadError::ContradictoryRun);
            }
            let source = manifest
                .source
                .programs
                .iter()
                .find(|p| p.component == process.component)
                .ok_or(ReadError::IdentityMismatch)?;
            if process.executable_sha256 != source.executable.sha256 {
                return Err(ReadError::IdentityMismatch);
            }
            if matches!(observation.event, Event::Started { .. }) {
                if process.start_sequence != observation.sequence
                    || processes.insert(&process.start_id, process).is_some()
                {
                    return Err(ReadError::ContradictoryRun);
                }
            } else if processes.get(process.start_id.as_str()).copied() != Some(process)
                || exited.contains(process.start_id.as_str())
            {
                return Err(ReadError::ContradictoryRun);
            }
            if matches!(observation.event, Event::Exited { .. }) {
                exited.insert(process.start_id.as_str());
            }
        } else if matches!(
            observation.event,
            Event::Started { .. }
                | Event::Gate { .. }
                | Event::RuntimeOpened { .. }
                | Event::ClientInput { .. }
                | Event::Selection { .. }
                | Event::Learned { .. }
                | Event::Exited { .. }
                | Event::Reopened { .. }
        ) {
            return Err(ReadError::ContradictoryRun);
        }
        match &observation.event {
            Event::Started { launch_path } => {
                let process = observation
                    .process
                    .as_ref()
                    .ok_or(ReadError::ContradictoryRun)?;
                let source = manifest
                    .source
                    .programs
                    .iter()
                    .find(|p| p.component == process.component)
                    .ok_or(ReadError::IdentityMismatch)?;
                if launch_path != &source.executable.path {
                    return Err(ReadError::IdentityMismatch);
                }
            }
            Event::RuntimeOpened { database } => captured_valid(database, files)?,
            Event::ClientInput { input_sha256 } => require_digest(input_sha256)?,
            Event::Selection { selection_sha256 } => require_digest(selection_sha256)?,
            Event::Learned { state } | Event::Reopened { state } => state_valid(state)?,
            Event::Storage {
                before,
                after,
                state,
            } => {
                if before.is_empty() || after.is_empty() {
                    return Err(ReadError::MissingEvidence);
                }
                for file in before.iter().chain(after) {
                    captured_valid(file, files)?;
                }
                paths_unique(before.iter().map(|f| f.path.as_str()))?;
                paths_unique(after.iter().map(|f| f.path.as_str()))?;
                state_valid(state)?;
            }
            Event::ReceiptState {
                history_sha256,
                protection_snapshots_sha256,
                ..
            } => {
                require_digest(history_sha256)?;
                require_digest(protection_snapshots_sha256)?;
            }
            Event::ReaderResult { fault, outcome }
                if *fault != row.scenario.rejection_fault || *outcome != row.actual =>
            {
                return Err(ReadError::AssertionFailed);
            }
            _ => {}
        }
    }
    let sample = manifest
        .samples
        .iter()
        .find(|s| s.sample_id == row.sample_id)
        .ok_or(ReadError::ContradictoryRun)?;
    if !matches!(&row.observations[0].event, Event::Storage { before, state, .. }
        if before == &sample.initial_files && state == &sample.initial_state)
    {
        return Err(ReadError::ContradictoryRun);
    }
    for assertions in [&row.preconditions, &row.assertions] {
        let mut kinds = BTreeSet::new();
        for assertion in assertions {
            if !kinds.insert(assertion.kind) || assertion.expected != assertion.actual {
                return Err(ReadError::AssertionFailed);
            }
            match &assertion.actual {
                AssertionValue::Digest(digest) => require_digest(digest)?,
                AssertionValue::State(state) => state_valid(state)?,
                _ => {}
            }
            if assertion.observation_sequences.is_empty() || assertion.evidence.is_empty() {
                return Err(ReadError::MissingEvidence);
            }
            if assertion
                .observation_sequences
                .iter()
                .any(|sequence| !sequences.contains(sequence))
            {
                return Err(ReadError::ContradictoryRun);
            }
            if row
                .observations
                .iter()
                .filter(|o| assertion.observation_sequences.contains(&o.sequence))
                .any(|o| !assertion.evidence.contains(&o.evidence))
            {
                return Err(ReadError::MissingEvidence);
            }
            if !assertion_supported(assertion, row, manifest) {
                return Err(ReadError::AssertionFailed);
            }
            for path in &assertion.evidence {
                reference(files, path, FileRole::StructuredObservation)?;
                if !row.independent_evidence.contains(path) {
                    return Err(ReadError::MissingEvidence);
                }
            }
        }
    }
    if row.group == CoverageGroup::NormalPrograms {
        if row.actual != Outcome::Allowed
            || row.end_state != EndState::LearnedAndReopened
            || row.observations.iter().any(|o| {
                matches!(
                    o.event,
                    Event::Gate {
                        outcome: Outcome::Rejected,
                        ..
                    }
                )
            })
        {
            return Err(ReadError::AssertionFailed);
        }
        validate_normal_programs(row)?;
    }
    if row.group == CoverageGroup::ReaderRejection
        && row.scenario.rejection_fault == RejectionFault::None
    {
        return Err(ReadError::AssertionFailed);
    }
    if row.group == CoverageGroup::BlockingState
        && (row.scenario.blocking_object == BlockingObject::None
            || row.actual != Outcome::Rejected
            || row.end_state != EndState::BlockedWithoutWrite
            || row
                .observations
                .iter()
                .any(|o| matches!(o.event, Event::RuntimeOpened { .. } | Event::Learned { .. }))
            || !row.observations.iter().any(|o| {
                matches!(
                    o.event,
                    Event::Gate {
                        outcome: Outcome::Rejected,
                        ..
                    }
                )
            }))
    {
        return Err(ReadError::AssertionFailed);
    }
    Ok(())
}

pub(super) fn validate_coverage(
    manifest: &Manifest,
    required: &[CaseDefinition],
) -> Result<Vec<String>, ReadError> {
    if required.is_empty() || required.len() > MAX_ROWS {
        return Err(ReadError::PolicyUnavailable);
    }
    let mut cases = BTreeMap::new();
    for case in required {
        for assertions in [&case.preconditions, &case.assertions] {
            let mut kinds = BTreeSet::new();
            if assertions.iter().any(|a| !kinds.insert(a.kind)) {
                return Err(ReadError::PolicyUnavailable);
            }
        }
        if !valid_id(&case.case_id)
            || case.preconditions.is_empty()
            || case.assertions.is_empty()
            || cases.insert(case.case_id.as_str(), case).is_some()
        {
            return Err(ReadError::PolicyUnavailable);
        }
    }
    let mut covered = BTreeSet::new();
    for row in &manifest.rows {
        let case = cases
            .get(row.case_id.as_str())
            .ok_or(ReadError::CoverageMissing)?;
        if row.group != case.group
            || row.scenario != case.scenario
            || row.expected != case.expected
            || row.end_state != case.end_state
        {
            return Err(ReadError::AssertionFailed);
        }
        for (actual, expected) in [
            (&row.preconditions, &case.preconditions),
            (&row.assertions, &case.assertions),
        ] {
            if actual.len() != expected.len() {
                return Err(ReadError::CoverageMissing);
            }
            for requirement in expected {
                if !actual.iter().any(|a| {
                    a.kind == requirement.kind
                        && a.expected == requirement.expected
                        && a.actual == requirement.expected
                }) {
                    return Err(ReadError::AssertionFailed);
                }
            }
        }
        covered.insert(row.case_id.as_str());
    }
    Ok(required
        .iter()
        .filter(|case| !covered.contains(case.case_id.as_str()))
        .map(|case| case.case_id.clone())
        .collect())
}
