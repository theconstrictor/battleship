module.exports = {
  branches: ['master'],
  plugins: [
    '@semantic-release/commit-analyzer',
    '@semantic-release/release-notes-generator',
    [
      '@semantic-release/exec',
      {
        prepareCmd:
          'sed -i -E \'s/^version = "[0-9]+\\.[0-9]+\\.[0-9]+"$/version = "${nextRelease.version}"/\' Cargo.toml && cargo metadata --format-version 1 > /dev/null',
        publishCmd: "printf '%s' '${nextRelease.version}' > release-version.txt",
      },
    ],
    [
      '@semantic-release/git',
      {
        assets: ['Cargo.toml', 'Cargo.lock'],
        message: 'chore(release): ${nextRelease.version} [skip ci]',
      },
    ],
    '@semantic-release/github',
  ],
};
