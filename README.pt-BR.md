# Undelete Master

O Undelete Master é um projeto de recuperação local, inicialmente para Windows,
com foco em preservar a origem e explicar a recuperabilidade com evidências.

> **Estado de desenvolvimento:** endurecimento da fundação. Este repositório não
> é uma versão de produção. Não o use como único meio para recuperar dados
> importantes.

[English](README.md)

## O que existe

- Abstração Rust `SourceReader` sem operação de escrita.
- Leitura somente para imagens regulares e regiões limitadas.
- Parsing MBR/GPT e suporte parcial a NTFS e FAT12/16/32.
- Imagens sintéticas determinísticas com comparação SHA-256.
- Demonstração React/Vite com dados explicitamente sintéticos.
- Incremento em andamento para CLI somente de imagens regulares, correções de
  limites, demonstração honesta, SDD e CI determinística.

O estado exato fica na
[matriz de rastreabilidade](docs/traceability-matrix.md). Trabalho em andamento
não significa `Verified`.

## O que não foi entregue

Não foram entregues acesso a disco físico, broker elevado, shell Tauri de
produção, restauração real, carving, worker isolado de preview/validação,
sessões persistentes, exFAT de produção, instaladores, assinatura ou release de
produção. Consulte as
[limitações conhecidas](docs/specs/015-known-limitations.md).

## Segurança

- Nunca use testes para escanear um disco real.
- Nunca adicione escrita, trim, formatação, lock, dismount ou comando genérico
  de dispositivo a uma fonte.
- Use apenas imagens determinísticas do repositório, memória, arquivos regulares
  temporários ou um futuro VHD de teste com governança e allowlist.
- Nunca execute conteúdo recuperado.

Consulte [SECURITY.md](SECURITY.md) e [CONTRIBUTING.md](CONTRIBUTING.md).

## Desenvolvimento

Qualidade Rust:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Frontend, dentro de `apps/desktop`:

```powershell
npm ci
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

`package-lock.json` é o lockfile canônico do frontend; o pnpm é usado aqui
apenas para invocar os scripts obrigatórios do pacote.

A CLI somente para imagens faz parte do incremento atual. Ela só deve ser
tratada como disponível/verificada quando a matriz apontar para evidência
aprovada do Task 8.

## Documentação

- [Visão do produto](docs/specs/000-product-vision.md)
- [Requisitos funcionais](docs/specs/001-functional-requirements.md)
- [Arquitetura](docs/specs/004-architecture.md)
- [Segurança e privacidade](docs/specs/011-security-and-privacy.md)
- [Plano de testes](docs/specs/012-test-and-validation-plan.md)
- [ADRs](docs/adr/README.md)
- [Registro de riscos](docs/risk-register.md)
- [Rastreabilidade](docs/traceability-matrix.md)

## Integração Git

O histórico remoto de `main` é autoritativo e deve ser preservado. Branches de
desenvolvimento são integradas sem force-push ou substituição de `main`. Esta
tarefa de documentação não publica, cria tag, assina nem move referências
remotas.
