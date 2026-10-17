use super::model::*;
use super::validation::{parse_manifest, validate_coverage, validate_manifest};
use super::ReadError;
use radishlex_ime_product_install::{
    ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity, ProgramComponent,
    INSTALL_PRODUCT_ID,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn synthetic_state() -> LogicalState {
    LogicalState {
        schema: 9,
        words_sha256: digest(b"synthetic words"),
        tombstones_sha256: digest(b"synthetic tombstones"),
        settings_sha256: digest(b"synthetic settings"),
        offline_projection_sha256: digest(b"synthetic offline projection"),
        wal_sha256: digest(b"synthetic wal"),
    }
}
fn captured() -> CapturedFile {
    CapturedFile {
        path: "db/synthetic.sqlite".into(),
        evidence: "db/synthetic.sqlite".into(),
        device: 10,
        inode: 20,
        bytes: b"synthetic database".len() as u64,
        sha256: digest(b"synthetic database"),
    }
}
fn assertion(kind: AssertionKind, value: AssertionValue, sequences: Vec<u64>) -> Assertion {
    Assertion {
        kind,
        expected: value.clone(),
        actual: value,
        observation_sequences: sequences,
        evidence: vec!["observations.json".into()],
    }
}
fn observation(sequence: u64, event: Event, process: Option<ProcessInstance>) -> Observation {
    Observation {
        run_id: "synthetic-run".into(),
        sample_id: "sample-1".into(),
        sequence,
        process,
        event,
        evidence: "observations.json".into(),
    }
}

/// Entirely synthetic private test fixture, never a production catalog record.
pub(super) fn complete_fixture() -> (Manifest, Vec<CaseDefinition>, BTreeMap<String, Vec<u8>>) {
    let attachments: BTreeMap<String, Vec<u8>> = [
        ("raw.log", "synthetic raw observations"),
        ("observations.json", "synthetic independent observations"),
        ("recipe.json", "synthetic recipe"),
        ("schema.json", "synthetic schema measurement"),
        ("admission.json", "synthetic isolated environment admission"),
        ("commands.json", "synthetic collector commands"),
        ("db/synthetic.sqlite", "synthetic database"),
    ]
    .into_iter()
    .map(|(name, bytes)| (name.into(), bytes.as_bytes().to_vec()))
    .collect();
    let files = attachments
        .iter()
        .map(|(path, bytes)| FileEntry {
            path: path.clone(),
            bytes: bytes.len() as u64,
            sha256: digest(bytes),
            role: match path.as_str() {
                "raw.log" => FileRole::RawObservation,
                "observations.json" => FileRole::StructuredObservation,
                "recipe.json" => FileRole::SyntheticRecipe,
                "schema.json" => FileRole::SchemaMeasurement,
                "admission.json" => FileRole::EnvironmentAdmission,
                "commands.json" => FileRole::CollectorCommands,
                "db/synthetic.sqlite" => FileRole::SyntheticData,
                _ => unreachable!(),
            },
        })
        .collect();
    let release = ProductRelease::new("0.1.0", 39).unwrap();
    let bundle = |component, name: &str| {
        ProgramBundleIdentity::new(
            component,
            format!("dev.radishlex.{name}"),
            digest(name.as_bytes()),
            digest(format!("signature-{name}").as_bytes()),
        )
        .unwrap()
    };
    let product = ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        release,
        digest(b"synthetic product manifest"),
        bundle(ProgramComponent::Manager, "manager"),
        bundle(ProgramComponent::InputMethod, "inputmethod"),
    )
    .unwrap();
    let programs: Vec<_> = [ProgramComponent::Manager, ProgramComponent::InputMethod]
        .into_iter()
        .map(|component| ProgramIdentity {
            component,
            executable: ContentIdentity {
                path: "Contents/MacOS/synthetic".into(),
                sha256: digest(b"synthetic executable"),
            },
            dylibs: vec![ContentIdentity {
                path: "Contents/Frameworks/synthetic.dylib".into(),
                sha256: digest(b"synthetic dylib"),
            }],
            resources: vec![ContentIdentity {
                path: "Contents/Resources/synthetic.dat".into(),
                sha256: digest(b"synthetic resource"),
            }],
            signatures: vec![ArchitectureSignature {
                architecture: Architecture::Arm64,
                code_directory_sha256: digest(b"synthetic cd"),
                designated_requirement_sha256: digest(b"synthetic requirement"),
                entitlements_sha256: digest(b"synthetic entitlements"),
                signing_identifier: "dev.radishlex.synthetic".into(),
                signature_kind: SignatureKind::AdHoc,
                team_identifier: None,
            }],
        })
        .collect();
    let initial = synthetic_state();
    let scenario = Scenario {
        maintenance: Maintenance::Before,
        outer_state: OuterState::Completed,
        data_predecessor: DataPredecessor::Absent,
        lifecycle: Lifecycle::NoIndex,
        family: SourceFamily::Original,
        repeated_cancellation: false,
        blocking_object: BlockingObject::None,
        rejection_fault: RejectionFault::None,
    };
    let mut rows = Vec::new();
    for (index, group) in [
        CoverageGroup::CompatiblePostcondition,
        CoverageGroup::HistoricalCombination,
        CoverageGroup::NormalPrograms,
        CoverageGroup::DataSemantics,
        CoverageGroup::BlockingState,
        CoverageGroup::ReaderRejection,
        CoverageGroup::SubsequentRecovery,
    ]
    .into_iter()
    .enumerate()
    {
        let mut row = CoverageRow {
            case_id: format!("synthetic-case-{index}"),
            group,
            scenario: scenario.clone(),
            run_id: "synthetic-run".into(),
            sample_id: "sample-1".into(),
            attempt: 1,
            retry_of: None,
            attempt_id: format!("synthetic-attempt-{index}"),
            preconditions: vec![assertion(
                AssertionKind::InitialState,
                AssertionValue::State(initial.clone()),
                vec![1],
            )],
            expected: Outcome::Allowed,
            actual: Outcome::Allowed,
            end_state: EndState::Preserved,
            observations: vec![observation(
                1,
                Event::Storage {
                    before: vec![captured()],
                    after: vec![captured()],
                    state: initial.clone(),
                },
                None,
            )],
            assertions: Vec::new(),
            raw_evidence: vec!["raw.log".into()],
            independent_evidence: vec!["observations.json".into()],
        };
        match group {
            CoverageGroup::CompatiblePostcondition => {
                row.observations.push(observation(
                    2,
                    Event::ReceiptState {
                        new_data_receipts: 0,
                        outer: OuterState::Completed,
                        cancellation_count: 1,
                        history_sha256: digest(b"history"),
                        protection_snapshots_sha256: digest(b"snapshots"),
                    },
                    None,
                ));
                row.assertions.push(assertion(
                    AssertionKind::NoNewDataReceipt,
                    AssertionValue::Count(0),
                    vec![2],
                ));
            }
            CoverageGroup::HistoricalCombination => {
                row.observations.push(observation(
                    2,
                    Event::ReceiptState {
                        new_data_receipts: 0,
                        outer: OuterState::Completed,
                        cancellation_count: 1,
                        history_sha256: digest(b"history"),
                        protection_snapshots_sha256: digest(b"snapshots"),
                    },
                    None,
                ));
                row.assertions.push(assertion(
                    AssertionKind::HistoryPreserved,
                    AssertionValue::Digest(digest(b"history")),
                    vec![2],
                ));
            }
            CoverageGroup::NormalPrograms => {
                let mut sequence = 2;
                let mut learned_state = initial.clone();
                learned_state.words_sha256 = digest(b"synthetic learned word");
                for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
                    for launch in 0..2 {
                        let process = ProcessInstance {
                            component,
                            pid: 100 + launch,
                            start_id: format!("{component:?}-{launch}"),
                            start_sequence: sequence,
                            executable_sha256: digest(b"synthetic executable"),
                        };
                        let mut events = vec![
                            Event::Started {
                                launch_path: "Contents/MacOS/synthetic".into(),
                            },
                            Event::Gate {
                                gate: Gate::OuterInstall,
                                outcome: Outcome::Allowed,
                            },
                            Event::Gate {
                                gate: Gate::DataUpgrade,
                                outcome: Outcome::Allowed,
                            },
                            Event::RuntimeOpened {
                                database: captured(),
                            },
                        ];
                        if launch == 0 {
                            events.extend([
                                Event::ClientInput {
                                    input_sha256: digest(b"synthetic input"),
                                },
                                Event::Selection {
                                    selection_sha256: digest(b"synthetic selection"),
                                },
                                Event::Learned {
                                    state: learned_state.clone(),
                                },
                            ]);
                        } else {
                            events.push(Event::Reopened {
                                state: learned_state.clone(),
                            });
                        }
                        events.push(Event::Exited { exit_code: 0 });
                        for event in events {
                            row.observations.push(observation(
                                sequence,
                                event,
                                Some(process.clone()),
                            ));
                            sequence += 1;
                        }
                    }
                }
                let selected = row
                    .observations
                    .iter()
                    .filter(|o| matches!(o.event, Event::Learned { .. } | Event::Reopened { .. }))
                    .map(|o| o.sequence)
                    .collect();
                row.assertions.push(assertion(
                    AssertionKind::LearningPersisted,
                    AssertionValue::State(learned_state),
                    selected,
                ));
                row.end_state = EndState::LearnedAndReopened;
            }
            CoverageGroup::DataSemantics => {
                row.observations.push(observation(
                    2,
                    Event::Transaction {
                        checkpoint: TransactionCheckpoint::Postcondition,
                        state: EndState::Preserved,
                    },
                    None,
                ));
                row.observations.push(observation(
                    3,
                    Event::Storage {
                        before: vec![captured()],
                        after: vec![captured()],
                        state: initial.clone(),
                    },
                    None,
                ));
                row.assertions.push(assertion(
                    AssertionKind::WalPreserved,
                    AssertionValue::Digest(initial.wal_sha256.clone()),
                    vec![3],
                ));
            }
            CoverageGroup::BlockingState => {
                row.scenario.blocking_object = BlockingObject::PreparationMarker;
                row.expected = Outcome::Rejected;
                row.actual = Outcome::Rejected;
                row.end_state = EndState::BlockedWithoutWrite;
                let process = ProcessInstance {
                    component: ProgramComponent::Manager,
                    pid: 100,
                    start_id: "blocked-process".into(),
                    start_sequence: 2,
                    executable_sha256: digest(b"synthetic executable"),
                };
                row.observations.push(observation(
                    2,
                    Event::Started {
                        launch_path: "Contents/MacOS/synthetic".into(),
                    },
                    Some(process.clone()),
                ));
                row.observations.push(observation(
                    3,
                    Event::Gate {
                        gate: Gate::OuterInstall,
                        outcome: Outcome::Rejected,
                    },
                    Some(process),
                ));
                row.assertions.push(assertion(
                    AssertionKind::GateDeniedBeforeWrite,
                    AssertionValue::Outcome(Outcome::Rejected),
                    vec![3],
                ));
            }
            CoverageGroup::ReaderRejection => {
                row.scenario.rejection_fault = RejectionFault::WrongProof;
                row.expected = Outcome::Rejected;
                row.actual = Outcome::Rejected;
                row.observations.push(observation(
                    2,
                    Event::ReaderResult {
                        fault: RejectionFault::WrongProof,
                        outcome: Outcome::Rejected,
                    },
                    None,
                ));
                row.assertions.push(assertion(
                    AssertionKind::ReaderRejected,
                    AssertionValue::Outcome(Outcome::Rejected),
                    vec![2],
                ));
            }
            CoverageGroup::SubsequentRecovery => {
                row.end_state = EndState::DataTerminalReleased;
                row.observations.push(observation(
                    2,
                    Event::Transaction {
                        checkpoint: TransactionCheckpoint::DataTerminalRelease,
                        state: EndState::DataTerminalReleased,
                    },
                    None,
                ));
                row.assertions.push(assertion(
                    AssertionKind::DataTerminalRelease,
                    AssertionValue::EndState(EndState::DataTerminalReleased),
                    vec![2],
                ));
            }
        }
        rows.push(row);
    }
    let requirements = rows
        .iter()
        .map(|row| CaseDefinition {
            case_id: row.case_id.clone(),
            group: row.group,
            scenario: row.scenario.clone(),
            expected: row.expected,
            end_state: row.end_state,
            preconditions: row
                .preconditions
                .iter()
                .map(|a| AssertionExpectation {
                    kind: a.kind,
                    expected: a.expected.clone(),
                })
                .collect(),
            assertions: row
                .assertions
                .iter()
                .map(|a| AssertionExpectation {
                    kind: a.kind,
                    expected: a.expected.clone(),
                })
                .collect(),
        })
        .collect();
    let manifest = Manifest {
        format: FORMAT.into(),
        run_id: "synthetic-run".into(),
        profile: "synthetic-full-profile".into(),
        schema: SchemaMeasurement {
            source: SourceSchema {
                version: 9,
                semantic_sha256: digest(b"synthetic schema"),
            },
            source_manifest_sha256: product.product_manifest_sha256().into(),
            recipe_sha256: digest(&attachments["recipe.json"]),
            method: SchemaMethod::IsolatedSourceSyntheticDatabase,
            output: "schema.json".into(),
            independent_observation: "observations.json".into(),
        },
        collector: Collector {
            source_commit: "a".repeat(40),
            executable_sha256: digest(b"synthetic collector"),
            protocol: "synthetic-protocol-v1".into(),
            command_list_sha256: digest(&attachments["commands.json"]),
            command_list: "commands.json".into(),
        },
        source: ExactSource { product, programs },
        environment: Environment {
            os_build: "synthetic-OS-build".into(),
            host_architecture: Architecture::Arm64,
            process_architecture: Architecture::Arm64,
            execution_mode: ExecutionMode::Native,
            filesystem: "APFS".into(),
            case_semantics: CaseSemantics::Insensitive,
            isolation: IsolationMethod::DedicatedGuest,
            isolation_record: "admission.json".into(),
            admission_record: "admission.json".into(),
        },
        samples: vec![Sample {
            sample_id: "sample-1".into(),
            run_id: "synthetic-run".into(),
            recipe: "recipe.json".into(),
            recipe_sha256: digest(&attachments["recipe.json"]),
            initial_state: initial,
            initial_files: vec![captured()],
        }],
        rows,
        files,
    };
    (manifest, requirements, attachments)
}

