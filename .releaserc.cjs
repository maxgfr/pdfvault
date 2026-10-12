// semantic-release: version from Conventional Commits, the npm bundle built
// and published when NPM_PUBLISH=true, the GitHub release created. The
// binaries are built per OS afterwards by the release workflow and attached
// to that release. No commit is pushed back: the version lives in the tag.
const publishToNpm = process.env.NPM_PUBLISH === "true";

module.exports = {
  branches: ["main"],
  plugins: [
    ["@semantic-release/commit-analyzer", { preset: "conventionalcommits" }],
    ["@semantic-release/release-notes-generator", { preset: "conventionalcommits" }],
    [
      "@semantic-release/exec",
      {
        prepareCmd: "bun scripts/prepare-release.ts ${nextRelease.version}",
        successCmd: 'echo "published=true" >> "$GITHUB_OUTPUT" && echo "version=${nextRelease.version}" >> "$GITHUB_OUTPUT"',
      },
    ],
    ["@semantic-release/npm", { pkgRoot: "packages/cli/dist", npmPublish: publishToNpm }],
    // A failed release shows in the Actions run; no tracking issue needed.
    ["@semantic-release/github", { failComment: false, failTitle: false, successComment: false }],
  ],
};
