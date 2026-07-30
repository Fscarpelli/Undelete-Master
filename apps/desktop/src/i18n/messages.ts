export type Locale = "pt-BR" | "en-US";

const ptBR = {
  "app.name": "Undelete Master",
  "nav.analysis": "Análise",
  "nav.settings": "Configurações",
  "nav.help": "Ajuda",

  "analysis.title": "Unidades conectadas",
  "analysis.subtitle":
    "Escolha um volume local real para procurar metadados de arquivos recuperáveis em modo somente leitura.",
  "analysis.runtime.title": "Aplicativo desktop necessário",
  "analysis.runtime.body":
    "A detecção de unidades está disponível somente no aplicativo Tauri instalado. O navegador não acessa discos e nunca exibe inventário substituto.",
  "analysis.safety.title": "Inventário sem elevação e leitura protegida",
  "analysis.safety.body":
    "O aplicativo consulta as unidades sem elevação. Somente ao iniciar a análise o Windows pode solicitar UAC para o broker mínimo, que revalida a identidade escolhida e apenas lê a origem: ele nunca grava, bloqueia, desmonta, formata ou executa arquivos recuperados.",
  "analysis.inventory.loadingTitle": "Detectando unidades conectadas",
  "analysis.inventory.loadingBody":
    "Consultando o inventário real do Windows sem solicitar elevação.",
  "analysis.inventory.title": "Volumes montados detectados",
  "analysis.inventory.body":
    "Drive letters e nomes são apenas rótulos. A leitura usa identificadores opacos validados pelo processo nativo.",
  "analysis.inventory.refresh": "Atualizar unidades",
  "analysis.inventory.refreshing": "Atualizando unidades",
  "analysis.inventory.emptyTitle": "Nenhuma unidade compatível",
  "analysis.inventory.emptyBody":
    "Nenhum volume local montado e compatível foi encontrado.",
  "analysis.inventory.errorTitle": "Não foi possível detectar as unidades",
  "analysis.disk": "Grupo de armazenamento",
  "analysis.disk.noVolumes": "Nenhum volume montado foi retornado.",
  "analysis.volume.choose": "Escolha um volume para analisar",
  "analysis.volume.unnamed": "Volume sem nome",
  "analysis.volume.system": "Sistema",
  "analysis.volume.unsupported": "Não compatível",
  "analysis.volume.free": "livres",
  "analysis.scope.title": "Escopo da análise",
  "analysis.scope.wholeVolume":
    "O volume inteiro será lido em modo RAW. Escolher uma pasta é opcional e filtra os candidatos depois da análise.",
  "analysis.scope.folderBody":
    "O volume inteiro será lido em modo RAW; os resultados serão filtrados pela identidade comprovada da pasta.",
  "analysis.scope.folderLabel": "Pasta selecionada",
  "analysis.scope.chooseFolder": "Escolher pasta",
  "analysis.scope.changeFolder": "Trocar pasta",
  "analysis.scope.clearFolder": "Remover filtro de pasta",
  "analysis.scope.folderCancelled":
    "A seleção da pasta foi cancelada. O volume inteiro continua selecionado.",
  "analysis.scope.folderChangeCancelled":
    "A troca da pasta foi cancelada. O filtro anterior continua selecionado.",
  "analysis.scope.unsupported":
    "Este volume não oferece uma identidade de pasta segura; a análise continuará no volume inteiro.",
  "analysis.scan.start": "Analisar volume selecionado",
  "analysis.scan.pendingTitle": "Análise em andamento",
  "analysis.scan.pendingBody":
    "O mecanismo está lendo o volume e validando metadados reais. Esta versão não oferece cancelamento, percentual ou previsão; mantenha o aplicativo aberto até a conclusão.",
  "analysis.scan.errorTitle": "Não foi possível concluir a análise",

  "analysis.results.readOnly": "Origem validada · somente leitura",
  "analysis.results.newScan": "Voltar aos volumes",
  "analysis.results.scope": "Escopo",
  "analysis.results.fileSystem": "Sistema de arquivos",
  "analysis.results.total": "Total observado",
  "analysis.results.matched": "Correspondências verificadas",
  "analysis.results.unknown": "Ancestralidade desconhecida",
  "analysis.results.partialTitle": "Cobertura parcial",
  "analysis.results.partialBody":
    "O scanner encontrou uma limitação segura ou metadados mutáveis/corrompidos. O resultado não é apresentado como exaustivo.",
  "analysis.results.unknownTitle": "Ancestralidade desconhecida",
  "analysis.results.unknownBody":
    "Esses candidatos não foram incluídos como correspondências da pasta: a cadeia de diretórios não pôde ser provada como interna ou externa.",
  "analysis.results.candidates": "candidatos",
  "analysis.results.table": "Candidatos encontrados",
  "analysis.results.caveat":
    "Estado, confiança e pontuação estimam a qualidade dos metadados; não comprovam que o conteúdo esteja íntegro ou possa ser recuperado.",
  "analysis.results.loaded": "carregados",
  "analysis.results.path": "Caminho reconstruído",
  "analysis.results.kind": "Tipo",
  "analysis.results.size": "Tamanho",
  "analysis.results.state": "Estado",
  "analysis.results.confidence": "Confiança",
  "analysis.results.score": "Pontuação",
  "analysis.results.noCandidates":
    "Nenhum candidato foi retornado para este escopo.",
  "analysis.results.loadMore": "Carregar mais",
  "analysis.results.loadingMore": "Carregando",
  "analysis.results.warnings": "Avisos do scanner",
  "analysis.results.noWarnings": "Nenhum aviso foi retornado pelo scanner.",

  "scope.volume": "Volume inteiro",
  "scope.folder": "Pasta verificada",
  "filesystem.ntfs": "NTFS",
  "filesystem.fat12": "FAT12",
  "filesystem.fat16": "FAT16",
  "filesystem.fat32": "FAT32",
  "filesystem.unrecognized": "Não reconhecido",
  "bus.unknown": "Barramento desconhecido",
  "bus.ata": "ATA",
  "bus.sata": "SATA",
  "bus.scsi": "SCSI",
  "bus.usb": "USB",
  "bus.nvme": "NVMe",
  "bus.virtual": "Virtual",

  "candidate.kind.file": "Arquivo",
  "candidate.kind.directory": "Pasta",
  "candidate.state.exactEvidence": "Evidência exata",
  "candidate.state.likelyComplete": "Provavelmente completo",
  "candidate.state.completeUnvalidated": "Completo, não validado",
  "candidate.state.structurallyValid": "Estrutura válida",
  "candidate.state.partial": "Parcial",
  "candidate.state.conflicted": "Conflitante",
  "candidate.state.readError": "Erro de leitura",
  "candidate.state.zeroedOrTrimmed": "Zerado ou TRIM",
  "candidate.state.overwritten": "Sobrescrito",
  "candidate.state.metadataOnly": "Somente metadados",
  "candidate.state.unknown": "Desconhecido",
  "candidate.confidence.high": "Alta",
  "candidate.confidence.medium": "Média",
  "candidate.confidence.low": "Baixa",

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
    "Remove animações de transição e o movimento dos indicadores de atividade.",

  "help.title": "Ajuda e limites",
  "help.subtitle":
    "Como a análise de volumes funciona e quais limites protegem seus dados.",
  "help.available.title": "Disponível agora",
  "help.available.body":
    "Detecta volumes locais montados reais sem elevação, permite uma pasta opcional, analisa o volume em modo RAW e mostra candidatos reais em páginas limitadas.",
  "help.safety.title": "Somente leitura, sempre",
  "help.safety.body":
    "A interface consulta o inventário sem elevação. Ao iniciar a análise, o broker solicitado pelo UAC apenas revalida a identidade escolhida e faz leituras limitadas; ele não grava, bloqueia, desmonta, formata ou executa o conteúdo encontrado.",
  "help.unavailable.title": "Ainda não disponível",
  "help.unavailable.body":
    "Esta versão não restaura, abre ou pré-visualiza arquivos recuperados e não lida com volumes bloqueados, remotos ou layouts compostos cuja identidade não possa ser comprovada.",
  "help.interpretation.title": "Pasta e ancestralidade",
  "help.interpretation.body":
    "A pasta é um filtro aplicado após a leitura RAW. Correspondências têm ancestralidade comprovada; candidatos desconhecidos ficam separados e nunca são apresentados como pertencentes à pasta.",

  "error.DESKTOP_RUNTIME_UNAVAILABLE":
    "O mecanismo desktop não está disponível neste ambiente.",
  "error.INVENTORY_UNAVAILABLE":
    "O Windows não retornou um inventário de armazenamento compatível.",
  "error.SOURCE_UNSUPPORTED":
    "O volume selecionado não é compatível com o scanner atual.",
  "error.SOURCE_GONE":
    "A unidade foi removida durante a operação. Conecte-a e atualize a lista.",
  "error.SOURCE_IDENTITY_CHANGED":
    "A identidade da unidade mudou. Atualize a lista antes de tentar novamente.",
  "error.FOLDER_SCOPE_UNSUPPORTED":
    "Esta pasta não pode ser vinculada com segurança ao volume selecionado.",
  "error.FOLDER_SCOPE_MISMATCH":
    "A pasta escolhida não pertence ao volume selecionado.",
  "error.UAC_CANCELLED":
    "A autorização do Windows foi cancelada. Nenhuma unidade foi aberta.",
  "error.BROKER_UNAVAILABLE":
    "O serviço de leitura segura não pôde ser iniciado.",
  "error.BROKER_PROTOCOL":
    "O componente de leitura segura é incompatível com esta versão do aplicativo.",
  "error.SOURCE_IO":
    "O volume não pôde ser lido em modo somente leitura.",
  "error.SCAN_CORRUPT":
    "A estrutura do volume está danificada ou não pôde ser analisada com segurança.",
  "error.SCAN_INTERNAL":
    "O scanner encontrou uma falha interna. Nenhum resultado inventado foi exibido.",
  "error.REPORT_INCOMPATIBLE":
    "O aplicativo recebeu uma resposta incompatível e não exibiu dados potencialmente incorretos.",
} as const;

