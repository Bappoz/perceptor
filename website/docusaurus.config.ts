import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const repo = 'https://github.com/Bappoz/perceptor';

const config: Config = {
  title: 'Perceptor',
  tagline: 'Visão computacional em Rust: kernels puros, pipeline ECS.',
  favicon: 'img/logo.svg',

  future: {
    v4: true,
  },

  url: 'https://bappoz.github.io',
  baseUrl: '/perceptor/',
  organizationName: 'Bappoz',
  projectName: 'perceptor',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  // Conteúdo em PT-BR; o locale `en` entra quando houver tradução (#141).
  i18n: {
    defaultLocale: 'pt-BR',
    locales: ['pt-BR'],
  },

  markdown: {
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },
  themes: ['@docusaurus/theme-mermaid'],

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: `${repo}/tree/master/website/`,
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Perceptor',
      logo: {
        alt: 'Perceptor',
        src: 'img/logo.svg',
      },
      items: [
        {type: 'docSidebar', sidebarId: 'docs', position: 'left', label: 'Documentação'},
        {to: '/docs/referencia', label: 'Referência', position: 'left'},
        {to: '/docs/projeto/roadmap', label: 'Roadmap', position: 'left'},
        {href: 'https://bappoz.github.io/perceptor/api/perceptor/', label: 'API (rustdoc)', position: 'right'},
        {href: repo, label: 'GitHub', position: 'right'},
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentação',
          items: [
            {label: 'Introdução', to: '/docs/intro'},
            {label: 'Primeiros passos', to: '/docs/primeiros-passos/instalacao'},
            {label: 'Arquitetura', to: '/docs/conceitos/arquitetura'},
          ],
        },
        {
          title: 'Projeto',
          items: [
            {label: 'Roadmap', to: '/docs/projeto/roadmap'},
            {label: 'Changelog', to: '/docs/projeto/changelog'},
            {label: 'Issues', href: `${repo}/issues`},
          ],
        },
        {
          title: 'Código',
          items: [{label: 'GitHub', href: repo}],
        },
      ],
      copyright: `Perceptor — licença MIT.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'bash'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
