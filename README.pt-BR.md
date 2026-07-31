# Undelete Master

O Undelete Master é um projeto local de recuperação de dados, inicialmente para
Windows, com foco em preservar a origem e apresentar resultados baseados em
evidências.

> **Estado de desenvolvimento:** o desktop já detecta armazenamentos locais
> conectados, analisa um volume montado escolhido pelo usuário e, em NTFS, pode
> limitar os resultados a uma pasta selecionada. Um modo explícito para o
> volume NTFS inteiro também executa carving limitado de JPEGs contíguos no
> espaço comprovadamente livre. O workspace de resultados acionáveis e o fluxo
> transacional de restauração estão implementados e verificados localmente com
> fixtures, mas a aceitação empacotada em dispositivo real ainda não foi
> comprovada. Não use esta pré-versão como único meio para recuperar dados
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
  limitados e paginados. A enumeração NTFS informa cobertura quantitativa da
  MFT e não para mais no antigo prefixo de 64 MiB. No filtro de pasta NTFS,
  somente ancestrais comprovados aparecem; ancestralidade desconhecida é
  contabilizada separadamente.
- Modo explícito de análise profunda de JPEG para o volume NTFS inteiro. Ele
  envia ao carver somente regiões que um snapshot validado do `$Bitmap`
  comprova estarem livres, valida a estrutura de forma incremental e limitada,
  e registra faixa física, SHA-256, versão do validador, cobertura e limites de
  trabalho. O modo nunca se expande silenciosamente para espaço alocado ou
  desconhecido.
- Workspace de resultados sob autoridade do backend, com busca explícita,
  facets dinâmicas de extensão, filtros de evidência, ordenação estável, páginas
  limitadas por cursor e seleção nativa preservada entre páginas, consultas,
  filtros e ordenações durante o processo atual.
- Fluxo transacional nativo para restaurar somente candidatos selecionados. Ele
  exige uma pasta NTFS autorizada em um único disco físico diferente e
  comprovado, preserva a árvore recuperada, renomeia colisões sem substituir
  arquivos existentes, lê em blocos limitados e publica um manifesto JSON
  versionado. A restauração baseada em metadados não depende da extensão do
  nome quando existe um plano de conteúdo utilizável; isso não torna qualquer
  candidato recuperável nem amplia o carving além do plugin JPEG implementado.
- Consentimento explícito para melhor esforço e sidecars com as faixas exatas
  preenchidas com zeros. Progresso por itens/bytes, cancelamento, contagens
  terminais e identidade do manifesto vêm do trabalho nativo. A ação final abre
  somente a pasta do trabalho concluído e nunca executa arquivo recuperado.
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
pasta exige NTFS. O carving está limitado a JPEGs contíguos no espaço NTFS
comprovadamente livre; outros formatos, reconstrução fragmentada, análise RAW
de sistema danificado e carving de pasta ainda não existem. A restauração não
pode usar o disco de origem, o caminho original nem um destino não NTFS ou sem
identidade comprovada, e não preserva ACLs, EFS, alternate data streams ou
compressão transparente. Prévia, execução de conteúdo, sessões persistentes,
retomada após reiniciar, layouts alternativos de destino,
progresso/ETA/cancelamento da análise e instalador assinado também estão
ausentes. Trabalhos de restauração possuem progresso nativo e cancelamento
cooperativo; esses controles não se aplicam à análise de metadados ou profunda.
Consulte as
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
- Restaure somente para a autoridade de destino retida em outro disco físico
  comprovado; as escritas ficam no componente de restauração sem elevação,
  nunca no broker da origem.

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
- [SDD-019 — cobertura NTFS e análise JPEG profunda limitada](docs/specs/019-ntfs-coverage-and-jpeg-deep-scan.md)
- [SDD-020 — resultados acionáveis e restauração transacional](docs/specs/020-actionable-results-and-transactional-restore.md)
- [ADR-0023 — broker somente leitura e escopo de pasta](docs/adr/0023-windows-read-only-broker-and-folder-scope.md)
- [ADR-0024 — MFT em streaming e carving limitado](docs/adr/0024-streaming-mft-and-bounded-content-carving.md)
- [ADR-0025 — consulta nativa e autoridade da seleção](docs/adr/0025-native-result-query-and-selection-authority.md)
- [ADR-0026 — planos limitados de conteúdo e recuperação parcial](docs/adr/0026-bounded-content-plan-and-partial-recovery.md)
- [ADR-0027 — capacidade de destino e separação de discos](docs/adr/0027-destination-capability-and-disk-separation.md)
- [ADR-0028 — ciclo de vida de plano, trabalho e manifesto](docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md)
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
