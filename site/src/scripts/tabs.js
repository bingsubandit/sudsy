// Tabs (`[data-tabs]` with `[role=tab]` buttons and `[role=tabpanel]` panels) and copy buttons (`[data-copy]`),
// shared by the install tabs and the terminal sessions. Without this script the first panel shows and the
// tab lists stay hidden (see `.js-only` in landing.css).
for (const root of document.querySelectorAll('[data-tabs]')) {
  const tabs = [...root.querySelectorAll('[role=tab]')];
  const select = (tab, focus) => {
    for (const t of tabs) {
      const on = t === tab;
      t.setAttribute('aria-selected', String(on));
      t.tabIndex = on ? 0 : -1;
      document.getElementById(t.getAttribute('aria-controls')).hidden = !on;
    }
    if (focus) tab.focus();
  };
  tabs.forEach((tab, i) => {
    tab.addEventListener('click', () => select(tab));
    tab.addEventListener('keydown', (e) => {
      const to = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: tabs.length - 1 }[e.key];
      if (to === undefined) return;
      e.preventDefault();
      select(tabs[(to + tabs.length) % tabs.length], true);
    });
  });
}

for (const button of document.querySelectorAll('[data-copy]')) {
  const label = button.textContent;
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      button.textContent = 'Copied';
    } catch {
      button.textContent = 'Press Ctrl+C';
    }
    setTimeout(() => (button.textContent = label), 1600);
  });
}
