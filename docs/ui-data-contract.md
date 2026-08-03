# Contrato de dados da interface desktop real-only

Este documento descreve a superficie **implementada** entre o WebView React e
o processo Tauri/Rust. Ele nao e um roteiro futuro: nomes de comandos, campos,
limites e estados abaixo foram derivados de `apps/desktop/src`,
`apps/desktop/src-tauri/src` e dos testes que auditam a superficie de producao.

Versoes atuais:

- inventario e selecao de pasta: `schemaVersion = 1`;
- resumo do scan: `schemaVersion = 3`;
- pagina bruta de candidatos: `schemaVersion = 2`;
- consulta, pagina acionavel e selecao: `schemaVersion = 1`;
- destino, plano, job e abertura do resultado: `schemaVersion = 1`;
- evento `scan-progress`: sem `schemaVersion`, vinculado por `requestId`.

Fontes normativas:

- [SDD-018](specs/018-windows-volume-and-folder-scan.md);
- [SDD-019](specs/019-ntfs-coverage-and-jpeg-deep-scan.md);
- [SDD-020](specs/020-actionable-results-and-transactional-restore.md);
- [ADR-0023](adr/0023-windows-read-only-broker-and-folder-scope.md);
- [ADR-0024](adr/0024-streaming-mft-and-bounded-content-carving.md);
- [ADR-0025](adr/0025-native-result-query-and-selection-authority.md);
- [ADR-0026](adr/0026-bounded-content-plan-and-partial-recovery.md);
- [ADR-0027](adr/0027-destination-capability-and-disk-separation.md);
- [ADR-0028](adr/0028-restore-plan-job-and-manifest-lifecycle.md).

O desktop nao aceita mais uma imagem escolhida pelo usuario. O CLI continua
aceitando somente imagens regulares autorizadas por seu proprio contrato.

## 1. Fronteira de autoridade

O WebView pode enviar apenas dados limitados e nao autoritativos:

- `requestId`, IDs opacos e revisoes decimais;
- a geracao do inventario recebida do Rust;
- os modos fechados `metadata` e `deepJpeg`;
- consulta, ordenacao, cursor e operacao de selecao;
- as politicas fechadas de colisao e arquivo parcial;
- IDs decimais de candidatos ja recebidos do backend.

Nenhum comando aceita caminho nativo de origem ou destino, GUID de volume,
`PhysicalDriveN`, numero de disco, extent, offset, handle, access mask, IOCTL,
programa, linha de comando ou bytes recuperados. O WebView recebe caminhos
**reconstruidos e sanitizados para apresentacao** em `displayPath`, mas eles
nunca sao autoridade para ler ou gravar.

Os seletores de pasta de scan e de destino sao nativos. Seus caminhos absolutos
permanecem no Rust. O WebView recebe somente IDs opacos e labels sanitizados. A
abertura do resultado final tambem usa apenas o `jobId`; o chamador nao escolhe
caminho nem executavel.

A origem e aberta somente para leitura pelo broker elevado. O desktop e o
motor de restore permanecem unelevated. Arquivos recuperados sao conteudo nao
confiavel: nao sao executados nem pre-visualizados por processo privilegiado.

## 2. Inventario exato de comandos e evento

Existem exatamente **12 comandos Tauri** registrados e chamados por um unico
wrapper (`api/storageDesktop.ts`):

1. `list_storage_sources`;
2. `select_scan_folder`;
3. `scan_storage_volume`;
4. `get_candidate_page`;
5. `query_candidate_page`;
6. `update_candidate_selection`;
7. `select_restore_destination`;
8. `create_restore_plan`;
9. `start_restore`;
10. `get_restore_job`;
11. `cancel_restore`;
12. `open_restore_destination`.

O backend emite um evento: `scan-progress`. Nao existem comandos de imagem,
disco fisico, preview, execucao, shell generico ou escrita na origem. O
`deepJpeg` e um modo fechado de `scan_storage_volume`, nao um comando paralelo.

Todos os payloads Tauri usam nomes **camelCase**. Em especial, a mutacao direta
de selecao usa `candidateIds`; `candidate_ids` existe apenas no campo Rust
interno apos a desserializacao Serde.

