# Undelete Master

O Undelete Master é um projeto local de recuperação de dados, inicialmente para
Windows, com foco em preservar a origem e apresentar resultados baseados em
evidências.

> **Estado de desenvolvimento:** a análise real de imagens está implementada;
> restauração de arquivos e acesso a disco físico não estão. Não use esta
> pré-versão como único meio para recuperar dados importantes.

[English](README.md)

## O que está implementado

- Abstração Rust `SourceReader` sem operação de escrita.
- Acesso somente leitura a arquivos comuns `.img`, `.dd`, `.raw` e `.bin`.
- Descoberta defensiva de MBR/GPT e análise parcial de metadados NTFS e
  FAT12/16/32.
- Aplicativo desktop Tauri 2 real, sem elevação, ligado diretamente ao scanner
  Rust.
- Seletor nativo controlado pelo Rust: o caminho escolhido nunca atravessa o
  IPC para o WebView.
- Relatório limitado com nome/tamanho real da origem, tipo de partição, volumes,
  contagens de candidatos de metadados e alertas do parser.
- CLI `undelete-master scan-image` com JSON sanitizado.
- Testes com fixtures sintéticas determinísticas, inclusive paridade do desktop
  e confirmação de que o SHA-256 da fixture não muda após a análise.
- Barreiras de CI contra operações destrutivas e contra a volta de
  comportamento fabricado no desktop.

O estado exato está na
[matriz de rastreabilidade](docs/traceability-matrix.md). Uma contagem de
candidatos de metadados não garante que o conteúdo possa ser recuperado.

## O que está deliberadamente ausente

O aplicativo não expõe discos físicos, handles de dispositivo, elevação,
restauração, prévia, sessões persistentes, carving, exFAT, percentuais fictícios,
pausa/retomada ou promessa de cancelamento. Esses recursos só voltarão à
interface depois de existir um backend real e testado. Consulte as
[limitações conhecidas](docs/specs/015-known-limitations.md).

## Segurança

- Nunca use testes para analisar um disco real.
- Nunca adicione escrita, trim, formatação, lock, dismount ou comando genérico
  de dispositivo ao caminho de análise.
- Use apenas imagens determinísticas do repositório, memória, arquivos regulares
  temporários ou um VHD de teste separado, governado e allowlisted.
- Nunca execute conteúdo recuperado.

Consulte [SECURITY.md](SECURITY.md) e [CONTRIBUTING.md](CONTRIBUTING.md).

## Desenvolvimento

Qualidade Rust:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Qualidade do desktop:

```powershell
Set-Location apps/desktop
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm tauri build --no-bundle
```

Execute o aplicativo nativo real com `pnpm desktop:dev`. Abrir apenas a URL do
Vite mostra intencionalmente que o runtime desktop é necessário e nunca
substitui dados no navegador.

O executável gerado localmente é um artefato de desenvolvimento sem assinatura.
Um build foi removido pelo Norton em 2026-07-29; a classificação permanece
inconclusiva. Não distribua esse binário, não desative a proteção de forma
permanente e não trate o alerta como falso positivo sem análise independente.
Os requisitos de assinatura e reputação estão em
[SDD-013](docs/specs/013-build-release-and-signing.md).

Exemplo da CLI:

```powershell
cargo run -p um-cli -- scan-image C:\imagens\evidencia.img --pretty
```

A CLI e o desktop aceitam somente arquivos-imagem locais comuns. Não informe
disco, volume, compartilhamento de rede, pipe, fluxo alternativo, symlink ou
origem reparse.

## Documentação orientada por especificação

- [Especificação mestre](UNDELETE_MASTER_CODEX_MASTER_SPEC.md)
- [SDD-017 do desktop real-only](docs/specs/017-real-only-image-desktop.md)
- [Requisitos funcionais](docs/specs/001-functional-requirements.md)
- [Arquitetura](docs/specs/004-architecture.md)
- [Contrato de dados do desktop](docs/ui-data-contract.md)
- [Segurança e privacidade](docs/specs/011-security-and-privacy.md)
- [Plano de testes](docs/specs/012-test-and-validation-plan.md)
- [ADRs](docs/adr/README.md)
- [Registro de riscos](docs/risk-register.md)
- [Rastreabilidade](docs/traceability-matrix.md)

## Integração Git

O histórico remoto de `main` é preservado. Branches de desenvolvimento são
integradas sem force-push nem substituição de `main`.
