# Contrato de dados do desktop connected-storage real-only

Versão do schema: `1`
Fonte normativa:
[SDD-018](specs/018-windows-volume-and-folder-scan.md)
Decisão arquitetural:
[ADR-0023](adr/0023-windows-read-only-broker-and-folder-scope.md)

Este contrato substitui o contrato de imagem do SDD-017 apenas no desktop. O
CLI continua aceitando imagens regulares.

## 1. Fronteira de autoridade

O WebView pode enviar:

- `requestId` curto e sem caracteres de controle;
- IDs opacos de volume, escopo, scan e cursor;
- `limit = 100` no pedido de página.

O WebView não pode enviar nem receber:

- caminho de arquivo, pasta, volume, device ou pipe;
- GUID de volume, `PhysicalDriveN`, handle, extent ou offset;
- access mask, IOCTL, executável ou argumento de processo;
- bytes da origem ou conteúdo recuperado.

O caminho escolhido no seletor de pasta existe somente no Rust nativo. A
resposta contém apenas `scopeId`, `volumeId` e um label sanitizado.

## 2. Superfície exata de comandos

### `list_storage_sources`

```ts
interface ListStorageSourcesRequest {
  requestId: string;
}
```

Retorna `StorageInventory`.

### `select_scan_folder`

```ts
interface SelectScanFolderRequest {
  requestId: string;
  volumeId: string;
}
```

Retorna `FolderSelection | null`. `null` significa cancelamento do seletor
nativo e não muda o escopo atual.

### `scan_storage_volume`

```ts
interface ScanStorageVolumeRequest {
  requestId: string;
  volumeId: string;
  scopeId: string | null;
}
```

Retorna `ScanSummary`. `scopeId = null` pede o volume montado inteiro.

### `get_candidate_page`

```ts
interface GetCandidatePageRequest {
  requestId: string;
  scanId: string;
  cursor: string | null;
  limit: 100;
}
```

Retorna `CandidatePage`. O backend rejeita qualquer limite diferente de 100.

Não há comando de imagem, scan de disco físico, cancelamento, restore, preview,
abrir conteúdo ou sessão persistente.

## 3. Inventário

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
  disks: StorageDisk[]; // no máximo 128
}

interface StorageDisk {
  id: string;
  number: null;         // inventário não alega número de disco físico
  displayName: string;
  busType: BusType;
  sizeBytes: string;    // decimal u64
  volumes: StorageVolume[]; // no máximo 128 por disco
}

interface StorageVolume {
  id: string;
  mountLabel: string;   // formato "C:"
  label: string;
  fileSystem: string;
  sizeBytes: string;    // decimal u64
  freeBytes: string;    // decimal u64 e <= sizeBytes
  isSystem: boolean;
  scanSupported: boolean;
  folderScopeSupported: boolean;
  warnings: string[];
}
```

`StorageDisk` mantém o nome legado do DTO, mas é somente um agrupador lógico e
sempre tem `number = null` neste incremento. O inventário unelevated não abre
DASD, não usa IOCTL/extents e não alega mapear um disco físico. O usuário
seleciona um `StorageVolume`; o grupo nunca autoriza `PhysicalDriveN`.

O ID estável do volume deriva de GUID mais serial e exclui mount letter,
filesystem, size/free quota-visible, extents e número de disco.
`sizeBytes`/`freeBytes` vindos de `GetDiskFreeSpaceExW` são apenas display da
quota visível, nunca autoridade ou comprimento canônico.

O backend inclui apenas volumes montados observados. Remote/mapped, CD, RAM e
unknown são inelegíveis unelevated. O broker elevado consulta extents e rejeita
sem mapeamento/multi-disk somente após o usuário iniciar o scan. Somente NTFS
pode ter `folderScopeSupported = true`.

## 4. Escopo de pasta

```ts
interface FolderSelection {
  schemaVersion: 1;
  scopeId: string;
  volumeId: string;
  label: string;
}
```

O `scopeId` representa uma autoridade nativa limitada e vinculada ao volume.
Internamente, ela contém volume serial, referência NTFS, MFT record e sequence.
Esses valores e o caminho absoluto não entram no DTO.

## 5. Resumo do scan

```ts
type SupportedFileSystem =
  | "ntfs"
  | "fat12"
  | "fat16"
  | "fat32"
  | "unrecognized";

type ScanStatus = "complete" | "partial" | "unrecognized";

interface ScanScope {
  kind: "volume" | "folder";
  label: string;
}

interface ScanSummary {
  schemaVersion: 1;
  scanId: string;
  sourceLabel: string;
  scope: ScanScope;
  fileSystem: SupportedFileSystem;
  scanStatus: ScanStatus;
  totalCandidates: string;   // decimal u64
  matchedCandidates: string; // decimal u64
  unknownCandidates: string; // decimal u64
  warnings: string[];
}
```

`fileSystem = "unrecognized"` exige `scanStatus = "unrecognized"` e vice-versa.
NTFS e FAT reconhecidos usam `complete` ou `partial`.

Em scan de volume, todos os candidatos observados são “matched” para fins de
entrega. Em escopo de pasta, somente ancestrais `Match` entram nas páginas;
ancestrais `Unknown` entram apenas em `unknownCandidates`; `NoMatch` não entra
em nenhuma dessas duas contagens de entrega.

Nenhuma contagem prova conteúdo intacto. Em `partial`, ela é observada dentro da
cobertura atingida, não exaustiva.

## 6. Candidatos e paginação

```ts
type CandidateKind = "file" | "directory";

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