## 3. Comandos de inventario e scan

### 3.1 `list_storage_sources`

```ts
interface ListStorageSourcesRequest {
  requestId: string;
}
```

Retorna `StorageInventory`.

### 3.2 `select_scan_folder`

```ts
interface SelectScanFolderRequest {
  requestId: string;
  generation: string;
  volumeId: string;
}
```

Retorna `FolderSelection | null`. `null` significa que o usuario cancelou o
seletor nativo; nenhum escopo novo e criado.

### 3.3 `scan_storage_volume`

```ts
interface ScanStorageVolumeRequest {
  requestId: string;
  generation: string;
  volumeId: string;
  scopeId: string | null;
  mode: "metadata" | "deepJpeg";
}
```

Retorna `ScanSummary`. `scopeId = null` seleciona o volume inteiro. O modo
`deepJpeg` exige volume NTFS inteiro e falha fechado para pasta ou filesystem
diferente. O comando reenumera o inventario, compara `generation`, reautoriza
o `volumeId` e somente entao abre a origem pelo broker read-only.

### 3.4 `get_candidate_page` (pagina bruta legada)

```ts
interface GetCandidatePageRequest {
  requestId: string;
  scanId: string;
  cursor: string | null;
  limit: 100;
}
```

Retorna `CandidatePage`. O backend aceita somente `limit = 100`. Este comando
permanece registrado para o contrato de pagina bruta, mas o workspace atual
usa `query_candidate_page`.

## 4. Inventario e escopo

```ts
type BusType =
  | "unknown"
  | "ata"
  | "sata"
  | "scsi"
  | "usb"
  | "nvme"
  | "virtual";

interface StorageInventory {
  schemaVersion: 1;
  generation: string;
  disks: StorageDisk[]; // no maximo 128
}

interface StorageDisk {
  id: string;
  displayName: string;
  busType: BusType;
  sizeBytes: string;
  volumes: StorageVolume[]; // no maximo 128 por grupo
}

interface StorageVolume {
  id: string;
  mountLabel: string; // por exemplo, "G:"
  label: string;
  fileSystem: string;
  sizeBytes: string;
  freeBytes: string;
  isSystem: boolean;
  scanSupported: boolean;
  folderScopeSupported: boolean;
  warnings: string[];
}

interface FolderSelection {
  schemaVersion: 1;
  scopeId: string;
  volumeId: string;
  label: string;
}
```

`StorageDisk` e um agrupador de apresentacao. Nenhum numero de disco fisico e
serializado. Tamanho e espaco livre sao strings decimais `u64`; os valores do
inventario servem para apresentacao e autorizacao limitada, nunca para ampliar
o comprimento canonico que o broker comprova ao abrir a origem.

O `scopeId` representa uma autoridade nativa vinculada a `generation` e
`volumeId`. Caminho absoluto, file reference NTFS, MFT record e sequence ficam
no Rust. Somente NTFS pode anunciar `folderScopeSupported = true`. O estado
retém no maximo 32 escopos; escopos antigos podem expirar.

## 5. Progresso real do scan

O listener e instalado antes do comando de scan e aceita somente o evento
`scan-progress` cujo `requestId` coincide com a requisicao corrente:

```ts
type ScanProgressPhase =
  | "bootstrap"
  | "mftRecords"
  | "namespace"
  | "candidates"
  | "deepJpeg"
  | "complete";

interface ScanProgressEvent {
  requestId: string;
  phase: ScanProgressPhase;
  completed: string; // decimal u64
  total: string;     // decimal u64
}
```

Regras de verdade:

- `completed` e `total` vem do scanner Rust; o frontend nao os incrementa;
- somente `phase = "mftRecords"`, `total > 0` e `completed <= total` produz
  barra percentual determinada;
- durante o comando pendente, o percentual e
  `floor(completed * 100 / total)`, limitado a 99%; 100% so e representado
  pelo termino real do comando;
