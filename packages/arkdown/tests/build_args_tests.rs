use arkdown_napi::cmd::build::BuildArgs;
use clap::Parser;

#[derive(Parser)]
struct BuildArgsHarness {
  #[command(flatten)]
  build: BuildArgs,
}

#[test]
fn enable_sign_task_accepts_explicit_false_for_unsigned_parity_builds() {
  let parsed =
    BuildArgsHarness::try_parse_from(["arkdown-build-args-test", "--enable-sign-task", "false"])
      .expect("explicit false must be accepted");

  assert!(!parsed.build.enable_sign_task);
}
