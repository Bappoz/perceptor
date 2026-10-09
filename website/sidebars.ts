import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'intro',
    {
      type: 'category',
      label: 'Primeiros passos',
      collapsed: false,
      items: ['primeiros-passos/instalacao', 'primeiros-passos/primeiro-pipeline'],
    },
    {
      type: 'category',
      label: 'Conceitos',
      collapsed: false,
      items: [
        'conceitos/arquitetura',
        'conceitos/modelo-de-imagem',
        'conceitos/pipeline-e-stages',
        'conceitos/plugins',
      ],
    },
    {
      type: 'category',
      label: 'Guias',
      items: ['guias/escrevendo-um-plugin'],
    },
    {
      type: 'category',
      label: 'Referência',
      link: {type: 'doc', id: 'referencia/index'},
      items: ['referencia/perceptor-core', 'referencia/perceptor-imgproc', 'referencia/perceptor-pipeline'],
    },
    'performance/index',
    {
      type: 'category',
      label: 'Projeto',
      items: ['projeto/roadmap', 'projeto/changelog'],
    },
  ],
};

export default sidebars;