- `bootstrap`, `namespace`, `candidates`, `deepJpeg`, `complete`, falta de
  evento e indisponibilidade do event bridge usam progresso indeterminado;
- o tempo decorrido parte de `Date.now()` no inicio real do scan e e atualizado
  visualmente a cada segundo;
- a ETA existe somente na fase MFT determinada e deriva do ritmo observado
  (`elapsed * (total - completed) / completed`);
- nenhuma fase sem total mensuravel recebe percentual ou ETA inventado;
- eventos malformados, de outro `requestId` ou de scan antigo sao ignorados.

O scan atual nao possui comando de cancelar, pausar ou retomar. O indicador
indeterminado e o fallback verdadeiro quando o backend nao consegue fornecer
um total confiavel.

## 6. Resumo do scan

```ts
type SupportedFileSystem =
  | "ntfs"
  | "fat12"
  | "fat16"
  | "fat32"
  | "unrecognized";

type ScanStatus = "complete" | "partial" | "unrecognized";
type ScanMode = "metadata" | "deepJpeg";

interface ScanScope {
  kind: "volume" | "folder";
  label: string;
}

interface MftScanCoverage {
  recordsDeclared: string;
  recordsAvailable: string;
  recordsExamined: string;
  bytesDeclared: string;
  bytesAvailable: string;
  bytesExamined: string;
}

interface JpegCarveCoverage {
  bytesRequested: string;
  bytesScanned: string;
  signaturesAttempted: string;
  validationBytesRead: string;
  partial: boolean;
  readErrorCount: string;
  candidateLimitReached: boolean;
  candidateByteLimitHits: string;
  signatureAttemptLimitReached: boolean;
  validationByteLimitReached: boolean;
  rejectedSignatures: string;
  truncatedSignatures: string;
  regionsSubmitted: string;
  regionLimitReached: boolean;
}

interface ScanSummary {
  schemaVersion: 3;
  scanId: string;
  sourceLabel: string;
  scope: ScanScope;
  scanMode: ScanMode;
  fileSystem: SupportedFileSystem;
  scanStatus: ScanStatus;
  totalCandidates: string;
  matchedCandidates: string;
  unknownCandidates: string;
  mftCoverage: MftScanCoverage | null;
  jpegCarveCoverage: JpegCarveCoverage | null;
  warnings: string[];
}
```

NTFS reconhecido retorna `mftCoverage`; FAT e `unrecognized` retornam `null`.
Os contadores declarados, disponiveis e examinados distinguem o tamanho logico
do `$MFT`, o prefixo fisico confiavel e o teto de trabalho efetivamente
tentado. Igualdade entre eles nao prova scan completo: corrupcao, namespace
incompleto ou outro limite pode manter `scanStatus = "partial"`.

`metadata` exige `jpegCarveCoverage = null`. `deepJpeg` exige escopo de volume,
NTFS e cobertura JPEG. Seus `bytesRequested` representam apenas regioes
comprovadamente livres pelo snapshot confiavel de `$Bitmap`; nao representam o
volume inteiro. Ausencia ou truncamento dessa evidencia nunca amplia o scan
para RAW arbitrario.

Em pasta, somente candidatos com ancestralidade `Match` entram na entrega;
ancestralidade `Unknown` e contabilizada separadamente. Contagens, score e
confianca nao garantem integridade nem recuperabilidade. Zero em resultado
parcial significa zero dentro da cobertura observada.

O estado nativo retém no maximo quatro sessoes de scan. Cada sessao aceita no
maximo 110.000 candidatos (100.000 por metadados NTFS mais 10.000 carved). A
eviccao torna IDs/cursors daquela sessao expirados; nao ha persistencia
silenciosa apos encerrar o processo.

## 7. Candidatos: pagina bruta e pagina acionavel

