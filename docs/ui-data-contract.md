# Contrato de dados do desktop real-only

Status: `Implemented-unverified`
Normative source: [SDD-017](specs/017-real-only-image-desktop.md)
Architecture decision: [ADR-0021](adr/0021-real-only-image-desktop.md)
Windows locality decision:
[ADR-0022](adr/0022-windows-locality-boundary-and-path-identity.md)

## 1. Fronteira de runtime

O frontend possui exatamente uma integração de produção com o host:

```text
React
  -> invoke("select_and_scan_image", { requestId })
  -> Rust abre o seletor nativo
  -> Rust mantém o caminho local fora do IPC
  -> spawn_blocking
  -> um_cli::scan_image_path
  -> io-common valida raiz local e ancestrais
  -> io-windows classifica a raiz com GetDriveTypeW
  -> FileImageReader abre somente leitura
  -> DesktopScanReport | null
```

`null` significa que o usuário cancelou o seletor. Não é erro e não cria uma
origem, sessão ou resultado.

Quando `isTauri()` é falso, a tela de análise mostra
`DESKTOP_RUNTIME_UNAVAILABLE`, não chama `invoke` e não apresenta dados
substitutos. Configurações de idioma, tema e movimento reduzido continuam
funcionando localmente.

A CSP de produção permanece restrita a recursos locais e IPC do Tauri. Apenas
no desenvolvimento, `devCsp` acrescenta `ws://localhost:1420` a
`connect-src`, para o live reload do Vite. Um transform exclusivo do servidor
de desenvolvimento injeta exatamente a mesma origem na meta CSP do HTML
servido, mantendo as duas políticas aplicadas em acordo. A configuração e o
HTML de build de produção não contêm WebSocket.

## 2. Comando permitido

### `select_and_scan_image`

Entrada:

```ts
interface SelectAndScanImageRequest {
  requestId: string; // ^[A-Za-z0-9_-]{1,128}$
}
```

Saída:

```ts
type SelectAndScanImageResponse = DesktopScanReport | null;
```

O comando não recebe caminho. O seletor nativo é aberto dentro do processo Rust
e contém apenas os filtros `img`, `dd`, `raw` e `bin`. O filtro é conveniência:
`um_cli::scan_image_path` repete a validação de arquivo regular, extensão,
namespace, nome reservado, tamanho e origem local antes da leitura.
`io-windows` usa somente `GetDriveTypeW` para classificar a raiz da unidade;
raiz remota, desconhecida ou não suportada falha fechada. `io-common` inspeciona
cada ancestral sem seguir links, rejeita symlink/reparse, abre o componente
final somente leitura sem seguir um reparse final e valida os metadados do
handle aberto.

Essa sequência não é uma garantia race-free de identidade. Picker,
classificação da unidade, inspeção dos ancestrais e abertura são operações
separadas por pathname; o contrato atual não retém handles dos ancestrais nem
vincula a seleção do picker a uma identidade estável. Uma mudança concorrente
de namespace permanece risco residual documentado, não uma propriedade
silenciosamente considerada resolvida.

Nenhum comando de disco físico, shell, filesystem genérico, restore, prévia,
sessão, carving, pausa ou cancelamento é registrado.

## 3. Relatório

```ts
interface DesktopScanReport {
  schemaVersion: 2;
  source: {
    label: string;        // basename sanitizado, sem controles bidi
    sizeBytes: UIntText;
  };
  partitionTable: "mbr" | "gpt" | "none";
  volumes: DesktopVolume[]; // no máximo 1.024
  warnings: string[];
  warningCount: UIntText;
  warningsOmitted: UIntText;
}

interface DesktopVolume {
  index: number; // u32
  offsetBytes: UIntText;
  lengthBytes: UIntText;
  fileSystem: "ntfs" | "fat12" | "fat16" | "fat32" | "unrecognized";
  scanStatus: "complete" | "partial" | "unrecognized";
  candidateCount: UIntText;
  warnings: string[];
}

type UIntText = string; // /^(0|[1-9][0-9]*)$/
```

Tamanhos, offsets, comprimentos e contagens atravessam IPC como strings decimais
canônicas. O frontend valida e usa `BigInt`; não converte esses valores por
`Number`.

`scanStatus` é obrigatório e segue combinações fechadas:

- `complete` somente com `ntfs`, `fat12`, `fat16` ou `fat32`;
- `partial` somente com `ntfs`, `fat12`, `fat16` ou `fat32`;
- `unrecognized` somente com `fileSystem: "unrecognized"`.

