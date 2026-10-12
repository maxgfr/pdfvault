// semantic-release: version from Conventional Commits, CLI built for every
// platform, binaries on the GitHub release, npm only when NPM_PUBLISH=true.
// No commit is pushed back: the version lives in the tag, the release and
// the published artifacts.
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
      },
    ],
    ["@semantic-release/npm", { pkgRoot: "packages/cli", npmPublish: publishToNpm }],
    [
      "@semantic-release/github",
      {
        assets: [
          { path: "packages/cli/dist/bin/pdfvault-*", label: "${path}" },
          { path: "packages/cli/dist/bin/SHA256SUMS", label: "SHA256SUMS" },
        ],
      },
    ],
  ],
};