```ts
type CandidateKind = "file" | "directory";
type MetadataConfidence = "high" | "medium" | "low";
type RecoveryEligibility = "complete" | "bestEffort" | "ineligible";

type CandidateState =
  | "exactEvidence"
  | "likelyComplete"
  | "completeUnvalidated"
  | "structurallyValid"
  | "partial"
  | "conflicted"
  | "readError"
  | "zeroedOrTrimmed"
  | "overwritten"
  | "metadataOnly"
  | "unknown";

type PathState =
  | "exact"
  | "reconstructed"
  | "incomplete"
  | "orphaned"
  | "ambiguous";

type DiscoveryMethod =
  | "ntfsMetadata"
  | "fatMetadata"
  | "exfatMetadata"
  | "carving"
  | "recycleBin";

interface CandidateRow {
  id: string; // candidate ID decimal u64
  displayPath: string;
  kind: CandidateKind;
  state: CandidateState;
  sizeBytes: string;
  metadataConfidence: MetadataConfidence;
  recoverabilityScore: number | null;
  pathState: PathState;
  method: DiscoveryMethod;
  contentSha256: string | null;
  validator: string | null;
  warnings: string[];
}

interface CandidatePage {
  schemaVersion: 2;
  scanId: string;
  cursor: string | null;
  nextCursor: string | null;
  candidates: CandidateRow[]; // no maximo 100
}

interface ActionableCandidateRow {
  id: string;
  displayPath: string;
  extension: string;
  kind: CandidateKind;
  state: CandidateState;
  sizeBytes: string;
  metadataConfidence: MetadataConfidence;
  recoverabilityScore: number | null;
  pathState: PathState;
  method: DiscoveryMethod;
  eligibility: RecoveryEligibility;
  selected: boolean;
  warnings: string[];
}
```

Arquivos tem score inteiro de 0 a 100; diretorios tem score `null`. A pagina
bruta pode expor hash/validador somente quando o Rust reteve evidencia exata.
A pagina acionavel deliberadamente expoe apenas `eligibility` e `selected`;
extents, offsets, hashes esperados e planos de conteudo continuam nativos.

## 8. Consulta, filtros, ordenacao, paginacao e cache

### 8.1 `query_candidate_page`

```ts
interface CandidateQuery {
  revision: string;
  search: string;
  extensions: string[];
  kinds: CandidateKind[];
  metadataConfidences: MetadataConfidence[];
  methods: DiscoveryMethod[];
  states: CandidateState[];
  minRecoverabilityScore: number | null;
  maxRecoverabilityScore: number | null;
  eligibilities: RecoveryEligibility[];
  selectedOnly: boolean;
}

type CandidateSortField =
  | "path"
  | "extension"
  | "size"
  | "state"
  | "confidence"
  | "recoverabilityScore"
  | "method";

interface CandidateSort {
  field: CandidateSortField;
  direction: "ascending" | "descending";
}

interface QueryCandidatePageRequest {
  requestId: string;
  scanId: string;
  query: CandidateQuery;
  sort: CandidateSort;
  cursor: string | null;
}
```

O limite de 100 linhas e fixo no backend; este comando nao recebe `limit`.
Retorna:

```ts
interface CandidateExtensionFacet {
  extension: string; // vazio representa sem extensao declarada
  count: string;
}

interface CandidateSelectionSummary {
  selectionRevision: string;
  selectedCandidates: string;
  selectedFiles: string;
  selectedDirectories: string;
  selectedLogicalBytes: string;
  bestEffortCandidates: string;
  conflictedCandidates: string;
  ineligibleCandidates: string;
  matchingSelectedCandidates: string;
}

interface CandidateQueryPage {
  schemaVersion: 1;
  scanId: string;
  queryId: string;
  queryRevision: string;
  cursor: string | null;
  nextCursor: string | null;
  filteredTotal: string;
  extensionFacets: CandidateExtensionFacet[];
  selection: CandidateSelectionSummary;
  candidates: ActionableCandidateRow[]; // no maximo 100
}
```

Filtros sao combinados por AND entre grupos e por OR dentro de cada lista. A
busca e case-insensitive sobre `displayPath`. Extensoes sao normalizadas para
lowercase, ordenadas e deduplicadas; listas de enums tambem sao canonicas.
`search` aceita ate 512 escalares, existem no maximo 128 extensoes, cada uma
com ate 255 escalares, e scores devem ficar entre 0 e 100 com minimo <= maximo.
Caracteres de controle e formatacao bidi falham fechados.

