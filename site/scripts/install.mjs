// The install channels, written once: the install tabs of the landing page and of Getting started
// (src/components/InstallTabs.astro) and the Markdown twin of Getting started (agents.mjs) read them.
// The smoke test checks each command against the README's install table.
const releases = 'https://github.com/bingsubandit/sudsy/releases';

export const channels = [
  { label: 'npm', command: 'npm i -D @sudsylint/sudsy', note: 'Run it with <code>npx sudsy</code>.', text: 'Run it with `npx sudsy`.' },
  { label: 'PyPI', command: 'pip install sudsy', note: 'Or run it without installing: <code>uvx sudsy</code>.', text: 'Or run it without installing: `uvx sudsy`.' },
  { label: 'Homebrew', command: 'brew install bingsubandit/sudsy/sudsy' },
  { label: 'Scoop', command: 'scoop bucket add sudsy https://github.com/bingsubandit/scoop-sudsy; scoop install sudsy' },
  {
    label: 'Binary',
    note: `Download the archive for your platform (Linux, macOS or Windows, x64 or arm64) from the <a href="${releases}">GitHub releases</a> and put <code>sudsy</code> on your <code>PATH</code>.`,
    text: `Download the archive for your platform (Linux, macOS or Windows, x64 or arm64) from the [GitHub releases](${releases}) and put \`sudsy\` on your \`PATH\`.`,
  },
];

// The Markdown form, for the twin of Getting started.
export const installMarkdown = () =>
  channels.map((c) => [`**${c.label}**`, c.command && '```sh\n' + c.command + '\n```', c.text].filter(Boolean).join('\n\n')).join('\n\n');
