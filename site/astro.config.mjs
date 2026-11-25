import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { site, base } from './config.mjs';

// The sidebar never lists rules: Starlight renders it into every page, so it would grow with
// rules squared. The Rules index (search and filters) is the way in.

export default defineConfig({
  site,
  base,
  outDir: process.env.SITE_OUT ?? './dist',
  integrations: [
    starlight({
      title: 'Sudsy',
      logo: { src: '../assets/logo/soap_color.svg', alt: 'Sudsy' },
      favicon: '/favicon.svg',
      // Starlight adds the canonical link (from `site`), og:title, og:description and og:url, and the sitemap.
      head: [
        { tag: 'script', content: "document.documentElement.classList.add('js')" },
        { tag: 'meta', attrs: { property: 'og:image', content: `${site}${base}/og.png` } },
        { tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
        { tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
        { tag: 'meta', attrs: { name: 'twitter:image', content: `${site}${base}/og.png` } },
      ],
      // `js` on <html> shows what needs a script (tab lists, copy buttons); without it the first panel shows.
      customCss: ['./src/styles/site.css', './src/styles/landing.css'],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/bingsubandit/sudsy' }],
      sidebar: [
        { label: 'Getting started', link: '/getting-started/' },
        { label: 'Command line', link: '/cli/' },
        { label: 'Rules', link: '/rules/' },
        { label: 'Languages', link: '/languages/' },
        { label: 'How languages are measured', link: '/measured/' },
        { label: 'Speed comparisons', link: '/speed/' },
        { label: 'Compared tools', link: '/compare/' },
        {
          label: 'Reference',
          items: [
            { label: 'Presets', link: '/reference/presets/' },
            { label: 'Format options', link: '/reference/format-options/' },
          ],
        },
      ],
    }),
  ],
});