Toda ordenacao usa candidate ID crescente como desempate final. Scores `null`
ficam depois dos scores numericos nas duas direcoes. A ordenacao inicial do
workspace e `recoverabilityScore` descendente; um novo campo inicia crescente,
exceto score, que inicia descendente.

O Rust retém uma unica consulta ativa por scan. `queryId` deriva da origem,
scan e consulta canonica; o cursor tambem e vinculado a ordenacao, offset e,
quando `selectedOnly = true`, revisao da selecao. Cursor estrangeiro, antigo ou
incompativel retorna `RESULT_CURSOR_STALE`. Uma nova consulta invalida o
binding anterior.

O frontend:

- ignora respostas tardias de outra geracao ou revisao de intencao;
- mantem historico de cursor para anterior/proxima pagina;
- conserva no maximo tres paginas em cache;
- invalida o cache ao mudar consulta/sort ou ao alterar selecao;
- recarrega a pagina atual apos selecao;
- volta a primeira pagina apos selecao quando `selectedOnly` esta ativo;
- refaz uma consulta fresca ao detectar cursor ou selecao obsoleta;
- serializa tarefas nativas para impedir que uma resposta antiga sobrescreva
  a intencao atual.

## 9. Selecao individual, por linha e do conjunto filtrado

### 9.1 `update_candidate_selection`

```ts
type CandidateSelectionOperation =
  | {
      type: "setIds";
      candidateIds: string[];
      selected: boolean;
    }
  | { type: "selectAllMatching" }
  | { type: "clearMatching" }
  | { type: "clearAll" };

interface UpdateCandidateSelectionRequest {
  requestId: string;
  scanId: string;
  queryId: string;
  operation: CandidateSelectionOperation;
  selectionRevision: string;
}

interface CandidateSelectionUpdate {
  schemaVersion: 1;
  scanId: string;
  queryId: string;
  selectionRevision: string;
  selection: CandidateSelectionSummary;
}
```

`setIds` aceita de 1 a 100 IDs decimais, unicos e existentes. O checkbox de
cada arquivo envia `candidateIds: [candidate.id]`; clicar na linha, fora do
checkbox, executa a mesma mutacao individual. Portanto a selecao de uma linha
nao depende do checkbox de cabecalho.

O checkbox do cabecalho usa `matchingSelectedCandidates` versus
`filteredTotal` para representar desmarcado, misto ou marcado. Ao marcar,
executa `selectAllMatching` no conjunto filtrado completo em Rust, inclusive
paginas nao visiveis; ao desmarcar, executa `clearMatching`. A barra de selecao
oferece ainda `clearAll`.

A selecao canonica fica no backend e persiste ao trocar pagina, filtro, busca
ou ordenacao enquanto a sessao do scan existir. Cada mutacao carrega a revisao
esperada, e cada sucesso a incrementa exatamente uma vez. Revisao antiga,
`queryId` invalido, ID duplicado ou candidato desconhecido rejeita a operacao
inteira sem mutacao parcial. Durante uma mutacao os controles de selecao ficam
temporariamente desabilitados.

Itens `ineligible` continuam selecionaveis para inspecao, mas qualquer item
ineligivel bloqueia o botao de recuperar. A barra mostra arquivos, diretorios,
bytes logicos, best-effort, conflitos e inelegiveis da selecao global.

## 10. Destino e plano de restore

### 10.1 `select_restore_destination`

```ts
interface SelectRestoreDestinationRequest {
  requestId: string;
  scanId: string;
}

interface RestoreDestinationSummary {
  schemaVersion: 1;
  destinationId: string;
  label: string;
  volumeLabel: string;
  fileSystem: "NTFS";
  freeBytes: string;
  relation: "different";
}
```