NTFS usa `partial` quando um limite conhecido impede enumeração exaustiva da
MFT: limite de trabalho por bytes/records, prefixo físico confiável menor que o
stream declarado, record não vazio sem assinatura `FILE`, record
ilegível/rompido/corrompido ignorado, atributos malformados em record comum ou
de extensão, qualquer `$ATTRIBUTE_LIST` cuja resolução completa de referências
e atributos definidores do candidato não esteja comprovada, ou falha ao mesclar
extension record. O scanner atual, portanto, mantém `partial` até mesmo para um
`$ATTRIBUTE_LIST` residente e bem formado.
`$MFT.data_size` não alinhado ao tamanho do record, ou alinhado mas cobrindo
menos que os 16 slots de records reservados, é corrupção fatal e não produz
relatório parcial. Atributos malformados no record 0 também são fatais porque
invalidam o bootstrap da MFT.

FAT usa `partial` quando qualquer cópia secundária declarada (o BPB permite até
quatro cópias no total) diverge da primeira ou está ilegível; a travessia de
diretório alcançável encontra cadeia quebrada, ciclo, cluster inicial
inutilizável ou falha de leitura; ou os limites `MAX_DIRS` (10.000 diretórios)
`MAX_DIR_BYTES` (8 MiB por stream de diretório), `MAX_DIR_DEPTH` (256 níveis)
ou o limite de clusters da cadeia interrompem a enumeração. Esse último limite
é derivado de 8 MiB divididos pelo tamanho do cluster e é aplicado antes da
leitura dos clusters do diretório. A primeira cópia continua autoritativa. Uma
tabela FAT declarada acima de 64 MiB é rejeitada antes da alocação; falha fatal
ao ler a primeira FAT/raiz continua sendo erro tipado, não relatório parcial.

Em qualquer volume parcial, `candidateCount` é uma contagem observada na
cobertura analisada, não um total. A tela exibe o status por volume e um aviso
localizado de cobertura. `FAT-COMPLETENESS-001` a
`FAT-COMPLETENESS-004`, `FAT-TABLE-BOUND-001`,
`FAT-DEPTH-BOUND-001`, `FAT-DIRECTORY-CHAIN-BOUND-001`,
`NTFS-MFT-SIZE-001`, `NTFS-RECORD-SIGNATURE-001`,
`NTFS-ATTRIBUTE-BOUNDS-001`, `NTFS-ATTRIBUTE-LIST-PARTIAL-001`,
`CLI-IMAGE-FAT-PARTIAL-001`,
`DESKTOP-SCAN-STATUS-001` e `DESKTOP-FAT-PARTIAL-001` cobrem pontos focados
dessa propagação e dos limites fatais relacionados. Em particular,
`DESKTOP-SCAN-STATUS-001` preserva status parcial de NTFS e FAT no adapter Rust.
Os gates locais do código congelado passaram; validações nativa, de pacote e
remota continuam pendentes.

Rótulos e warnings são texto não confiável. O adapter Rust remove caracteres de
controle e controles de formatação bidirecional Unicode antes do IPC. React
renderiza text nodes, e o título da origem usa `<bdi dir="auto">`. Isso impede
reordenação por controles bidi conhecidos, mas não transforma o rótulo exibido
em identidade autoritativa nem elimina caracteres Unicode visualmente
confundíveis.

O adapter Rust limita o relatório a:

- 1.024 volumes;
- 512 warnings retornados;
- 512 caracteres Unicode por warning;
- 255 caracteres Unicode no rótulo;
- 1 MiB no payload serializado.

`warningCount` registra a quantidade original e `warningsOmitted` informa a
parte não transportada. Estruturas desconhecidas, schema incompatível ou
payload excessivo falham de forma fechada.

Paridade do relatório não é inferida apenas pelo schema. Testes distintos
comparam a saída direta do CLI com o adapter Tauri para FAT sem partição,
MBR/NTFS e GPT/NTFS determinísticos e verificam que o SHA-256 da imagem não
muda. Esses testes passaram no código congelado; a execução empacotada no shell
Tauri nativo continua pendente.

Na descoberta GPT, header e tabela de entries de cada cópia são validados como
uma unidade. As duas cópias canônicas são avaliadas mesmo quando a primária é
utilizável. O backup só é lido no último LBA lógico, deve apontar de volta ao
LBA 1 e manter a tabela na área reservada. Se os dois headers forem válidos,
devem concordar em ponteiros recíprocos, intervalo utilizável, GUID do disco,
geometria e CRC da tabela; conflito falha fechado. Falha de leitura, header ou
tabela primária permite usar somente um backup canônico independentemente
válido. Se as duas cópias forem inutilizáveis, uma entry MBR protetora `0xEE`
não é exposta como volume.