#[test]
fn synthetic_model_round_trip_and_exact_coverage() {
    let (manifest, requirements, _) = complete_fixture();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let parsed = parse_manifest(&bytes).unwrap();
    assert_eq!(parsed, manifest);
    assert!(validate_coverage(&parsed, &requirements)
        .unwrap()
        .is_empty());
}
#[test]
fn duplicate_keys_rejected_inside_core_identity_and_arbitrary_nested_objects() {
    let (manifest, _, _) = complete_fixture();
    let json = serde_json::to_string(&manifest).unwrap();
    let duplicated = json.replacen(
        "\"build_number\":39",
        "\"build_number\":39,\"build_number\":39",
        1,
    );
    assert_eq!(
        parse_manifest(duplicated.as_bytes()),
        Err(ReadError::Malformed)
    );
    let nested = json.replacen(
        "\"protocol\":",
        "\"extra\":{\"a\":1,\"a\":1},\"protocol\":",
        1,
    );
    assert_eq!(parse_manifest(nested.as_bytes()), Err(ReadError::Malformed));
    let escaped = json.replacen(
        "\"run_id\":",
        "\"run_id\":\"synthetic-run\",\"run_\\u0069d\":",
        1,
    );
    assert_eq!(
        parse_manifest(escaped.as_bytes()),
        Err(ReadError::Malformed)
    );
}
#[test]
fn unknown_fields_numeric_forms_and_bom_rejected() {
    let (manifest, _, _) = complete_fixture();
    let json = serde_json::to_string(&manifest).unwrap();
    for changed in [
        json.replacen(
            "\"build_number\":39",
            "\"build_number\":39,\"unknown\":0",
            1,
        ),
        json.replacen("\"build_number\":39", "\"build_number\":3.9e1", 1),
        json.replacen(
            "\"build_number\":39",
            "\"build_number\":18446744073709551616",
            1,
        ),
        format!("\u{feff}{json}"),
    ] {
        assert_eq!(
            parse_manifest(changed.as_bytes()),
            Err(ReadError::Malformed)
        );
    }
}
#[test]
fn depth_size_and_count_limits_fail_closed() {
    assert_eq!(
        parse_manifest(&vec![b' '; MAX_MANIFEST_BYTES + 1]),
        Err(ReadError::ResourceLimit)
    );
    let json = format!(
        "{}0{}",
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    assert_eq!(
        parse_manifest(json.as_bytes()),
        Err(ReadError::ResourceLimit)
    );
    let (mut manifest, _, _) = complete_fixture();
    manifest.files[0].bytes = MAX_FILE_BYTES;
    assert!(validate_manifest(&manifest).is_ok());
    manifest.files[0].bytes += 1;
    assert_eq!(validate_manifest(&manifest), Err(ReadError::ResourceLimit));
    manifest.files = vec![manifest.files[0].clone(); MAX_FILES + 1];
    assert_eq!(validate_manifest(&manifest), Err(ReadError::ResourceLimit));
}
#[test]
fn parent_case_aliases_non_ascii_and_traversal_are_rejected() {
    for path in [
        "../outside",
        "/absolute",
        "a//b",
        "a/./b",
        "a/../b",
        "é/file",
        "a\\b",
        "a/%62",
        ".hidden/file",
    ] {
        assert!(!super::validation::valid_relative_path(path));
    }
    let (mut manifest, _, _) = complete_fixture();
    let mut extra = manifest.files[0].clone();
    extra.path = "Dir/one".into();
    manifest.files.push(extra.clone());
    extra.path = "dir/two".into();
    manifest.files.push(extra);
    assert_eq!(validate_manifest(&manifest), Err(ReadError::Malformed));
}
#[test]
fn exact_missing_case_not_tag_presence_and_skip_is_unknown() {
    let (mut manifest, requirements, _) = complete_fixture();
    let missing = manifest.rows.remove(0).case_id;
    assert_eq!(
        validate_coverage(&manifest, &requirements).unwrap(),
        vec![missing]
    );
    manifest.rows[0].scenario.data_predecessor = DataPredecessor::DistinctOperation;
    assert_eq!(
        validate_coverage(&manifest, &requirements),
        Err(ReadError::AssertionFailed)
    );
    let json = serde_json::to_string(&manifest).unwrap().replacen(
        "historical_combination",
        "not_applicable",
        1,
    );
    assert_eq!(parse_manifest(json.as_bytes()), Err(ReadError::Malformed));
}
#[test]
fn altered_result_mixed_sample_process_restart_and_unclosed_refs_rejected() {
    let (manifest, _, _) = complete_fixture();
    let mut changed = manifest.clone();
    changed.rows[0].actual = Outcome::Rejected;
    assert_eq!(validate_manifest(&changed), Err(ReadError::AssertionFailed));
    let mut changed = manifest.clone();
    changed.rows[2].observations[3].sample_id = "sample-other".into();
    assert_eq!(
        validate_manifest(&changed),
        Err(ReadError::ContradictoryRun)
    );
    let mut changed = manifest.clone();
    changed.rows[2].observations[9]
        .process
        .as_mut()
        .unwrap()
        .start_id = "Manager-0".into();
    assert_eq!(
        validate_manifest(&changed),
        Err(ReadError::ContradictoryRun)
    );
    let mut changed = manifest.clone();
    changed.rows[0]
        .independent_evidence
        .push("absent.json".into());
    assert_eq!(validate_manifest(&changed), Err(ReadError::MissingEvidence));
    let mut changed = manifest;
    changed.rows[0].assertions[0].actual = AssertionValue::Count(1);
    assert_eq!(validate_manifest(&changed), Err(ReadError::AssertionFailed));
}
#[test]
fn negative_expected_rejection_passes_but_allow_and_write_do_not() {
    let (manifest, requirements, _) = complete_fixture();
    assert!(validate_coverage(&manifest, &requirements)
        .unwrap()
        .is_empty());
    let mut changed = manifest.clone();
    changed.rows[4].actual = Outcome::Allowed;
    assert_eq!(validate_manifest(&changed), Err(ReadError::AssertionFailed));
    let mut changed = manifest;
    let mut write = changed.rows[4].observations[2].clone();
    write.sequence = 4;
    write.event = Event::RuntimeOpened {
        database: captured(),
    };
    changed.rows[4].observations.push(write);
    assert_eq!(validate_manifest(&changed), Err(ReadError::AssertionFailed));
}
#[test]
fn source_schema_environment_and_policy_expectations_are_not_wildcards() {
    let (manifest, requirements, _) = complete_fixture();
    let mut changed = manifest.clone();
    changed.schema.source_manifest_sha256 = digest(b"other manifest");
    assert_eq!(
        validate_manifest(&changed),
        Err(ReadError::IdentityMismatch)
    );
    let mut changed = manifest.clone();
    changed.schema.source.version = 40;
    assert_eq!(
        validate_manifest(&changed),
        Err(ReadError::SchemaUnsupported)
    );
    let mut changed = manifest.clone();
    changed.environment.execution_mode = ExecutionMode::Translated;
    assert_eq!(
        validate_manifest(&changed),
        Err(ReadError::EnvironmentUnsupported)
    );
    let mut changed_requirements = requirements;
    changed_requirements[0].assertions[0].expected = AssertionValue::Count(3);
    assert_eq!(
        validate_coverage(&manifest, &changed_requirements),
        Err(ReadError::AssertionFailed)
    );
}

#[test]
fn row_and_total_byte_budgets_and_manifest_alias_are_bounded() {
    let (manifest, _, _) = complete_fixture();
    let mut changed = manifest.clone();
    changed.rows = vec![changed.rows[0].clone(); MAX_ROWS + 1];
    assert_eq!(validate_manifest(&changed), Err(ReadError::ResourceLimit));
    let mut changed = manifest.clone();
    let initial_total: u64 = changed.files.iter().map(|f| f.bytes).sum();
    for index in 0..16 {
        changed.files.push(FileEntry {
            path: format!("budget/file-{index}"),
            bytes: if index == 15 {
                MAX_FILE_BYTES - initial_total
            } else {
                MAX_FILE_BYTES
            },
            sha256: digest(b"synthetic budget"),
            role: FileRole::RawObservation,
        });
    }
    assert_eq!(
        changed.files.iter().map(|f| f.bytes).sum::<u64>(),
        MAX_TOTAL_BYTES
    );
    assert!(validate_manifest(&changed).is_ok());
    changed.files.last_mut().unwrap().bytes += 1;
    assert_eq!(validate_manifest(&changed), Err(ReadError::ResourceLimit));
    let mut changed = manifest;
    changed.files.push(FileEntry {
        path: "MANIFEST.JSON/child".into(),
        bytes: 1,
        sha256: digest(b"synthetic alias"),
        role: FileRole::RawObservation,
    });
    assert_eq!(validate_manifest(&changed), Err(ReadError::Malformed));
}

#[test]
fn signature_kind_explicitly_distinguishes_ad_hoc_from_developer_id() {
    let (manifest, _, _) = complete_fixture();
    assert!(manifest.source.programs.iter().all(|p| p
        .signatures
        .iter()
        .all(|s| s.signature_kind == SignatureKind::AdHoc && s.team_identifier.is_none())));
    assert!(parse_manifest(&serde_json::to_vec(&manifest).unwrap()).is_ok());
    let mut changed = manifest.clone();
    changed.source.programs[0].signatures[0].team_identifier = Some("SYNTHETIC".into());
    assert_eq!(validate_manifest(&changed), Err(ReadError::Malformed));
    let mut changed = manifest.clone();
    changed.source.programs[0].signatures[0].signature_kind = SignatureKind::DeveloperId;
    assert_eq!(validate_manifest(&changed), Err(ReadError::Malformed));
    changed.source.programs[0].signatures[0].team_identifier = Some("SYNTHETIC1".into());
    assert!(parse_manifest(&serde_json::to_vec(&changed).unwrap()).is_ok());
    changed.source.programs[0].signatures[0].team_identifier = Some(String::new());
    assert_eq!(validate_manifest(&changed), Err(ReadError::Malformed));
    changed.source.programs[0].signatures[0].team_identifier = Some("invalid team".into());
    assert_eq!(validate_manifest(&changed), Err(ReadError::Malformed));
}

fn preservation_value(kind: AssertionKind, state: &LogicalState) -> AssertionValue {
    match kind {
        AssertionKind::MaintenanceEquivalent => AssertionValue::State(state.clone()),
        AssertionKind::WalPreserved => AssertionValue::Digest(state.wal_sha256.clone()),
        AssertionKind::TombstonePreserved => {
            AssertionValue::Digest(state.tombstones_sha256.clone())
        }
        AssertionKind::SettingsPreserved => AssertionValue::Digest(state.settings_sha256.clone()),
        AssertionKind::OfflineProjectionPreserved => {
            AssertionValue::Digest(state.offline_projection_sha256.clone())
        }
        _ => unreachable!(),
    }
}
fn drift_preserved_state(kind: AssertionKind, state: &mut LogicalState) {
    let field = match kind {
        AssertionKind::MaintenanceEquivalent => &mut state.words_sha256,
        AssertionKind::WalPreserved => &mut state.wal_sha256,
        AssertionKind::TombstonePreserved => &mut state.tombstones_sha256,
        AssertionKind::SettingsPreserved => &mut state.settings_sha256,
        AssertionKind::OfflineProjectionPreserved => &mut state.offline_projection_sha256,
        _ => unreachable!(),
    };
    *field = digest(b"synthetic postcondition drift");
}
#[test]
fn preserved_semantics_require_final_storage_after_the_postcondition() {
    for kind in [
        AssertionKind::MaintenanceEquivalent,
        AssertionKind::WalPreserved,
        AssertionKind::TombstonePreserved,
        AssertionKind::SettingsPreserved,
        AssertionKind::OfflineProjectionPreserved,
    ] {
        let (mut manifest, _, _) = complete_fixture();
        let initial = manifest.samples[0].initial_state.clone();
        manifest.rows[3].assertions =
            vec![assertion(kind, preservation_value(kind, &initial), vec![3])];
        assert!(validate_manifest(&manifest).is_ok(), "{kind:?}");
        let mut changed = manifest.clone();
        changed.rows[3].assertions[0].observation_sequences = vec![1];
        assert_eq!(
            validate_manifest(&changed),
            Err(ReadError::AssertionFailed),
            "initial-only {kind:?}"
        );
        let mut changed = manifest.clone();
        changed.rows[3].observations.remove(1);
        assert_eq!(
            validate_manifest(&changed),
            Err(ReadError::AssertionFailed),
            "no postcondition {kind:?}"
        );
        let mut changed = manifest.clone();
        if let Event::Storage { state, .. } = &mut changed.rows[3].observations[2].event {
            drift_preserved_state(kind, state);
        }
        assert_eq!(
            validate_manifest(&changed),
            Err(ReadError::AssertionFailed),
            "final drift {kind:?}"
        );
        let mut changed = manifest.clone();
        let mut drift = changed.rows[3].observations[2].clone();
        drift.sequence = 4;
        if let Event::Storage { state, .. } = &mut drift.event {
            drift_preserved_state(kind, state);
        }
        changed.rows[3].observations.push(drift);
        assert_eq!(
            validate_manifest(&changed),
            Err(ReadError::AssertionFailed),
            "uncited later drift {kind:?}"
        );
        let mut restored = changed.rows[3].observations[2].clone();
        restored.sequence = 5;
        changed.rows[3].observations.push(restored);
        changed.rows[3].assertions[0].observation_sequences = vec![5];
        assert_eq!(
            validate_manifest(&changed),
            Err(ReadError::AssertionFailed),
            "restored after contradiction {kind:?}"
        );
        if kind != AssertionKind::MaintenanceEquivalent {
            let mut changed = manifest;
            if let Event::Storage { state, .. } = &mut changed.rows[3].observations[2].event {
                state.words_sha256 = digest(b"legitimate unrelated word learning");
            }
            assert!(
                validate_manifest(&changed).is_ok(),
                "unrelated semantics {kind:?}"
            );
        }
    }
}
#[test]
fn final_storage_cannot_precede_a_later_state_observation() {
    let (mut manifest, _, _) = complete_fixture();
    let initial = manifest.samples[0].initial_state.clone();
    let process = ProcessInstance {
        component: ProgramComponent::Manager,
        pid: 99,
        start_id: "later-observer".into(),
        start_sequence: 4,
        executable_sha256: digest(b"synthetic executable"),
    };
    manifest.rows[3].observations.push(observation(
        4,
        Event::Started {
            launch_path: "Contents/MacOS/synthetic".into(),
        },
        Some(process.clone()),
    ));
    manifest.rows[3].observations.push(observation(
        5,
        Event::Learned { state: initial },
        Some(process),
    ));
    assert_eq!(
        validate_manifest(&manifest),
        Err(ReadError::AssertionFailed)
    );
}
#[test]
fn normal_reopen_matches_the_same_component_and_has_no_hidden_second_state() {
    let (manifest, _, _) = complete_fixture();
    let mut different_component_states = manifest.clone();
    for observed in &mut different_component_states.rows[2].observations {
        if observed
            .process
            .as_ref()
            .is_some_and(|p| p.component == ProgramComponent::InputMethod)
        {
            match &mut observed.event {
                Event::Learned { state } | Event::Reopened { state } => {
                    state.words_sha256 = digest(b"different InputMethod learned words")
                }
                _ => {}
            }
        }
    }
    assert!(validate_manifest(&different_component_states).is_ok());
    let mut wrong_pair = different_component_states.clone();
    for observed in &mut wrong_pair.rows[2].observations {
        if observed
            .process
            .as_ref()
            .is_some_and(|p| p.component == ProgramComponent::InputMethod)
        {
            if let Event::Reopened { state } = &mut observed.event {
                state.words_sha256 = digest(b"synthetic learned word");
            }
        }
    }
    assert_eq!(
        validate_manifest(&wrong_pair),
        Err(ReadError::AssertionFailed)
    );
    // A later contradictory Reopened in the same live process cannot be hidden
    // behind the earlier green Reopened selected by the assertion.
    let mut duplicate = manifest;
    let row = &mut duplicate.rows[2];
    let index = row
        .observations
        .iter()
        .position(|o| matches!(o.event, Event::Reopened { .. }))
        .unwrap();
    let mut extra = row.observations[index].clone();
    extra.sequence += 1;
    if let Event::Reopened { state } = &mut extra.event {
        state.words_sha256 = digest(b"hidden changed reopened words");
    }
    for later in &mut row.observations[index + 1..] {
        later.sequence += 1;
        if let Some(process) = &mut later.process {
            if process.start_sequence >= extra.sequence {
                process.start_sequence += 1;
            }
        }
    }
    row.observations.insert(index + 1, extra);
    for a in row.preconditions.iter_mut().chain(&mut row.assertions) {
        for sequence in &mut a.observation_sequences {
            if *sequence > row.observations[index].sequence {
                *sequence += 1;
            }
        }
    }
    assert_eq!(
        validate_manifest(&duplicate),
        Err(ReadError::ContradictoryRun)
    );
}