Retorna `RestoreDestinationSummary | null`; `null` e cancelamento do picker.
O Rust retém o handle/capability da pasta escolhida e exige destino local,
NTFS, nao-reparse, com backing fisico conhecido de um unico disco e diferente
do disco da origem. O destino e revalidado ao planejar e iniciar; a origem e
vinculada ao plano, conferida com a selecao no inicio e reaberta/revalidada pelo
worker read-only antes de criar qualquer entrada no destino. Mesma unidade
logica nao e o criterio: a separacao e por disco fisico.
Existem no maximo 32 autoridades de destino por processo; o limite falha
fechado, sem eviccao silenciosa.

### 10.2 `create_restore_plan`

```ts
interface CreateRestorePlanRequest {
  requestId: string;
  scanId: string;
  selectionRevision: string;
  destinationId: string;
  collisionPolicy: "rename";
  partialFilePolicy: "completeOnly" | "zeroFillAndMap";
}

interface RestorePlanSummary {
  schemaVersion: 1;
  planId: string;
  planDigest: string; // SHA-256 lowercase
  scanId: string;
  destinationId: string;
  selectionRevision: string;
  collisionPolicy: "rename";
  partialFilePolicy: "completeOnly" | "zeroFillAndMap";
  itemsTotal: string;
  filesTotal: string;
  directoriesTotal: string;
  logicalBytes: string;
  bestEffortItems: string;
}
```

O plano usa a selecao nativa da revisao exata; o WebView nao envia a lista de
arquivos novamente. A politica `rename` nunca sobrescreve um nome existente.
`completeOnly` rejeita itens best-effort. `zeroFillAndMap` exige consentimento
explicito na UI e gera evidencia de lacunas/ranges. Itens ineligiveis, selecao
vazia, path inseguro, destino expirado ou espaco insuficiente falham antes do
job. O plano e imutavel, vincula candidatos ordenados, origem, destino e
politicas, e pode ser iniciado uma unica vez.

O coordenador retém no maximo oito planos e oito jobs. Saturacao rejeita nova
admissao; jobs ativos ou evidencia terminal nao sao removidos silenciosamente.

## 11. Job, progresso, cancelamento e abertura

### 11.1 Comandos

```ts
interface StartRestoreRequest {
  requestId: string;
  planId: string;
}

interface RestoreJobRequest {
  requestId: string;
  jobId: string;
}
```

- `start_restore` recebe `StartRestoreRequest` e retorna o snapshot inicial;
- `get_restore_job` recebe `RestoreJobRequest` e retorna o snapshot atual;
- `cancel_restore` recebe `RestoreJobRequest`, solicita cancelamento cooperativo
  e retorna o snapshot atual;
- `open_restore_destination` recebe `RestoreJobRequest` e retorna
  `{ schemaVersion: 1, opened: true }`.

```ts
type RestoreJobStatus =
  | "queued"
  | "running"
  | "cancelling"
  | "completed"
  | "failed"
  | "cancelled";

interface RestoreCurrentItem {
  ordinal: string;
  candidateId: string;
  kind: CandidateKind;
}

interface RestoreManifestSummary {
  manifestSha256: string;
  completionStatus: "completedDurable" | "needsReconciliation";
  publishedItems: string;
  partialItems: string;
}

interface RestoreJobSnapshot {
  schemaVersion: 1;
  jobId: string;
  planId: string;
  status: RestoreJobStatus;
  itemsTotal: string;
  itemsCompleted: string;
  itemsFailed: string;
  itemsCancelled: string;
  bytesTotal: string;
  bytesCompleted: string;
  currentItem: RestoreCurrentItem | null;
  warnings: string[];
  manifest: RestoreManifestSummary | null;
}
```

O frontend consulta o job a cada 1 segundo. O percentual real de restore usa
`bytesCompleted / bytesTotal`; quando `bytesTotal = 0`, usa a soma de itens
concluidos, falhos e cancelados sobre `itemsTotal`. O valor interno tem 10.000
pontos-base e nunca regride. Contadores, status, item corrente e manifesto vem
do job nativo; nao existe timer que fabrique trabalho.

O parser aceita apenas transicoes legais e contadores monotonicos. Snapshot
terminal precisa permanecer identico em consultas posteriores. Um job
`completed` exige todos os itens concluidos, zero falhas/cancelamentos, todos
os bytes concluidos e manifesto publicado. Jobs nao terminais nao podem expor
manifesto.