type MetadataConfidence = "high" | "medium" | "low";

type PathState =
  | "exact"
  | "reconstructed"
  | "incomplete"
  | "orphaned"
  | "ambiguous";

interface CandidateRow {
  id: string;
  displayPath: string;
  kind: CandidateKind;
  state: CandidateState;
  sizeBytes: string; // decimal u64
  metadataConfidence: MetadataConfidence;
  recoverabilityScore: number | null;
  pathState: PathState;
  warnings: string[];
}

interface CandidatePage {
  schemaVersion: 1;
  scanId: string;
  cursor: string | null;
  nextCursor: string | null;
  candidates: CandidateRow[]; // no máximo 100 na API de produção
}
```

Arquivos têm score inteiro de 0 a 100. Diretórios obrigatoriamente têm
`recoverabilityScore = null`. Score não é garantia de recuperação.

O cursor é opaco e pertence a um único `scanId`. Cursor estrangeiro, duplicado
ou desconhecido falha fechado. A primeira página tem `cursor = null`.
`nextCursor = null` encerra a paginação. A UI acrescenta a nova página sem
apagar as anteriores e rejeita IDs repetidos.

## 7. Números, IDs, texto e limites

- cada `u64` usa string decimal canônica entre `0` e
  `18446744073709551615`;
- IDs opacos usam somente `[A-Za-z0-9_-]` e têm de 1 a 128 caracteres;
- `requestId` tem de 1 a 128 escalares e não pode conter controles;
- texto de DTO tem no máximo 512 code points;
- `displayPath` que exceda esse limite deve ser elidido explicitamente e
  terminar com uma referência opaca curta derivada do ID do candidato; dois
  candidatos com o mesmo prefixo longo não podem ficar visualmente
  indistinguíveis por truncamento silencioso;
- listas de warning têm no máximo 128 itens;
- controles C0/C1 e formatação bidi são rejeitados ou removidos;
- campos desconhecidos fazem o parser TypeScript falhar fechado;
- IDs de disco, volume e candidato não podem se repetir dentro da mesma
  coleção;
- estado nativo retém no máximo 32 escopos de pasta e 4 scans.

O estado excedente é evicto; não existe persistência silenciosa de path ou
resultado.

A tabela usa o rótulo neutro “Candidatos encontrados” e mostra, antes das
linhas, que estado, confiança e score estimam apenas a qualidade dos metadados:
eles não comprovam conteúdo íntegro ou recuperável.

## 8. Erros estáveis

| Código | Significado público |
| --- | --- |
| `DESKTOP_RUNTIME_UNAVAILABLE` | O app não está no runtime Tauri. |
| `INVENTORY_UNAVAILABLE` | O inventário nativo não pôde ser obtido. |
| `SOURCE_UNSUPPORTED` | O volume não atende à política de scan. |
| `SOURCE_GONE` | A origem não está mais disponível. |
| `SOURCE_IDENTITY_CHANGED` | A identidade observada mudou. |
| `FOLDER_SCOPE_UNSUPPORTED` | O volume não permite escopo verificável. |
| `FOLDER_SCOPE_MISMATCH` | O escopo não pertence ao volume/identidade atual. |
| `UAC_CANCELLED` | A elevação foi cancelada. |
| `BROKER_UNAVAILABLE` | O broker não iniciou ou conectou corretamente. |
| `BROKER_PROTOCOL` | O protocolo falhou fechado. |
| `SOURCE_IO` | A leitura da origem falhou. |
| `SCAN_CORRUPT` | Estrutura reconhecida está corrompida. |
| `SCAN_INTERNAL` | O worker/coordenador não concluiu. |
| `REPORT_INCOMPATIBLE` | O DTO não satisfaz este contrato. |

O frontend converte qualquer erro desconhecido em `SCAN_INTERNAL`. Mensagens
não incluem path, GUID, pipe, handle, raw OS error, offset, bytes ou backtrace.

## 9. Estados reais da interface

- runtime indisponível;
- inventário carregando, atualizando, vazio, pronto ou com erro;
- volume selecionado ou nenhum;
- pasta selecionando, cancelada, vinculada ou removida;
- scan idle, executando, concluído ou com erro;
- página idle, carregando ou com erro;
- resultado `complete`, `partial` ou `unrecognized`;
- ancestralidade desconhecida explicitamente contada.

Não há progressão por timer, percentual inventado, resultado de fallback nem
linha sintética.

## 10. Acessibilidade e apresentação segura

Cada volume é um radio com label acessível dentro do grupo lógico. A tabela
de candidatos tem caption e região nomeada/focalizável. Paths recuperados usam
isolamento direcional e são renderizados como texto. Tema, reduced motion,
focus visível e navegação por heading continuam efetivos.

Validação nativa no Tauri real, zoom de 200%, forced colors e tecnologia
assistiva permanecem gates pendentes.
