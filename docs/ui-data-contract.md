# UI Data Contract — mapa campo → comando/payload

> Objetivo: permitir a conexão posterior do UI ao engine Rust **sem alterar as
> views**. Toda a fronteira UI ↔ engine passa pela interface `DataProvider`
> (`apps/desktop/src/api/provider.ts`); os tipos espelham as structs Rust e
> estão documentados campo a campo em `apps/desktop/src/api/types.ts`.
>
> Convenções:
> - Comandos Tauri em `snake_case`; payloads serializados com
>   `serde(rename_all = "camelCase")` para casar 1:1 com os tipos TS.
> - Eventos usam canais `scan://progress` e `restore://progress`.
> - O mock (`src/api/mock.ts`) implementa o mesmo contrato para desenvolvimento;
>   ele não é evidência de engine, Tauri, broker, sessão ou restauração reais.
>
> **Estado:** este é um contrato-alvo. Onde a tabela cita Tauri, Win32, broker,
> sessão ou restore, a implementação permanece futura até existir código e
> evidência na `docs/traceability-matrix.md`.

## 1. Tela Origens (`SourcesView.tsx`)

| Campo/ação no UI | Comando Tauri | Payload → Resposta | Tipo TS | Fonte no engine |
|---|---|---|---|---|
| Grade de dispositivos | `list_sources` | `{}` → `SourceInfo[]` | `SourceInfo` | Alvo futuro: broker Win32 read-only + `um_partition::discover`; hoje: mock explícito |
| Modelo/rótulo do disco | idem | `SourceInfo.label` | `string` | STORAGE_PROPERTY_QUERY (model) |
| Capacidade | idem | `SourceInfo.sizeBytes` | `number` | `SourceIdentity.size` |
| Bus (NVMe/SATA/USB/SD) | idem | `SourceInfo.bus` | `BusKind` | STORAGE_ADAPTER_DESCRIPTOR |
| HDD/SSD | idem | `SourceInfo.media` | `MediaKind` | DEVICE_SEEK_PENALTY / TRIM query |
| Nº do disco físico | idem | `SourceInfo.physicalNumber` | `number\|null` | índice PhysicalDrive |
| Badge "Disco do sistema" | idem | `SourceInfo.isSystemDisk` | `boolean` | comparação com volume do Windows |
| Badge removível | idem | `SourceInfo.isRemovable` | `boolean` | hotplug/removable flag |
| Saúde (SMART) | idem | `SourceInfo.health` | `"ok"\|"warning"\|"failing"\|null` | SMART overall status |
| Linhas de volume (letra, label, FS, tamanho, livre) | idem | `SourceInfo.volumes[]` | `VolumeInfo` | GetVolumeInformation + tabela de partições |
| Badge BitLocker | idem | `VolumeInfo.encryption` | `EncryptionState` | FVE status query |
| Botão "Abrir imagem de disco…" | `open_image_dialog` | `{}` → `SourceInfo \| null` | `SourceInfo` | diálogo nativo + `FileImageReader` |
| Botão "Atualizar" | `list_sources` (re-invoca) | — | — | — |
| Botão "Escanear" | (navegação local; `SourceInfo`/`VolumeInfo.id` vão para o wizard) | — | — | — |

## 2. Tela Novo escaneamento (`ScanSetupView.tsx`)

Campos coletados no `ScanRequest` (payload do `start_scan`):

| Campo do wizard | Campo do payload | Validação no UI | FR |
|---|---|---|---|
| Origem/volume (herdados da tela 1) | `sourceId`, `volumeId` | obrigatório | FR-020 |
| Cartões de modo (Rápido/Profundo/Imagem primeiro) | `mode: "quick"\|"deep"\|"imageFirst"` | radio único | FR-030/031/032 |
| Chips de prioridade de tipos | `priorityCategories: FileCategory[]` | opcional, multi; prioriza carving/validação, nunca limita metadata | FR-031 e §17.5 |
| Input pasta de trabalho | `workingFolder: string` | `validate_working_folder` (debounce 300 ms) | FR-021/022/023 |
| Checkbox de consentimento (passo 3) | `consentReadOnlyAcknowledged: boolean` | botão Iniciar desabilitado sem ele; engine **rejeita** se `false` | FR-001/002 |