O cancelamento e cooperativo entre limites seguros de leitura/escrita; ele nao
interrompe uma fronteira atomica de publicacao. A UI serializa poll e cancel,
remove polls pendentes ao cancelar e continua acompanhando o snapshot retornado.
Apos tres falhas internas consecutivas de polling, entra em `trackingLost` em
vez de inventar um resultado final.

`open_restore_destination` funciona somente para job `completed` com manifesto.
O Rust revalida a capability e pede ao shell fixo do Windows para abrir apenas
o diretorio opaco daquele job. Nao ha fallback para `window.open`, plugin de
shell, caminho ou executavel escolhido pelo chamador.

## 12. Estados implementados da interface

### 12.1 Inventario e scan

- inventario: `unavailable`, `loading`, `refreshing`, `ready`, `error`;
- pasta: `idle`, `selecting`, alem do sinal explicito de cancelamento;
- scan: `idle`, `scanning`, `success`, `error`;
- modo: `metadata` ou `deepJpeg`, com deep desabilitado para pasta/nao-NTFS;
- progresso: evento real determinado de MFT ou indicador verdadeiro
  indeterminado para fases sem total;
- resultado: `complete`, `partial` ou `unrecognized`.

### 12.2 Workspace de resultados

- pagina: `idle`, `loading`, `ready`, `error`;
- selecao: `idle`, `updating`;
- query vazia, filtrada, selecionados somente, pagina anterior/proxima;
- erro inicial sem linhas ou erro de refresh preservando a pagina ja exibida;
- checkbox individual, clique de linha, checkbox filtrado tri-state, limpar
  correspondentes e limpar tudo.

### 12.3 Restore

- workflow: `idle`, `selectingDestination`, `setup`, `planning`, `review`,
  `starting`, `active`, `trackingLost`, `finished`, `error`;
- operacao secundaria: `idle`, `polling`, `cancelRequest`, `opening`;
- job: `queued`, `running`, `cancelling`, `completed`, `failed`, `cancelled`;
- dialogo nao fecha durante selecao, planejamento, inicio ou job ativo;
- mudanca de revisao da selecao invalida plano ainda nao iniciado;
- abertura da pasta permanece repetivel se uma tentativa de shell falhar.

Nao ha resultado sintetico, linha de fallback, progresso simulado ou sucesso
presumido. Uma falha conserva apenas a evidencia real que ja estava validada.

## 13. Erros estaveis

### 13.1 Storage, scan, consulta e selecao

| Codigo | Significado publico |
| --- | --- |
| `DESKTOP_RUNTIME_UNAVAILABLE` | O app nao esta no runtime Tauri. |
| `INVENTORY_UNAVAILABLE` | O inventario nativo nao pode ser obtido. |
| `SOURCE_UNSUPPORTED` | A origem nao atende a politica read-only. |
| `SOURCE_GONE` | A origem selecionada nao esta mais disponivel. |
| `SOURCE_IDENTITY_CHANGED` | Inventario ou identidade da origem mudou. |
| `FOLDER_SCOPE_UNSUPPORTED` | O volume nao permite escopo verificavel. |
| `FOLDER_SCOPE_MISMATCH` | O escopo nao pertence a origem atual. |
| `SCAN_MODE_UNSUPPORTED` | O modo nao e permitido para filesystem/escopo. |
| `UAC_CANCELLED` | O usuario cancelou a elevacao do broker. |
| `BROKER_UNAVAILABLE` | O broker read-only nao esta disponivel. |
| `BROKER_PROTOCOL` | A sessao do broker falhou fechado. |
| `SOURCE_IO` | A origem nao pode ser aberta/lida completamente. |
| `SCAN_CORRUPT` | Estruturas reconhecidas nao puderam ser validadas. |
| `SCAN_INTERNAL` | Coordenacao/worker nativo nao concluiu. |
| `RESULT_QUERY_INVALID` | Consulta, revisao ou operacao invalida. |
| `RESULT_CURSOR_STALE` | Cursor nao pertence mais a consulta ativa. |
| `RESULT_SELECTION_STALE` | A revisao da selecao mudou. |
| `RESULT_SELECTION_INVALID` | A selecao contem candidato desconhecido. |
| `REPORT_INCOMPATIBLE` | A resposta viola o schema/invariantes. |

