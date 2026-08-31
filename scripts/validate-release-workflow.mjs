#!/usr/bin/env node

import { readFileSync } from "node:fs";

const workflowPath = ".github/workflows/release-norn-macos.yml";
const workflow = readFileSync(workflowPath, "utf8");
const lifecycleWorkflow = readFileSync(".github/workflows/homebrew-lifecycle-smoke.yml", "utf8");
const formulaLifecycleScript = readFileSync("scripts/homebrew-formula-lifecycle.sh", "utf8");
const caskLifecycleScript = readFileSync("scripts/homebrew-cask-lifecycle.sh", "utf8");
const appVerificationScript = readFileSync("scripts/verify-macos-app.sh", "utf8");
const toolingTestRunner = readFileSync("scripts/run-tooling-tests.mjs", "utf8");
const githubRefNameExpression = "$" + "{GITHUB_REF_NAME}";

const checks = [
  [
    workflow.includes("group: norn-macos-release") &&
      workflow.includes("cancel-in-progress: false"),
    "release and tap publication runs are serialized without cancelling an active release",
  ],
  [
    workflow.includes("dtolnay/rust-toolchain@stable"),
    "release jobs install an explicit Rust toolchain",
  ],
  [
    workflow.includes('RUST_TOOLCHAIN: "1.94.0"') &&
      workflow.match(/toolchain: \$\{\{ env\.RUST_TOOLCHAIN \}\}/g)?.length === 3,
    "verification and artifact jobs use the same pinned Rust toolchain",
  ],
  [
    workflow.includes("components: rustfmt, clippy"),
    "the verification toolchain includes rustfmt and Clippy",
  ],
  [
    workflow.includes("cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check"),
    "the release gate checks Rust formatting",
  ],
  [workflow.includes("pnpm run lint"), "the release gate runs the frontend lint gate"],
  [
    workflow.includes(
      "cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings",
    ),
    "the release gate denies Clippy warnings across every target and feature",
  ],
  [
    workflow.includes(
      "cargo test --locked --manifest-path src-tauri/Cargo.toml --test cli_entrypoint --no-default-features",
    ),
    "the release gate separately verifies zero-argument command behavior without desktop features",
  ],
  [
    workflow.includes(
      "cargo test --locked --manifest-path src-tauri/Cargo.toml --bin norn --all-features",
    ),
    "the release gate verifies desktop routing without launching GUI integration tests",
  ],
  [
    workflow.includes(
      "cargo test --locked --manifest-path src-tauri/Cargo.toml --all-targets --no-default-features --features custom-protocol",
    ),
    "the release gate tests the exact command release feature set",
  ],
  [
    workflow.includes("target: x86_64-apple-darwin") &&
      workflow.includes("target: aarch64-apple-darwin"),
    "both supported macOS target triples are explicit",
  ],
  [
    workflow.includes(
      'cargo build --locked --manifest-path src-tauri/Cargo.toml --release --target "$TARGET" --no-default-features --features custom-protocol --bin norn --bin norn-tui',
    ),
    "release archives explicitly disable desktop routing and use the production protocol",
  ],
  [
    workflow.includes("src-tauri/target/$TARGET/release"),
    "archives read binaries from the target-specific release directory",
  ],
  [
    /"target": "\$\{TARGET\}"/.test(workflow) &&
      /"toolchain": "\$\{rustc_version\}"/.test(workflow),
    "release metadata records the target and Rust toolchain",
  ],
  [workflow.includes("if-no-files-found: error"), "missing release artifacts fail the upload step"],
  [
    workflow.includes("overwrite_files: false"),
    "published assets remain immutable on workflow reruns",
  ],
  [
    workflow.includes("pnpm run test:tooling") &&
      toolingTestRunner.includes('"scripts/render-homebrew-formula.test.mjs"') &&
      workflow.includes("node scripts/render-homebrew-formula.mjs"),
    "formula rendering is tested and runs against release artifacts",
  ],
  [
    workflow.includes("node scripts/validate-homebrew-manifests.mjs --formula") &&
      workflow.includes("ruby -c dist/homebrew/norn.rb"),
    "the rendered formula passes structural and Ruby syntax validation",
  ],
  [
    workflow.includes("name: norn-homebrew-formula") && workflow.includes("dist/homebrew/norn.rb"),
    "the verified formula is retained as a release and workflow artifact",
  ],
  [
    toolingTestRunner.includes('"scripts/render-homebrew-cask.test.mjs"') &&
      workflow.includes("node scripts/render-homebrew-cask.mjs") &&
      workflow.includes("name: norn-homebrew-cask") &&
      workflow.includes("dist/homebrew/norn-cask.rb"),
    "the architecture-specific cask is rendered, tested, and retained",
  ],
  [
    workflow.includes("build-macos-desktop:") &&
      workflow.includes(
        'pnpm tauri build --ci --target "$TARGET" --bundles dmg --features custom-protocol,desktop-bundle',
      ) &&
      workflow.includes("APPLE_SIGNING_IDENTITY") &&
      workflow.includes("APPLE_CERTIFICATE") &&
      workflow.includes("APPLE_ID") &&
      workflow.includes("APPLE_PASSWORD") &&
      workflow.includes("APPLE_TEAM_ID"),
    "desktop builds require Apple signing and notarization credentials and explicit desktop routing",
  ],
  [
    workflow.includes('codesign --verify --deep --strict "$app_path"') ||
      (workflow.includes('bash scripts/verify-macos-app.sh "$app_path"') &&
        appVerificationScript.includes("codesign --verify --deep --strict")),
    "desktop app bundles pass strict code-signature verification",
  ],
  [
    workflow.includes(
      'spctl --assess --type open --context context:primary-signature "$dmg_path"',
    ) &&
      appVerificationScript.includes('spctl --assess --type execute "$app_path"') &&
      appVerificationScript.includes('xcrun stapler validate "$app_path"'),
    "desktop artifacts pass Gatekeeper and stapled notarization verification",
  ],
  [
    /homebrew-formula-smoke:[\s\S]*needs: release/.test(workflow) &&
      workflow.includes("runner: macos-15-intel") &&
      workflow.includes("runner: macos-15") &&
      workflow.includes("bash scripts/homebrew-formula-lifecycle.sh") &&
      workflow.includes(`releases/download/${githubRefNameExpression}/norn.rb`),
    "both Homebrew smoke runners install and test the formula from its public release URL",
  ],
  [
    /homebrew-cask-smoke:[\s\S]*needs: release/.test(workflow) &&
      workflow.includes("bash scripts/homebrew-cask-lifecycle.sh") &&
      workflow.includes("NORN_RELEASE_CHANNEL: desktop") &&
      workflow.includes('curl -fsSL "$release_url/norn-cask.rb"'),
    "both desktop smoke runners use the public cask and test a true prior-release upgrade",
  ],
  [
    workflow.includes("homebrew-formula-smoke:") &&
      workflow.includes("secrets.HOMEBREW_TAP_TOKEN") &&
      workflow.includes("repository: delaudio/homebrew-tap") &&
      workflow.includes("public-tap/Formula/norn.rb") &&
      workflow.includes("public-tap/Casks/norn.rb"),
    "the public tap advances only after smoke tests with a secret-scoped credential",
  ],
  [
    toolingTestRunner.includes('"scripts/resolve-previous-homebrew-release.test.mjs"') &&
      workflow.includes("node scripts/resolve-previous-homebrew-release.mjs"),
    "the upgrade gate resolves and tests the previous complete stable release",
  ],
  [
    workflow.includes("vars.NORN_HOMEBREW_BOOTSTRAP_TAG") &&
      workflow.includes("steps.previous.outputs.bootstrap != 'true'") &&
      lifecycleWorkflow.includes("vars.NORN_HOMEBREW_BOOTSTRAP_TAG") &&
      formulaLifecycleScript.includes("NORN_HOMEBREW_BOOTSTRAP:-false") &&
      caskLifecycleScript.includes("NORN_HOMEBREW_BOOTSTRAP:-false"),
    "only an explicitly tagged first governed release may use the fail-closed bootstrap path",
  ],
  [
    workflow.includes("bash scripts/homebrew-formula-lifecycle.sh") &&
      workflow.includes("bash scripts/homebrew-cask-lifecycle.sh") &&
      !workflow.includes("|| true"),
    "command and desktop clean install, upgrade, uninstall, and reinstall gates fail closed",
  ],
  [
    workflow.includes("prerelease: true") &&
      /finalize-stable-release:[\s\S]*needs:[\s\S]*homebrew-formula-smoke[\s\S]*homebrew-cask-smoke/.test(
        workflow,
      ) &&
      workflow.includes("--prerelease=false --latest"),
    "the candidate becomes a stable release only after both lifecycle smoke jobs",
  ],
  [
    /publish-homebrew-tap:[\s\S]*needs: finalize-stable-release/.test(workflow),
    "tap publication follows stable-release promotion",
  ],
  [
    lifecycleWorkflow.includes("Require complete stable release assets") &&
      lifecycleWorkflow.includes("steps.release.outputs.arm64_url") &&
      lifecycleWorkflow.includes("steps.release.outputs.x86_64_url") &&
      lifecycleWorkflow.includes("steps.release.outputs.formula_url") &&
      lifecycleWorkflow.includes("steps.release.outputs.desktop_arm64_url") &&
      lifecycleWorkflow.includes("steps.release.outputs.desktop_x86_64_url") &&
      lifecycleWorkflow.includes("steps.release.outputs.cask_url") &&
      !lifecycleWorkflow.includes("skip=true"),
    "scheduled lifecycle validation fails when any required release asset is missing",
  ],
  [
    lifecycleWorkflow.includes("bash scripts/homebrew-formula-lifecycle.sh") &&
      lifecycleWorkflow.includes("bash scripts/homebrew-cask-lifecycle.sh") &&
      !lifecycleWorkflow.includes("|| true") &&
      toolingTestRunner.includes('"scripts/run-with-timeout.test.mjs"') &&
      formulaLifecycleScript.includes("node scripts/run-with-timeout.mjs --timeout-ms 60000") &&
      !formulaLifecycleScript.includes("perl -e") &&
      caskLifecycleScript.includes("bash scripts/verify-macos-app.sh") &&
      caskLifecycleScript.includes("security add-generic-password") &&
      caskLifecycleScript.includes("security find-generic-password"),
    "scheduled lifecycle validation uses the same fail-closed upgrade path",
  ],
];

const failures = checks.filter(([passed]) => !passed);
if (failures.length > 0) {
  for (const [, description] of failures) {
    console.error(`FAIL: ${description}`);
  }
  process.exit(1);
}

console.log("Release workflow contract passes:");
for (const [, description] of checks) {
  console.log(`- ${description}`);
}
