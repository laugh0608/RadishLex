use super::*;

fn terminal(state: InstallState) -> InstallReceipt {
    let mut value = receipt(InstallOperationKind::Upgrade);
    if state == InstallState::AbortedPreserved {
        value
            .abort_preserved(InstallFailureCode::SourceArtifactInvalid)
            .unwrap();
        return value;
    }
    value.advance(InstallState::Quiesced).unwrap();
    record_source_programs(&mut value);
    record_target_staging(&mut value);
    value.advance(InstallState::TargetStaged).unwrap();
    record_source_backups(&mut value);
    value.advance(InstallState::SourcePreserved).unwrap();
    record_installed_manager(&mut value);
    value.advance(InstallState::ManagerCommitted).unwrap();
    record_installed_input_method(&mut value);
    value.advance(InstallState::ProgramsCommitted).unwrap();
    value.advance(InstallState::DataCoordinating).unwrap();
    if state == InstallState::RolledBack {
        value
            .require_rollback(InstallFailureCode::DataCoordinationFailed)
            .unwrap();
        value.mark_programs_restored().unwrap();
        value.mark_rolled_back().unwrap();
    } else {
        value.advance(InstallState::DataSettled).unwrap();
        value.advance(InstallState::FinalVerified).unwrap();
        value.advance(InstallState::Completed).unwrap();
    }
    value
}

#[test]
fn exact_old_terminal_and_absent_or_artifact_free_new_outer_only() {
    const NEXT: &str = "ffeeddccbbaa99887766554433221100";
    for state in [
        InstallState::Completed,
        InstallState::AbortedPreserved,
        InstallState::RolledBack,
    ] {
        let old = terminal(state);
        let source = old.installed_product().unwrap();
        let target = product("0.2.0", 36, 2);
        let mut new = InstallReceipt::new(
            NEXT,
            Some(old.operation_id().into()),
            InstallOperationKind::Upgrade,
            root(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .unwrap();
        let old_bytes = old.encode().unwrap();
        let check = |previous: &[u8], new: Option<&[u8]>| {
            validate_preparation_cancellation_outer(
                previous,
                new,
                NEXT,
                old.operation_id(),
                &root(),
                source,
                &target,
            )
        };
        assert!(check(&old_bytes, None).is_ok());
        assert!(check(&old_bytes, Some(&new.encode().unwrap())).is_ok());
        assert!(!old.can_replace(&new));
        let mut noncanonical = old_bytes.clone();
        noncanonical.push(b' ');
        assert!(check(&noncanonical, None).is_err());
        assert!(validate_preparation_cancellation_outer(
            &old_bytes,
            None,
            NEXT,
            old.operation_id(),
            &InstallRootIdentity::new(10, 21, 501, 0o700).unwrap(),
            source,
            &target
        )
        .is_err());
        assert!(validate_preparation_cancellation_outer(
            &old_bytes,
            None,
            NEXT,
            old.operation_id(),
            &root(),
            &product("0.1.0", 35, 0),
            &target
        )
        .is_err());
        new.advance(InstallState::Quiesced).unwrap();
        assert!(check(&old_bytes, Some(&new.encode().unwrap())).is_err());
        record_source_programs(&mut new);
        assert!(check(&old_bytes, Some(&new.encode().unwrap())).is_err());
    }
    let nonterminal = receipt(InstallOperationKind::Upgrade);
    assert!(validate_preparation_cancellation_outer(
        &nonterminal.encode().unwrap(),
        None,
        NEXT,
        nonterminal.operation_id(),
        &root(),
        nonterminal.source_product().unwrap(),
        nonterminal.target_product().unwrap()
    )
    .is_err());
}
