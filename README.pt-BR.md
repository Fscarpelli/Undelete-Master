# Undelete Master

O Undelete Master é um projeto local de recuperação de dados, inicialmente para
Windows, com foco em preservar a origem e apresentar resultados baseados em
evidências.

> **Estado de desenvolvimento:** o desktop já detecta armazenamentos locais
> conectados, analisa um volume montado escolhido pelo usuário e, em NTFS, pode
> limitar os resultados a uma pasta selecionada. A restauração ainda não está
> implementada. Não use esta pré-versão como único meio para recuperar dados
> importantes.

[English](README.md)

## O que está implementado

- Abstração Rust `SourceReader` sem operação de escrita.
- Descoberta real e sem elevação dos volumes montados compatíveis no Windows. Os
  cartões iniciais são agrupamentos lógicos; os extents físicos só são
  resolvidos pelo broker elevado depois que o usuário inicia a análise.
- Aplicativo desktop Tauri 2 real e sem elevação, sem seletor de arquivo-imagem,
  provedor mock, resultado fabricado ou progresso fictício.
- Seleção de um volume montado compatível e, em NTFS, de uma pasta opcional. O
  caminho nativo permanece no Rust e nunca atravessa o IPC até o WebView.
- Broker Windows temporário, elevado e somente leitura. Ele aceita apenas
  identidades opacas de volume e leituras limitadas, revalida a identidade da
  origem e não possui API de escrita, trim, formatação, lock, dismount,
  restauração ou comando genérico de dispositivo.
- Varredura defensiva de metadados NTFS e FAT12/16/32, com resultados reais,
  limitados e paginados. No filtro de pasta NTFS, somente ancestrais comprovados
  aparecem; ancestralidade desconhecida é contabilizada separadamente.
- CLI separada `undelete-master scan-image` para arquivos-imagem locais comuns,
  com JSON sanitizado.
- Testes determinísticos de fixtures e protocolo. Nenhum teste automatizado
  analisa volume montado comum ou disco físico real.
- Barreiras de CI contra operações destrutivas e contra a volta de seleção de
  imagem, dados fabricados ou uma superfície de comandos ampla demais.

O estado exato está na
[matriz de rastreabilidade](docs/traceability-matrix.md). Um candidato de
metadados não é garantia de que seu conteúdo possa ser recuperado.

## O que está deliberadamente ausente

O desktop não analisa o disco físico inteiro, partição desmontada, volume
multidisco, compartilhamento de rede, unidade óptica ou RAM disk. O filtro por
pasta exige NTFS. Restauração, prévia, sessões persistentes, carving, exFAT,
pausa/retomada, cancelamento e instalador assinado ainda não estão
implementados. Consulte as
[limitações conhecidas](docs/specs/015-known-limitations.md).

## Segurança

- Nunca use testes para analisar disco real ou volume montado comum.
- Nunca adicione escrita, trim, formatação, lock, dismount ou comando genérico
  de dispositivo ao caminho de análise.
- Use fixtures determinísticas do repositório, imagens em memória, arquivos
  regulares temporários ou um VHD de teste descartável, governado e
  explicitamente permitido.
- Nunca execute conteúdo recuperado.
- Mantenha o desktop sem elevação. A elevação fica restrita ao broker somente
  leitura, iniciado quando o usuário solicita expressamente a análise.

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
```

Compile e execute o aplicativo nativo:

```powershell
Set-Location apps/desktop
pnpm desktop:dev
# ou gere o desktop release e seu broker irmão
pnpm desktop:build
```

Abrir apenas a URL do Vite mostra intencionalmente que o runtime desktop é
necessário e nunca substitui dados reais no navegador.

Os executáveis locais são artefatos de desenvolvimento sem assinatura. Um
build foi removido pelo Norton em 2026-07-29; a classificação permanece
inconclusiva. Não redistribua esse binário, não desative a proteção
permanentemente e não trate o alerta como falso positivo sem análise
independente. Os requisitos de assinatura e reputação estão em
[SDD-013](docs/specs/013-build-release-and-signing.md).

Exemplo da CLI para imagem:

```powershell
cargo run -p um-cli -- scan-image C:\imagens\evidencia.img --pretty
```

A CLI aceita apenas arquivos-imagem locais comuns. O desktop usa o fluxo de
volumes montados conectados descrito acima e não oferece seleção de imagem.

## Documentação orientada por especificação

- [Especificação mestre](UNDELETE_MASTER_CODEX_MASTER_SPEC.md)
- [SDD-018 — volumes e pastas no Windows](docs/specs/018-windows-volume-and-folder-scan.md)
- [ADR-0023 — broker somente leitura e escopo de pasta](docs/adr/0023-windows-read-only-broker-and-folder-scope.md)
- [Requisitos funcionais](docs/specs/001-functional-requirements.md)
- [Arquitetura](docs/specs/004-architecture.md)
- [Segurança e privacidade](docs/specs/011-security-and-privacy.md)
- [Plano de testes](docs/specs/012-test-and-validation-plan.md)
- [ADRs](docs/adr/README.md)
- [Registro de riscos](docs/risk-register.md)
- [Rastreabilidade](docs/traceability-matrix.md)

## Integração Git

O histórico remoto de `main` é preservado. Branches de desenvolvimento são
integradas sem force-push nem substituição de `main`.
