use super::*;

#[test]
fn builds_script_launch_plans_without_shell_interpolation() {
    let plan = script_launch_plan(Path::new("/tmp/project"), PackageManager::Pnpm, "dev:web");

    assert_eq!(plan.program, "pnpm");
    assert_eq!(plan.args, ["run", "dev:web"]);
    assert_eq!(plan.directory, Path::new("/tmp/project"));
}

#[test]
fn yarn_script_plan_uses_native_yarn_syntax() {
    let plan = script_launch_plan(Path::new("/tmp/project"), PackageManager::Yarn, "preview");

    assert_eq!(plan.program, "yarn");
    assert_eq!(plan.args, ["preview"]);
}
