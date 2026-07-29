export type Locale = "pt-BR" | "en-US";

const ptBR = {
  "app.name": "Undelete Master",
  "nav.analysis": "Análise",
  "nav.settings": "Configurações",
  "nav.help": "Ajuda",
  "analysis.title": "Análise de imagem",
  "analysis.subtitle":
    "Inspecione uma imagem local comum usando o mecanismo real e somente leitura.",
  "analysis.runtime.title": "Aplicativo desktop necessário",
  "analysis.runtime.body":
    "A análise só está disponível no aplicativo Tauri instalado. Esta visualização no navegador não acessa arquivos nem apresenta dados substitutos.",
  "analysis.idle.title": "Selecione uma imagem de disco",
  "analysis.idle.body":
    "Formatos aceitos: IMG, DD, RAW e BIN. O caminho permanece no processo Rust e a origem é aberta somente para leitura.",
  "analysis.select": "Selecionar e analisar imagem",
  "analysis.pending.title": "Análise em andamento",
  "analysis.pending.body":
    "O mecanismo está lendo a imagem. O progresso é indeterminado porque o scanner ainda não fornece fases ou percentual.",
  "analysis.cancelled": "A seleção foi cancelada. Nenhuma imagem foi aberta.",
  "analysis.error.title": "Não foi possível analisar a imagem",
  "analysis.tryAgain": "Tentar novamente",
  "analysis.report.new": "Analisar outra imagem",
  "analysis.report.readOnly": "Origem validada · somente leitura",
  "analysis.report.sourceSize": "Tamanho da origem",
  "analysis.report.partition": "Tabela de partição",
  "analysis.report.volumes": "Volumes",
  "analysis.report.volumeTable": "Tabela de volumes analisados",
  "analysis.report.candidates": "Metadados candidatos",
  "analysis.report.candidateCaveat":
    "A contagem indica metadados identificados pelo scanner. Ela não garante que os arquivos possam ser recuperados.",
  "analysis.report.partialCaveat":
    "A contagem é parcial: o scanner atingiu um limite seguro ou encontrou metadados ilegíveis. Consulte os avisos; nenhum candidato fora da região analisada está incluído.",
  "analysis.report.volume": "Volume",
  "analysis.report.fileSystem": "Sistema de arquivos",
  "analysis.report.coverage": "Cobertura",
  "analysis.report.offset": "Offset em bytes",
  "analysis.report.length": "Tamanho em bytes",
  "analysis.report.warnings": "Avisos do scanner",
  "analysis.report.noVolumes": "Nenhum volume reconhecido foi retornado.",
  "analysis.report.noWarnings": "Nenhum aviso foi retornado pelo scanner.",
  "analysis.report.omitted": "Avisos omitidos pelo limite de transporte",
  "partition.mbr": "MBR",
  "partition.gpt": "GPT",
  "partition.none": "Não reconhecida",
  "filesystem.ntfs": "NTFS",
  "filesystem.fat12": "FAT12",
  "filesystem.fat16": "FAT16",
  "filesystem.fat32": "FAT32",
  "filesystem.unrecognized": "Não reconhecido",
  "scanStatus.complete": "Completa",
  "scanStatus.partial": "Parcial",
  "scanStatus.unrecognized": "Não reconhecida",
  "settings.title": "Configurações",
  "settings.subtitle":
    "Estas preferências alteram o aplicativo imediatamente e ficam salvas apenas neste dispositivo.",
  "settings.persistenceUnavailable":
    "As alterações valem nesta sessão, mas não puderam ser salvas neste dispositivo.",
  "settings.language": "Idioma",
  "settings.language.pt": "Português (Brasil)",
  "settings.language.en": "English (United States)",
  "settings.theme": "Tema",
  "settings.theme.system": "Usar o sistema",
  "settings.theme.dark": "Escuro",
  "settings.theme.light": "Claro",
  "settings.motion": "Reduzir movimento",
  "settings.motion.help":
    "Remove animações de transição e o movimento do indicador de atividade.",
  "help.title": "Ajuda e limites",
  "help.subtitle":
    "O que esta versão realmente faz — e o que ainda não está disponível.",
  "help.available.title": "Disponível agora",
  "help.available.body":
    "Seleciona uma imagem local IMG, DD, RAW ou BIN, valida a origem no Rust e mostra o resumo real de partições, volumes, contagens de metadados candidatos e avisos.",
  "help.safety.title": "Limite de segurança",
  "help.safety.body":
    "A origem é aberta somente para leitura. O caminho completo não é enviado à interface nem persistido. Discos reais nunca são usados pelos testes automatizados.",
  "help.unavailable.title": "Ainda não disponível",
  "help.unavailable.body":
    "Esta versão não acessa discos físicos, não recupera ou restaura arquivos e não oferece visualização, sessões, carving, exFAT, pausa, cancelamento de uma análise em andamento ou percentual de progresso.",
  "help.interpretation.title": "Como interpretar o resultado",
  "help.interpretation.body":
    "Uma contagem de metadados candidatos confirma apenas que o parser encontrou registros compatíveis. Integridade e recuperabilidade exigem recursos adicionais que esta versão não afirma possuir.",
  "error.SOURCE_FORBIDDEN":
    "A origem selecionada é proibida. Dispositivos, caminhos de rede e caminhos especiais não são aceitos.",
  "error.SOURCE_UNSUPPORTED":
    "O formato não é aceito. Selecione um arquivo IMG, DD, RAW ou BIN.",
  "error.SOURCE_NOT_REGULAR":
    "A origem precisa ser um arquivo local comum, sem links ou redirecionamentos.",
  "error.SOURCE_EMPTY": "A imagem selecionada está vazia.",
  "error.SOURCE_IO":
    "Não foi possível abrir ou ler a imagem em modo somente leitura.",
  "error.SCAN_CORRUPT":
    "A estrutura da imagem está danificada ou não pôde ser analisada com segurança.",
  "error.SCAN_REPORT_TOO_LARGE":
    "O relatório excedeu o limite seguro de transporte e não foi exibido parcialmente.",
  "error.SCAN_INTERNAL":
    "O scanner encontrou uma falha interna. Nenhum resultado parcial foi exibido.",
  "error.REPORT_INCOMPATIBLE":
    "O aplicativo recebeu uma versão de relatório incompatível e não exibiu dados potencialmente incorretos.",
  "error.DESKTOP_RUNTIME_UNAVAILABLE":
    "O mecanismo desktop não está disponível neste ambiente.",
} as const;