## 4. Erros

O backend retorna somente códigos estáveis e uma mensagem genérica. O frontend
ignora a mensagem recebida e localiza pelo código:

| Código | Significado |
| --- | --- |
| `DESKTOP_RUNTIME_UNAVAILABLE` | O frontend não está no runtime Tauri. |
| `SOURCE_FORBIDDEN` | Namespace ou origem proibida pela política. |
| `SOURCE_UNSUPPORTED` | Tipo/extensão não suportado. |
| `SOURCE_NOT_REGULAR` | A origem não é arquivo local regular. |
| `SOURCE_EMPTY` | A imagem tem tamanho zero. |
| `SOURCE_IO` | A imagem não pôde ser aberta somente leitura. |
| `SCAN_CORRUPT` | Estrutura inválida ou região fora dos limites. |
| `SCAN_REPORT_TOO_LARGE` | O relatório excedeu limite estrutural/de transporte. |
| `SCAN_INTERNAL` | Falha interna sanitizada. |
| `REPORT_INCOMPATIBLE` | O frontend rejeitou o DTO recebido. |

O caminho completo, conteúdo, backtrace e erro bruto do sistema operacional não
fazem parte de sucesso ou erro. Falhas de leitura da tabela de partição também
omitem offset e detalhes brutos e usam código genérico.

Fallback de parser ocorre somente com `NotRecognized`: a imagem inteira só é
usada quando nenhuma tabela de partição é reconhecida, e FAT só é tentado
quando NTFS não reconhece o volume. Se um parser reconhecido retorna `Read`, o
desktop usa `SOURCE_IO`; se retorna `Corrupt`, usa `SCAN_CORRUPT`. Essas falhas
não são mascaradas como volume não reconhecido nem por tentativa de outro
filesystem. `$MFT.data_size` inválido e topologia GPT
canônica/recíproca inválida, atributos malformados no record 0 e tabela FAT
declarada acima de 64 MiB chegam como `SCAN_CORRUPT`, sem detalhe bruto.

## 5. Estados reais da interface

| Estado | Origem |
| --- | --- |
| Runtime indisponível | `isTauri() === false`; nenhum invoke. |
| Ocioso | Nenhuma análise concluída. |
| Seletor cancelado | Resposta `null`. |
| Em análise | Promise real do comando ainda pendente; progresso indeterminado. |
| Erro | Código estruturado real. |
| Relatório completo | DTO validado; todos os volumes reconhecidos estão `complete`. |
| Relatório parcial | DTO validado; ao menos um NTFS ou FAT reconhecido está `partial` e a tela mostra o aviso de cobertura. |

O frontend usa um guard síncrono contra duplo clique e uma geração monotônica
para descartar resposta após a tela ser substituída. Como o scanner ainda não
oferece cancelamento cooperativo nem fases observáveis, a interface não mostra
percentual, ETA, pausa, retomada ou sucesso de cancelamento.

## 6. Foco e cores forçadas

Ao trocar de tela principal, o frontend move o foco para o `h1` da nova tela.
Assim, a mudança de contexto não depende apenas de aparência. Em
`forced-colors: active`, controles e headings focáveis usam outline explícito
na cor de sistema `Highlight` e não dependem de box shadow.

A região que permite rolagem horizontal da tabela de volumes recebe foco pelo
teclado, `role="region"` e nome acessível localizado. A própria tabela possui
uma `caption` localizada e visualmente oculta com o mesmo nome, descrevendo seu
propósito para tecnologia assistiva.

Os testes focados `DESKTOP-NAVIGATION-FOCUS-001` e
`DESKTOP-FORCED-COLORS-FOCUS-001` cobrem foco e cores em componente/CSS;
`DESKTOP-VOLUME-TABLE-A11Y-001` cobre a região e a caption acessíveis.
Validação visual nativa, zoom de 200% e tecnologia assistiva continuam
pendentes.

## 7. Preferências

Somente preferências com efeito imediato são persistidas em
`undelete-master.preferences.v1`:

- `locale`: `pt-BR | en-US`;
- `theme`: `system | dark | light`;
- `reducedMotion`: boolean.

Nenhum caminho, relatório, warning ou conteúdo da origem é salvo.

## 8. Contratos futuros, ausentes do produto atual

Resultados individuais exigirão DTO versionado, identidade composta por volume
e record reference, paginação e limites. Restore exigirá executor separado,
sanitização de destino e prova de que nunca escreve na origem. Discos físicos
exigirão broker read-only auditado. Até essas especificações e implementações
existirem, não há rota, comando ou controle correspondente no aplicativo.