Erro desconhecido nesta familia e normalizado para `SCAN_INTERNAL`.

### 13.2 Restore

| Codigo | Significado publico |
| --- | --- |
| `REPORT_INCOMPATIBLE` | Resposta ou snapshot viola o contrato. |
| `RESTORE_DESTINATION_INVALID` | Destino nao pode ser autorizado/revalidado. |
| `RESTORE_DESTINATION_LIMIT` | Limite de authorities de destino atingido. |
| `RESTORE_DESTINATION_EXPIRED` | A authority do destino expirou ou mudou. |
| `RESTORE_DIFFERENT_DISK_REQUIRED` | Destino deve estar em outro disco fisico conhecido. |
| `RESTORE_SOURCE_CHANGED` | Origem mudou desde scan/plano. |
| `RESTORE_SELECTION_STALE` | Selecao mudou antes do plano/inicio. |
| `RESTORE_SELECTION_EMPTY` | Nenhum candidato foi selecionado. |
| `RESTORE_ITEM_INELIGIBLE` | Um item nao possui plano de conteudo seguro. |
| `RESTORE_PARTIAL_POLICY_REQUIRED` | Best-effort exige `zeroFillAndMap` e consentimento. |
| `RESTORE_PLAN_INVALID` | Plano nao pode ser criado/iniciado. |
| `RESTORE_PLAN_LIMIT` | Limite de planos retidos atingido. |
| `RESTORE_PLAN_EXPIRED` | Plano nao esta mais retido/valido. |
| `RESTORE_JOB_LIMIT` | Limite de jobs retidos atingido. |
| `RESTORE_JOB_NOT_FOUND` | Job nao esta mais retido. |
| `RESTORE_JOB_NOT_COMPLETE` | Somente job concluido pode abrir o destino. |
| `RESTORE_INTERNAL` | Coordenador, worker ou operacao nativa falhou. |

Erro desconhecido nesta familia e normalizado para `RESTORE_INTERNAL`.
Mensagens publicas sao genericas e nao incluem caminho nativo, GUID, numero de
disco, pipe, handle, offset, bytes da origem ou backtrace.

## 14. Validacao, numeros, IDs e texto

- `requestId` aceita 1 a 128 caracteres ASCII alfanumericos, `_` ou `-`;
- IDs opacos aceitam `[A-Za-z0-9_-]{1,128}`;
- candidate IDs e revisoes usam string decimal canonica `u64` entre `0` e
  `18446744073709551615`;
- cada outro contador `u64` tambem cruza a fronteira como string decimal;
- hashes SHA-256 usam 64 caracteres hexadecimais lowercase;
- texto de DTO tem no maximo 512 code points;
- warnings tem no maximo 128 entradas;
- controles C0/C1, controles bidi e campos desconhecidos falham fechados;
- IDs duplicados, contagens inconsistentes, 101a linha acionavel e regressao
  de contador/revisao tornam a resposta `REPORT_INCOMPATIBLE`;
- labels de destino nao podem conter `/` ou `\\`; warnings de restore tambem
  rejeitam caminho com drive letter;
- estado de scan, selecao, destino, plano e job e somente em memoria e e
  perdido ao encerrar o desktop.

## 15. Acessibilidade e apresentacao segura

Volumes sao radios com labels acessiveis. A tabela possui caption, regiao
nomeada e `aria-sort`; paths reconstruidos sao texto dentro de `bdi`, nunca
HTML. O checkbox filtrado expoe estado misto, e mudancas/erros usam regioes
`aria-live`. Foco, tema, reduced motion e navegacao por teclado permanecem
ativos.

Validacao empacotada com tecnologia assistiva, forced colors e zoom de 200%
continua sendo gate de aceitacao separado; os schemas acima nao transformam
teste local em evidencia de campo.
