import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import CodeBlock from '@theme/CodeBlock';
import Heading from '@theme/Heading';
import Example from '!!raw-loader!@site/../examples/grayscale.rs';

import styles from './index.module.css';

const pillars = [
  {
    title: 'Kernels puros',
    text: 'Cada algoritmo é uma função sobre buffers, sem ECS nem I/O: testável, mensurável e otimizável isoladamente.',
  },
  {
    title: 'Pipeline ECS',
    text: 'Frames são entidades, transformações são sistemas. Stages ordenados e plugins compõem o fluxo.',
  },
  {
    title: 'Memória segura',
    text: 'unsafe proibido fora de módulos isolados e auditados; Miri e fuzzing fazem parte do plano de CI.',
  },
  {
    title: 'Otimização com prova',
    text: 'Toda versão rápida de um kernel é comparada com a referência escalar por testes de propriedade.',
  },
];

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout description={siteConfig.tagline}>
      <header className={styles.hero}>
        <Heading as="h1" className={styles.title}>
          {siteConfig.title}
        </Heading>
        <p className={styles.tagline}>{siteConfig.tagline}</p>
        <div className={styles.actions}>
          <Link className="button button--primary button--lg" to="/docs/primeiros-passos/instalacao">
            Primeiros passos
          </Link>
          <Link className="button button--secondary button--lg" to="/docs/conceitos/arquitetura">
            Arquitetura
          </Link>
        </div>
      </header>
      <main>
        <section className={styles.section}>
          <div className="container">
            <div className={styles.grid}>
              {pillars.map(({title, text}) => (
                <div key={title} className={styles.card}>
                  <Heading as="h3">{title}</Heading>
                  <p>{text}</p>
                </div>
              ))}
            </div>
          </div>
        </section>
        <section className={styles.section}>
          <div className={`container ${styles.quickstart}`}>
            <Heading as="h2">Um pipeline completo</Heading>
            <CodeBlock language="rust" title="examples/grayscale.rs">
              {Example}
            </CodeBlock>
            <p className={styles.status}>
              Projeto em desenvolvimento inicial (0.1). O que já existe e o que vem a seguir está no{' '}
              <Link to="/docs/projeto/roadmap">roadmap</Link>.
            </p>
          </div>
        </section>
      </main>
    </Layout>
  );
}