type MessageKey = keyof typeof ptBR;

const enUS: Record<MessageKey, string> = {
  "app.name": "Undelete Master",
  "nav.analysis": "Analysis",
  "nav.settings": "Settings",
  "nav.help": "Help",

  "analysis.title": "Connected storage",
  "analysis.subtitle":
    "Choose a real local volume and search its read-only metadata for recoverable files.",
  "analysis.runtime.title": "Desktop application required",
  "analysis.runtime.body":
    "Drive detection is available only in the installed Tauri application. The browser cannot access disks and never displays substitute inventory.",
  "analysis.safety.title": "Unelevated inventory and protected reads",
  "analysis.safety.body":
    "The application queries drives without elevation. Only when a scan starts may Windows request UAC for the minimal broker, which revalidates the selected identity and only reads the source: it never writes, locks, dismounts, formats, or executes recovered files.",
  "analysis.inventory.loadingTitle": "Detecting connected drives",
  "analysis.inventory.loadingBody":
    "Querying the real Windows inventory without requesting elevation.",
  "analysis.inventory.title": "Detected mounted volumes",
  "analysis.inventory.body":
    "Drive letters and names are display labels only. Reads use opaque identifiers validated by the native process.",
  "analysis.inventory.refresh": "Refresh drives",
  "analysis.inventory.refreshing": "Refreshing drives",
  "analysis.inventory.emptyTitle": "No compatible drives",
  "analysis.inventory.emptyBody": "No compatible mounted local volume was found.",
  "analysis.inventory.errorTitle": "Drives could not be detected",
  "analysis.disk": "Storage group",
  "analysis.disk.noVolumes": "No mounted volume was returned.",
  "analysis.volume.choose": "Choose a volume to scan",
  "analysis.volume.unnamed": "Unnamed volume",
  "analysis.volume.system": "System",
  "analysis.volume.unsupported": "Unsupported",
  "analysis.volume.free": "free",
  "analysis.scope.title": "Scan scope",
  "analysis.scope.wholeVolume":
    "The entire volume will be read as RAW. Choosing a folder is optional and filters candidates after the scan.",
  "analysis.scope.folderBody":
    "The entire volume will be read as RAW; results will be filtered by the folder's proven identity.",
  "analysis.scope.folderLabel": "Selected folder",
  "analysis.scope.chooseFolder": "Choose folder",
  "analysis.scope.changeFolder": "Change folder",
  "analysis.scope.clearFolder": "Remove folder filter",
  "analysis.scope.folderCancelled":
    "Folder selection was canceled. The whole volume remains selected.",
  "analysis.scope.folderChangeCancelled":
    "Folder change was canceled. The previous filter remains selected.",
  "analysis.scope.unsupported":
    "This volume does not expose a safe folder identity; scanning remains available for the whole volume.",
  "analysis.scan.start": "Scan selected volume",
  "analysis.scan.pendingTitle": "Scan in progress",
  "analysis.scan.pendingBody":
    "The engine is reading the volume and validating real metadata. This version has no cancellation, percentage, or ETA; keep the application open until it completes.",
  "analysis.scan.errorTitle": "The scan could not be completed",

  "analysis.results.readOnly": "Validated source · read-only",
  "analysis.results.newScan": "Back to volumes",
  "analysis.results.scope": "Scope",
  "analysis.results.fileSystem": "File system",
  "analysis.results.total": "Total observed",
  "analysis.results.matched": "Verified matches",
  "analysis.results.unknown": "Unknown ancestry",
  "analysis.results.partialTitle": "Partial coverage",
  "analysis.results.partialBody":
    "The scanner reached a safe bound or encountered mutable/corrupt metadata. This result is not presented as exhaustive.",
  "analysis.results.unknownTitle": "Unknown ancestry",
  "analysis.results.unknownBody":
    "These candidates were not included as folder matches because their directory chain could not be proven inside or outside the scope.",
  "analysis.results.candidates": "candidates",
  "analysis.results.table": "Discovered candidates",
  "analysis.results.caveat":
    "State, confidence, and score estimate metadata quality; they do not prove that file content is intact or recoverable.",
  "analysis.results.loaded": "loaded",
  "analysis.results.path": "Reconstructed path",
  "analysis.results.kind": "Kind",
  "analysis.results.size": "Size",
  "analysis.results.state": "State",
  "analysis.results.confidence": "Confidence",
  "analysis.results.score": "Score",
  "analysis.results.noCandidates":
    "No candidate was returned for this scope.",
  "analysis.results.loadMore": "Load more",
  "analysis.results.loadingMore": "Loading",
  "analysis.results.warnings": "Scanner warnings",
  "analysis.results.noWarnings": "The scanner returned no warnings.",

  "scope.volume": "Whole volume",
  "scope.folder": "Verified folder",
  "filesystem.ntfs": "NTFS",
  "filesystem.fat12": "FAT12",
  "filesystem.fat16": "FAT16",
  "filesystem.fat32": "FAT32",
  "filesystem.unrecognized": "Unrecognized",
  "bus.unknown": "Unknown bus",
  "bus.ata": "ATA",
  "bus.sata": "SATA",
  "bus.scsi": "SCSI",
  "bus.usb": "USB",
  "bus.nvme": "NVMe",
  "bus.virtual": "Virtual",

  "candidate.kind.file": "File",
  "candidate.kind.directory": "Folder",
  "candidate.state.exactEvidence": "Exact evidence",
  "candidate.state.likelyComplete": "Likely complete",
  "candidate.state.completeUnvalidated": "Complete, unvalidated",
  "candidate.state.structurallyValid": "Structurally valid",
  "candidate.state.partial": "Partial",
  "candidate.state.conflicted": "Conflicted",
  "candidate.state.readError": "Read error",
  "candidate.state.zeroedOrTrimmed": "Zeroed or trimmed",
  "candidate.state.overwritten": "Overwritten",
  "candidate.state.metadataOnly": "Metadata only",
  "candidate.state.unknown": "Unknown",
  "candidate.confidence.high": "High",
  "candidate.confidence.medium": "Medium",
  "candidate.confidence.low": "Low",

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
    "Removes transition animations and movement from activity indicators.",

  "help.title": "Help and limitations",
  "help.subtitle":
    "How volume scanning works and which boundaries protect your data.",
  "help.available.title": "Available now",
  "help.available.body":
    "Detects real mounted local volumes without elevation, accepts an optional folder, scans the volume as RAW, and shows real candidates in bounded pages.",
  "help.safety.title": "Always read-only",
  "help.safety.body":
    "The interface queries inventory without elevation. When a scan starts, the UAC broker only revalidates the selected identity and performs bounded reads; it does not write, lock, dismount, format, or execute discovered content.",
  "help.unavailable.title": "Not available yet",
  "help.unavailable.body":
    "This version does not restore, open, or preview recovered files, and does not handle locked, remote, or composite volumes whose identity cannot be proven.",
  "help.interpretation.title": "Folder scope and ancestry",
  "help.interpretation.body":
    "A folder is a post-RAW-scan filter. Matches have proven ancestry; unknown candidates stay separate and are never presented as belonging to the folder.",

  "error.DESKTOP_RUNTIME_UNAVAILABLE":
    "The desktop engine is not available in this environment.",
  "error.INVENTORY_UNAVAILABLE":
    "Windows did not return a compatible storage inventory.",
  "error.SOURCE_UNSUPPORTED":
    "The selected volume is not supported by the current scanner.",
  "error.SOURCE_GONE":
    "The drive was removed during the operation. Reconnect it and refresh.",
  "error.SOURCE_IDENTITY_CHANGED":
    "The drive identity changed. Refresh before trying again.",
  "error.FOLDER_SCOPE_UNSUPPORTED":
    "This folder cannot be safely bound to the selected volume.",
  "error.FOLDER_SCOPE_MISMATCH":
    "The selected folder does not belong to the selected volume.",
  "error.UAC_CANCELLED":
    "Windows authorization was canceled. No drive was opened.",
  "error.BROKER_UNAVAILABLE":
    "The secure read service could not be started.",
  "error.BROKER_PROTOCOL":
    "The secure read component is incompatible with this application version.",
  "error.SOURCE_IO": "The volume could not be read in read-only mode.",
  "error.SCAN_CORRUPT":
    "The volume structure is damaged or could not be analyzed safely.",
  "error.SCAN_INTERNAL":
    "The scanner encountered an internal failure. No invented result was shown.",
  "error.REPORT_INCOMPATIBLE":
    "The application received an incompatible response and did not display potentially incorrect data.",
};

const catalogs: Record<Locale, Record<MessageKey, string>> = {
  "pt-BR": ptBR,
  "en-US": enUS,
};

export function translate(locale: Locale, key: MessageKey): string {
  return catalogs[locale][key];
}

export type { MessageKey };