Comandos:

| Ação | Comando | Payload → Resposta |
|---|---|---|
| Validação da pasta | `validate_working_folder` | `{ path, sourceId }` → `WorkingFolderValidation { ok, sameDiskAsSource, freeBytes, errorCode }` |
| Iniciar | `start_scan` | `ScanRequest` → `sessionId: string` |
| Botão "Escolher…" | plugin de diálogo do Tauri (folder picker) | → `string` (caminho) |

Regra dura: `sameDiskAsSource === true` bloqueia o avanço (banner vermelho,
mensagem `scan.working.sameDisk`).

## 3. Tela Escaneamento ativo (`LiveScanView.tsx`)

Fonte única: evento `scan://progress` (payload `ScanProgress`, ~2 Hz).

| Elemento do UI | Campo de `ScanProgress` |
|---|---|
| Anel de progresso (%) | `fraction` (indeterminado quando `null`) |
| Rótulo de fase | `phase` (i18n `live.phase.*`) |
| Card "Lidos" | `bytesRead` |
| Card "Velocidade" | `throughputBps` |
| Card "Candidatos" | `candidatesFound` |
| Chips de qualidade | `candidatesByLabel` (por `ScoreLabel`) |
| Card "Erros de leitura" (âmbar quando >0) | `readErrorCount` |
| Card "Decorrido" | `elapsedMs` |
| Card "Restante (estimado)" | `etaMs` (`null` → "calculando…") |

| Ação | Comando |
|---|---|
| Pausar / Retomar | `pause_scan` / `resume_scan` `{ sessionId }` |
| Cancelar (mantém parciais) | `cancel_scan` `{ sessionId }` |
| Ver resultados parciais | navegação local → Resultados |

## 4. Tela Resultados (`ResultsView.tsx`)

Grade virtualizada (janelas de ~250 linhas): `query_results` com `ResultsQuery`.

| Elemento do UI | Campo do payload/resposta |
|---|---|
| Busca por texto | `filter.text` |
| Chips de qualidade (com contagens) | `filter.labels` / `facets.byLabel` |
| Chips de categoria | `filter.categories` / `facets.byCategory` |
| Chips de extensão (top 12) | `filter.extensions` / `facets.byExtension` |
| Chips de método | `filter.methods` |
| Checkbox "Somente validados" | `filter.validatedOnly` |
| Checkbox "Incluir parciais" | `filter.includePartial` |
| Ordenação por cabeçalho | `sort: { key, dir }` |
| Janela do scroll | `offset` / `limit` → `ResultsPage { total, items, facets }` |

Colunas (FR-061) → campos de `CandidateView`: `name` (+`validated` ✓),
`parentPath`, `sizeBytes`, `modifiedMs`, `score` (badge + `recoverableFraction`
como "% recuperável"), `method`.

Painel de detalhes: `get_candidate { sessionId, candidateId }` → `CandidateDetail`:

| Seção do painel | Campo |
|---|---|
| Badges (nota, origem, método, confiança) | `score`, `origin`, `method`, `metadataConfidence` |
| "Por que esta nota?" | `score.factors[]` (códigos i18n `factor.*`) |
| "Como foi encontrado" | `evidence[]` (códigos i18n `evidence.*`) |
| Mapa físico + legenda | `extents[]` (`ExtentRunView.availability` → cores) |
| Localizador | `locator` (ex.: `mft:181075`) |
| Pré-visualização | `get_preview` → `PreviewData { kind, payload }` |

Seleção: `AppContext.selection` (persistente entre filtros, FR-065); barra
inferior (FR-066) mostra contagem/tamanho e leva à tela Restauração.

## 5. Tela Restauração (`RestoreView.tsx`)