type MessageKey = keyof typeof ptBR;

const enUS: Record<MessageKey, string> = {
  "app.name": "Undelete Master",
  "nav.analysis": "Analysis",
  "nav.settings": "Settings",
  "nav.help": "Help",
  "analysis.title": "Image analysis",
  "analysis.subtitle":
    "Inspect an ordinary local image with the real, read-only engine.",
  "analysis.runtime.title": "Desktop application required",
  "analysis.runtime.body":
    "Analysis is available only in the installed Tauri application. This browser view cannot access files and never presents substitute data.",
  "analysis.idle.title": "Select a disk image",
  "analysis.idle.body":
    "Accepted formats: IMG, DD, RAW, and BIN. The path stays in the Rust process and the source is opened read-only.",
  "analysis.select": "Select and analyze image",
  "analysis.pending.title": "Analysis in progress",
  "analysis.pending.body":
    "The engine is reading the image. Progress is indeterminate because the scanner does not yet report phases or a percentage.",
  "analysis.cancelled": "Selection was canceled. No image was opened.",
  "analysis.error.title": "The image could not be analyzed",
  "analysis.tryAgain": "Try again",
  "analysis.report.new": "Analyze another image",
  "analysis.report.readOnly": "Validated source · read-only",
  "analysis.report.sourceSize": "Source size",
  "analysis.report.partition": "Partition table",
  "analysis.report.volumes": "Volumes",
  "analysis.report.volumeTable": "Analyzed volumes table",
  "analysis.report.candidates": "Candidate metadata",
  "analysis.report.candidateCaveat":
    "The count represents metadata identified by the scanner. It does not guarantee that files can be recovered.",
  "analysis.report.partialCaveat":
    "This count is partial: the scanner reached a safety bound or encountered unreadable metadata. Review the warnings; candidates beyond the scanned region are not included.",
  "analysis.report.volume": "Volume",
  "analysis.report.fileSystem": "File system",
  "analysis.report.coverage": "Coverage",
  "analysis.report.offset": "Byte offset",
  "analysis.report.length": "Length in bytes",
  "analysis.report.warnings": "Scanner warnings",
  "analysis.report.noVolumes": "The scanner returned no recognized volumes.",
  "analysis.report.noWarnings": "The scanner returned no warnings.",
  "analysis.report.omitted": "Warnings omitted by the transport limit",
  "partition.mbr": "MBR",
  "partition.gpt": "GPT",
  "partition.none": "Not recognized",
  "filesystem.ntfs": "NTFS",
  "filesystem.fat12": "FAT12",
  "filesystem.fat16": "FAT16",
  "filesystem.fat32": "FAT32",
  "filesystem.unrecognized": "Unrecognized",
  "scanStatus.complete": "Complete",
  "scanStatus.partial": "Partial",
  "scanStatus.unrecognized": "Unrecognized",
  "settings.title": "Settings",
  "settings.subtitle":
    "These preferences change the application immediately and are stored only on this device.",
  "settings.persistenceUnavailable":
    "Changes apply for this session, but could not be saved on this device.",
  "settings.language": "Language",
  "settings.language.pt": "Português (Brasil)",
  "settings.language.en": "English (United States)",
  "settings.theme": "Theme",
  "settings.theme.system": "Use system setting",
  "settings.theme.dark": "Dark",
  "settings.theme.light": "Light",
  "settings.motion": "Reduce motion",
  "settings.motion.help":
    "Removes transition animations and movement from the activity indicator.",
  "help.title": "Help and limitations",
  "help.subtitle":
    "What this version actually does — and what is not available yet.",
  "help.available.title": "Available now",
  "help.available.body":
    "Selects a local IMG, DD, RAW, or BIN image, validates the source in Rust, and shows the real summary of partitions, volumes, candidate metadata counts, and warnings.",
  "help.safety.title": "Safety boundary",
  "help.safety.body":
    "The source is opened read-only. Its full path is not sent to the interface or persisted. Automated tests never use real disks.",
  "help.unavailable.title": "Not available yet",
  "help.unavailable.body":
    "This version does not access physical disks, recover or restore files, and does not provide preview, sessions, carving, exFAT, pause, in-progress scan cancellation, or progress percentages.",
  "help.interpretation.title": "Interpreting the result",
  "help.interpretation.body":
    "A candidate metadata count confirms only that the parser found compatible records. Integrity and recoverability require additional capabilities that this version does not claim.",
  "error.SOURCE_FORBIDDEN":
    "The selected source is forbidden. Devices, network paths, and special paths are not accepted.",
  "error.SOURCE_UNSUPPORTED":
    "That format is not accepted. Select an IMG, DD, RAW, or BIN file.",
  "error.SOURCE_NOT_REGULAR":
    "The source must be an ordinary local file without links or redirection.",
  "error.SOURCE_EMPTY": "The selected image is empty.",
  "error.SOURCE_IO": "The image could not be opened or read in read-only mode.",
  "error.SCAN_CORRUPT":
    "The image structure is damaged or could not be analyzed safely.",
  "error.SCAN_REPORT_TOO_LARGE":
    "The report exceeded the safe transport limit and was not shown partially.",
  "error.SCAN_INTERNAL":
    "The scanner encountered an internal failure. No partial result was shown.",
  "error.REPORT_INCOMPATIBLE":
    "The application received an incompatible report version and did not display potentially incorrect data.",
  "error.DESKTOP_RUNTIME_UNAVAILABLE":
    "The desktop engine is not available in this environment.",
};

const catalogs: Record<Locale, Record<MessageKey, string>> = {
  "pt-BR": ptBR,
  "en-US": enUS,
};

export function translate(locale: Locale, key: MessageKey): string {
  return catalogs[locale][key];
}

export type { MessageKey };
