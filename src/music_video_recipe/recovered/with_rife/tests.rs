use super::*;
#[test]
fn selected_phase_order_keeps_interpolation_after_complete_recovery_and_before_assembly() {
    assert_eq!(phases(false),[WithRifePhase::CompleteCarrier,WithRifePhase::Assembly]);
    assert_eq!(phases(true),[WithRifePhase::CompleteCarrier,WithRifePhase::Interpolation,WithRifePhase::AssemblyCarrier,WithRifePhase::Assembly]);
}
#[test]
fn omitted_interpolation_never_observes_a_rife_tool_or_model() {assert!(extra_components(None,&CancellationToken::default()).unwrap().is_empty());}
#[test]
fn missing_work_still_preserves_original_ordinals_and_excludes_selected_reuse() {
    assert_eq!(batches(&[2,5601,99999],2).unwrap(),vec![vec![2,5601],vec![99999]]);assert!(batches(&[3,3],2).is_err());
}
#[test]
fn empty_or_legacy_root_cannot_be_adopted_as_a_v2_completion() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();std::fs::write(root.join("recovered-intent.json"),b"{}").unwrap();
    assert!(status(&root,&CancellationToken::default()).is_err());assert!(completion(&root,&CancellationToken::default()).is_err());assert_eq!(std::fs::read_dir(&root).unwrap().count(),1);assert!(!root.join("recovered-rife-intent.json").exists());
}
#[test]
fn premature_interpolation_or_assembly_root_does_not_supply_predecessor_authority() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();std::fs::create_dir(root.join("children")).unwrap();std::fs::create_dir(child(&root,"interpolate")).unwrap();
    assert!(runtime::native(&root,"interpolate".into(),WithRifePhase::Interpolation,vec![],None).is_err());assert!(!root.join("rife-assembly.json").exists());
}