Payload `RestoreRequest` para `start_restore`:

| Campo do UI | Campo do payload | Observação |
|---|---|---|
| Seleção (vinda de Resultados) | `candidateIds[]` | vazio → tela de estado vazio |
| Input destino | `destination` | `validate_destination` (debounce) |
| Radios estrutura | `structure: preserveTree\|flatten\|byType` | FR-080 |
| Radios conflitos | `conflicts: rename\|skip` | FR-083 |
| Radios parciais | `partials: skipPartial\|restorePartialMarked` | FR-085 |
| Frase digitada (só modo avançado) | `sameDiskOverrideConfirmed` | exige igualdade com `restore.destination.confirmPhrase` após o scan (FR-023, AC-020) |

| Elemento | Comando/evento |
|---|---|
| Validação (espaço/mesmo disco) | `validate_destination { sessionId, path, candidateIds }` → `DestinationValidation { ok, sameDiskAsSource, freeBytes, requiredBytes, errorCode }` |
| Progresso (barra, item atual, bytes) | evento `restore://progress` → `RestoreProgress` |
| Cancelar | `cancel_restore { restoreId }` |
| Relatório final (status por item, hash, lacunas) | `get_restore_report { restoreId }` → `RestoreReport` |
| "Abrir pasta" | `open_path { path }` (allow-list: apenas destinos de restauração) |

## 6. Tela Sessões (`SessionsView.tsx`)

| Elemento | Comando | Campos exibidos |
|---|---|---|
| Lista de sessões | `list_sessions` → `SessionInfo[]` | `sourceLabel`, `status` (badge), `mode`, `createdAtMs`, `candidatesFound`, `sessionBytes`, `checkpointAtMs` |
| Retomar | `resume_scan { sessionId }` + navega para tela 3 | status `resumable`/`paused` |
| Abrir resultados | (local) `setActiveSessionId` + navega para tela 4 | status `completed` |
| Exportar / Importar | `export_session` / `import_session` (diálogos nativos) | FR-092 |
| Excluir (com confirmação) | `delete_session { sessionId }` | apaga só dados de trabalho e explica limites de secure delete (FR-102) |

## 7. Tela Configurações (`SettingsView.tsx`)

Carga/persistência: `get_settings` / `set_settings` (objeto `AppSettings`
completo, gravação atômica). Campos:

| Controle | Campo | Valores |
|---|---|---|
| Dropdown idioma | `language` | `"pt-BR" \| "en-US"` |
| Dropdown tema | `theme` | `dark \| light \| system` |
| Switch reduzir animações | `reducedMotion` | boolean (aplica `data-reduced-motion` no `<html>`) |
| Switch redação de logs | `logRedaction` | boolean (FR-101, §22) |
| Dropdown perfil de recursos | `resourceProfile` | `economy \| balanced \| maximum \| gentle` (FR-043) |
| Switch modo avançado | `advancedMode` | boolean (FR-004; permite avaliar o override controlado de FR-023) |

## 8. Tela Ajuda e limites (`HelpView.tsx`)

Conteúdo estático localizado (mensagens honestas da §2). Sem comandos.

## Passos de wiring (quando `src-tauri` existir)

1. Criar `apps/desktop/src-tauri` com os comandos acima registrados no builder
   e handshake com o helper elevado (§ segurança da spec).
2. Implementar `TauriDataProvider` em `src/api/tauri.ts` usando
   `@tauri-apps/api` (`invoke` + `listen`), método a método conforme a
   documentação em `provider.ts`.
3. Selecionar o provider real apenas após detectar e validar o runtime Tauri;
   runtime desconhecido falha para indisponível, nunca para dados sintéticos.
4. Os testes de contrato (`src/api/mock.test.ts`) devem passar inalterados
   contra o provider real — eles codificam regras de FR, não detalhes do mock.
5. Substituir os botões "Escolher…" por pickers nativos (plugin dialog),
   pontos marcados com comentários `WIRING:` nas views.
