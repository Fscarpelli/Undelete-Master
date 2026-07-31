export type Locale = "pt-BR" | "en-US";

const ptBR = {
  "app.name": "Undelete Master",
  "nav.analysis": "Análise",
  "nav.settings": "Configurações",
  "nav.help": "Ajuda",

  "analysis.title": "Unidades conectadas",
  "analysis.subtitle":
    "Escolha um volume local real para procurar metadados recuperáveis e, opcionalmente, JPEGs em regiões livres, sempre em modo somente leitura.",
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
    "O escopo cobre todo o volume em modo somente leitura. A técnica escolhida define se serão examinados apenas metadados ou também regiões NTFS livres para JPEG; escolher uma pasta mantém somente o modo de metadados.",
  "analysis.scope.folderBody":
    "A análise atual examina os metadados do sistema de arquivos do volume; os resultados são filtrados pela identidade comprovada da pasta.",
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
  "analysis.mode.title": "Técnica de análise",
  "analysis.mode.metadata.title": "Metadados (padrão)",
  "analysis.mode.metadata.body":
    "Examina estruturas reais do sistema de arquivos. É a opção mais rápida, mas não procura assinaturas no espaço livre; um resultado zero não prova que nada possa ser recuperado.",
  "analysis.mode.deepJpeg.title": "Profunda para JPEG",
  "analysis.mode.deepJpeg.body":
    "Além dos metadados, examina regiões NTFS comprovadamente livres em busca de JPEGs estruturalmente válidos. Pode levar bastante tempo e aplica limites seguros; nesta versão funciona apenas no volume inteiro e somente para JPEG.",
  "analysis.mode.folderBlocked":
    "A análise profunda para JPEG exige o volume inteiro. Remova o filtro de pasta para habilitá-la.",
  "analysis.mode.ntfsBlocked":
    "A análise profunda para JPEG está disponível somente para volumes NTFS.",
  "analysis.scan.start": "Analisar volume selecionado",
  "analysis.scan.pendingTitle": "Análise em andamento",
  "analysis.scan.pendingBody":
    "O mecanismo está lendo o volume e validando metadados reais. Esta versão não oferece cancelamento, percentual ou previsão; mantenha o aplicativo aberto até a conclusão.",
  "analysis.scan.pendingDeepBody":
    "O mecanismo está validando metadados e examinando regiões NTFS comprovadamente livres por assinaturas JPEG. Esta análise pode demorar bastante; não há percentual, previsão nem cancelamento nesta versão.",
  "analysis.scan.errorTitle": "Não foi possível concluir a análise",

  "analysis.results.readOnly": "Origem validada · somente leitura",
  "analysis.results.newScan": "Voltar aos volumes",
  "analysis.results.scope": "Escopo",
  "analysis.results.fileSystem": "Sistema de arquivos",
  "analysis.results.mode": "Técnica",
  "analysis.results.total": "Total observado",
  "analysis.results.matched": "Candidatos no escopo",
  "analysis.results.unknown": "Ancestralidade desconhecida",
  "analysis.results.mftRecordsExamined": "Registros MFT examinados",
  "analysis.results.jpegBytesExamined": "Bytes JPEG examinados",
  "analysis.results.jpegCoverageTitle": "Cobertura profunda limitada",
  "analysis.results.jpegCoverageBody":
    "A busca profunda verifica JPEGs contíguos e estruturalmente válidos somente nas regiões NTFS comprovadamente livres e dentro dos limites informados. Ela não cobre outros formatos, fragmentação arbitrária, bytes sobrescritos, criptografados ou descartados por TRIM.",
  "analysis.results.jpegCoverageStatus": "Estado da cobertura JPEG",
  "analysis.results.jpegCoveragePartial": "Parcial",
  "analysis.results.jpegCoverageComplete": "Concluída no limite informado",
  "analysis.results.jpegRegions": "Regiões examinadas",
  "analysis.results.jpegSignaturesAttempted": "Tentativas de validação",
  "analysis.results.jpegValidationBytes": "Bytes lidos na validação",
  "analysis.results.jpegSignatureLimit": "Limite de tentativas",
  "analysis.results.jpegValidationLimit": "Limite de bytes de validação",
  "analysis.results.limitReached": "Atingido",
  "analysis.results.limitNotReached": "Não atingido",
  "analysis.results.jpegRejected": "Assinaturas rejeitadas",
  "analysis.results.jpegTruncated": "Assinaturas truncadas",
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
  "analysis.results.method": "Método",
  "analysis.results.size": "Tamanho",
  "analysis.results.state": "Estado",
  "analysis.results.confidence": "Confiança",
  "analysis.results.score": "Pontuação",
  "analysis.results.evidence": "Evidência de conteúdo",
  "analysis.results.validator": "Validador",
  "analysis.results.noContentEvidence":
    "Sem hash ou validador de conteúdo",
  "analysis.results.noCandidatesPartial":
    "Nenhum candidato por metadados foi encontrado na cobertura examinada. A análise foi parcial e não é exaustiva; isso não significa que não existam bytes recuperáveis.",
  "analysis.results.noCandidatesComplete":
    "Nenhum candidato por metadados foi encontrado na cobertura concluída. Isso não prova que não existam bytes recuperáveis por outras técnicas.",
  "analysis.results.noCandidatesDeepPartial":
    "Nenhum candidato por metadados ou JPEG estrutural foi encontrado na cobertura examinada. A análise profunda foi parcial e não é exaustiva; outros formatos, regiões ou técnicas ainda podem produzir resultados.",
  "analysis.results.noCandidatesDeepComplete":
    "Nenhum candidato por metadados ou JPEG estrutural foi encontrado na cobertura informada. Isso não prova que não existam bytes recuperáveis por outros formatos, fragmentação ou técnicas.",
  "analysis.results.loadMore": "Carregar mais",
  "analysis.results.loadingMore": "Carregando",
  "analysis.results.warnings": "Avisos do scanner",
  "analysis.results.noWarnings": "Nenhum aviso foi retornado pelo scanner.",

  "scope.volume": "Volume inteiro",
  "scope.folder": "Pasta verificada",
  "scanMode.metadata": "Metadados",
  "scanMode.deepJpeg": "Profunda para JPEG",
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
  "candidate.method.ntfsMetadata": "Metadados NTFS",
  "candidate.method.fatMetadata": "Metadados FAT",
  "candidate.method.exfatMetadata": "Metadados exFAT",
  "candidate.method.carving": "Carving de assinatura JPEG",
  "candidate.method.recycleBin": "Lixeira",
  "candidate.method.jpegCorroborated": "Corroborado por validação JPEG",
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

  "results.search.label": "Pesquisar resultados",
  "results.search.placeholder": "Nome, caminho ou extensão",
  "results.search.submit": "Pesquisar",
  "results.filters.title": "Filtros",
  "results.filters.extensions": "Extensões",
  "results.filters.extensionSearch": "Filtrar extensões",
  "results.filters.selectedExtensions": "Extensões selecionadas",
  "results.filters.removeExtension": "Remover extensão",
  "results.filters.noExtension": "Sem extensão",
  "results.filters.kinds": "Tipos",
  "results.filters.confidences": "Confiança dos metadados",
  "results.filters.methods": "Métodos",
  "results.filters.states": "Estados",
  "results.filters.eligibilities": "Possibilidade de recuperação",
  "results.filters.score": "Pontuação de recuperação",
  "results.filters.minScore": "Pontuação mínima",
  "results.filters.maxScore": "Pontuação máxima",
  "results.filters.selectedOnly": "Mostrar somente selecionados",
  "results.eligibility.complete": "Recuperação completa",
  "results.eligibility.bestEffort": "Recuperação parcial",
  "results.eligibility.ineligible": "Não recuperável",
  "results.summary.filtered": "Resultados filtrados",
  "results.summary.visible": "Visíveis nesta página",
  "results.summary.loading": "Carregando resultados reais",
  "results.summary.empty": "Nenhum resultado corresponde aos filtros atuais.",
  "results.summary.error": "Não foi possível carregar os resultados.",
  "results.summary.retry": "Tentar novamente",
  "results.table.extension": "Extensão",
  "results.table.eligibility": "Recuperação",
  "results.table.warnings": "Avisos",
  "results.selection.filtered": "Selecionar todos os resultados filtrados",
  "results.selection.row": "Selecionar este resultado",
  "results.selection.summary": "Resumo da seleção",
  "results.selection.files": "Arquivos",
  "results.selection.directories": "Pastas",
  "results.selection.bytes": "Bytes lógicos",
  "results.selection.bestEffort": "Itens parciais",
  "results.selection.conflicts": "Itens conflitantes",
  "results.selection.ineligible": "Itens não recuperáveis",
  "results.selection.clearMatching": "Limpar correspondentes",
  "results.selection.clearAll": "Limpar seleção",
  "results.selection.recover": "Recuperar selecionados",
  "results.selection.ineligibleBlocked":
    "Remova os itens não recuperáveis antes de continuar.",
  "results.selection.updating": "Atualizando seleção",
  "results.pagination.previous": "Página anterior",
  "results.pagination.next": "Próxima página",
  "results.pagination.page": "Página",

  "restore.dialog.title": "Recuperar itens selecionados",
  "restore.dialog.close": "Fechar recuperação",
  "restore.destination.selecting": "Aguardando a seleção do Windows",
  "restore.destination.title": "Destino seguro",
  "restore.destination.selected": "Destino autorizado",
  "restore.destination.volume": "Volume",
  "restore.destination.filesystem": "Sistema de arquivos",
  "restore.destination.freeBytes": "Espaço livre",
  "restore.destination.change": "Escolher outro destino",
  "restore.setup.title": "Escolha onde recuperar",
  "restore.setup.body":
    "Use uma pasta NTFS em outro disco físico. O destino é validado novamente antes de qualquer arquivo ser criado.",
  "restore.setup.review": "Revisar plano",
  "restore.policy.title": "Política para arquivos incompletos",
  "restore.policy.completeOnly": "Somente itens completos",
  "restore.policy.completeOnlyBody":
    "Remova da seleção todos os itens que exigem recuperação parcial ou escolha “Preencher lacunas com zeros e gerar mapa” para continuar.",
  "restore.policy.zeroFillAndMap": "Preencher lacunas com zeros e gerar mapa",
  "restore.policy.zeroFillAndMapBody":
    "Recupera o que foi lido, preenche intervalos indisponíveis com zeros e registra exatamente esses intervalos no arquivo lateral.",
  "restore.policy.consent":
    "Entendo que arquivos parciais podem não abrir ou representar o conteúdo original.",
  "restore.review.title": "Revise antes de iniciar",
  "restore.review.destination": "Destino",
  "restore.review.items": "Itens",
  "restore.review.files": "Arquivos",
  "restore.review.directories": "Pastas",
  "restore.review.bytes": "Bytes lógicos",
  "restore.review.bestEffort": "Itens parciais",
  "restore.review.collision": "Colisões",
  "restore.review.collisionRename":
    "Renomear sem substituir arquivos existentes",
  "restore.review.digest": "Identidade do plano",
  "restore.review.start": "Iniciar recuperação",
  "restore.progress.title": "Recuperação em andamento",
  "restore.progress.label": "Progresso da recuperação",
  "restore.progress.items": "Itens processados",
  "restore.progress.bytes": "Bytes gravados",
  "restore.progress.completed": "Concluídos",
  "restore.progress.failed": "Falharam",
  "restore.progress.cancelled": "Cancelados",
  "restore.progress.current": "Item atual",
  "restore.progress.cancelling": "Cancelamento solicitado",
  "restore.progress.cancel": "Cancelar recuperação",
  "restore.progress.warnings": "Avisos",
  "restore.finish.title": "Resultado da recuperação",
  "restore.finish.status": "Estado final",
  "restore.finish.completed": "Trabalho concluído",
  "restore.finish.failed": "Trabalho encerrado com falha",
  "restore.finish.cancelled": "Trabalho cancelado",
  "restore.finish.itemsCompleted": "Itens concluídos",
  "restore.finish.itemsFailed": "Itens com falha",
  "restore.finish.itemsCancelled": "Itens cancelados",
  "restore.finish.published": "Itens publicados",
  "restore.finish.partial": "Itens parciais",
  "restore.finish.manifestSha256": "SHA-256 do manifesto",
  "restore.finish.reconciliation": "Reconciliação do manifesto",
  "restore.finish.durable": "Concluído e persistido",
  "restore.finish.needsReconciliation": "Requer reconciliação",
  "restore.finish.openDestination": "Abrir pasta de destino",
  "restore.finish.done": "Concluir",
  "restore.error.title": "Não foi possível continuar a recuperação",
  "restore.error.retryDestination": "Escolher novamente o destino",
  "restore.trackingLost.title": "Resultado da recuperação desconhecido",
  "restore.trackingLost.body":
    "Não foi possível confirmar o resultado da recuperação. O último estado conhecido pode estar desatualizado. Fechar esta janela não cancela o trabalho nem confirma que ele terminou.",

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
    "Detecta volumes locais montados reais sem elevação, permite uma pasta opcional no modo de metadados, oferece busca profunda limitada para JPEG e apresenta resultados acionáveis com seleção, filtros, ordenação e paginação.",
  "help.safety.title": "Somente leitura, sempre",
  "help.safety.body":
    "A interface consulta o inventário sem elevação. Ao iniciar a análise, o broker solicitado pelo UAC apenas revalida a identidade escolhida e faz leituras limitadas; ele não grava, bloqueia, desmonta, formata ou executa o conteúdo encontrado.",
  "help.restore.title": "Recuperação segura",
  "help.restore.body":
    "Os itens selecionados podem ser recuperados somente para uma pasta NTFS em outro disco físico, depois que o destino for autorizado. O aplicativo renomeia colisões sem substituir arquivos existentes; itens incompletos exigem consentimento explícito para preencher lacunas com zeros e gerar o mapa lateral.",
  "help.unavailable.title": "Limites atuais",
  "help.unavailable.body":
    "A recuperação não grava na origem e não restaura no caminho original nem no mesmo disco físico. O aplicativo nunca abre, pré-visualiza ou executa conteúdo recuperado e ainda não retoma trabalhos depois de fechado nem aceita destinos bloqueados, remotos ou de identidade incerta.",
  "help.interpretation.title": "Pasta e ancestralidade",
  "help.interpretation.body":
    "A pasta é um filtro aplicado após a análise de metadados do volume. Correspondências têm ancestralidade comprovada; candidatos desconhecidos ficam separados e nunca são apresentados como pertencentes à pasta.",

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
  "error.SCAN_MODE_UNSUPPORTED":
    "A técnica escolhida não é compatível com este sistema de arquivos ou escopo.",
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
  "error.RESULT_QUERY_INVALID":
    "Os filtros ou a ordenação não puderam ser aplicados com segurança.",
  "error.RESULT_CURSOR_STALE":
    "Esta página expirou porque os resultados mudaram. A consulta foi reiniciada.",
  "error.RESULT_SELECTION_STALE":
    "A seleção mudou em outra operação. Revise os itens antes de continuar.",
  "error.RESULT_SELECTION_INVALID":
    "A seleção recebida não corresponde aos resultados desta análise.",
  "error.RESTORE_DESTINATION_INVALID":
    "Escolha uma pasta NTFS local, gravável e autorizada em outro disco físico.",
  "error.RESTORE_DESTINATION_LIMIT":
    "Há seleções de destino demais em andamento. Aguarde e tente novamente.",
  "error.RESTORE_DESTINATION_EXPIRED":
    "A autorização do destino expirou. Escolha a pasta novamente.",
  "error.RESTORE_DIFFERENT_DISK_REQUIRED":
    "O destino precisa estar em um disco físico diferente da origem.",
  "error.RESTORE_SOURCE_CHANGED":
    "A identidade da origem mudou. Faça uma nova análise antes de recuperar.",
  "error.RESTORE_SELECTION_STALE":
    "A seleção mudou depois que o plano foi criado. Revise e gere outro plano.",
  "error.RESTORE_SELECTION_EMPTY":
    "Selecione ao menos um item recuperável.",
  "error.RESTORE_ITEM_INELIGIBLE":
    "A seleção contém itens que não podem ser recuperados.",
  "error.RESTORE_PARTIAL_POLICY_REQUIRED":
    "Confirme explicitamente a política para arquivos incompletos.",
  "error.RESTORE_PLAN_INVALID":
    "O plano de recuperação não é válido para esta análise.",
  "error.RESTORE_PLAN_LIMIT":
    "Há planos de recuperação demais em andamento. Aguarde e tente novamente.",
  "error.RESTORE_PLAN_EXPIRED":
    "O plano expirou. Revise a seleção e crie outro plano.",
  "error.RESTORE_JOB_LIMIT":
    "Já há um trabalho de recuperação ativo. Aguarde sua conclusão.",
  "error.RESTORE_JOB_NOT_FOUND":
    "O trabalho de recuperação não está mais disponível.",
  "error.RESTORE_JOB_NOT_COMPLETE":
    "A pasta só pode ser aberta depois que o trabalho terminar.",
  "error.RESTORE_INTERNAL":
    "Uma falha interna impediu confirmar ou concluir a operação solicitada.",
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
    "Choose a real local volume to search recoverable metadata and, optionally, JPEGs in free regions, always in read-only mode.",
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
    "The scope covers the whole volume in read-only mode. The selected technique determines whether only metadata or also free NTFS regions for JPEG are examined; choosing a folder keeps metadata mode only.",
  "analysis.scope.folderBody":
    "The current scan examines the volume's file-system metadata; results are filtered by the folder's proven identity.",
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
  "analysis.mode.title": "Scan technique",
  "analysis.mode.metadata.title": "Metadata (default)",
  "analysis.mode.metadata.body":
    "Examines real file-system structures. This is the faster option, but it does not search free-space signatures; a zero result does not prove that nothing is recoverable.",
  "analysis.mode.deepJpeg.title": "Deep JPEG",
  "analysis.mode.deepJpeg.body":
    "In addition to metadata, examines proven-free NTFS regions for structurally valid JPEGs. It can take substantially longer and applies safe bounds; this version supports only a whole NTFS volume and JPEG content.",
  "analysis.mode.folderBlocked":
    "Deep JPEG requires the whole volume. Remove the folder filter to enable it.",
  "analysis.mode.ntfsBlocked":
    "Deep JPEG is available only for NTFS volumes.",
  "analysis.scan.start": "Scan selected volume",
  "analysis.scan.pendingTitle": "Scan in progress",
  "analysis.scan.pendingBody":
    "The engine is reading the volume and validating real metadata. This version has no cancellation, percentage, or ETA; keep the application open until it completes.",
  "analysis.scan.pendingDeepBody":
    "The engine is validating metadata and examining proven-free NTFS regions for JPEG signatures. This scan can take substantially longer; this version has no percentage, ETA, or cancellation.",
  "analysis.scan.errorTitle": "The scan could not be completed",

  "analysis.results.readOnly": "Validated source · read-only",
  "analysis.results.newScan": "Back to volumes",
  "analysis.results.scope": "Scope",
  "analysis.results.fileSystem": "File system",
  "analysis.results.mode": "Technique",
  "analysis.results.total": "Total observed",
  "analysis.results.matched": "Candidates in scope",
  "analysis.results.unknown": "Unknown ancestry",
  "analysis.results.mftRecordsExamined": "MFT records examined",
  "analysis.results.jpegBytesExamined": "JPEG bytes examined",
  "analysis.results.jpegCoverageTitle": "Bounded deep coverage",
  "analysis.results.jpegCoverageBody":
    "The deep scan validates contiguous, structurally valid JPEGs only in proven-free NTFS regions and within the reported bounds. It does not cover other formats, arbitrary fragmentation, overwritten or encrypted bytes, or data discarded by TRIM.",
  "analysis.results.jpegCoverageStatus": "JPEG coverage status",
  "analysis.results.jpegCoveragePartial": "Partial",
  "analysis.results.jpegCoverageComplete": "Completed within the reported bound",
  "analysis.results.jpegRegions": "Regions examined",
  "analysis.results.jpegSignaturesAttempted": "Validation attempts",
  "analysis.results.jpegValidationBytes": "Validation bytes read",
  "analysis.results.jpegSignatureLimit": "Attempt limit",
  "analysis.results.jpegValidationLimit": "Validation-byte limit",
  "analysis.results.limitReached": "Reached",
  "analysis.results.limitNotReached": "Not reached",
  "analysis.results.jpegRejected": "Rejected signatures",
  "analysis.results.jpegTruncated": "Truncated signatures",
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
  "analysis.results.method": "Method",
  "analysis.results.size": "Size",
  "analysis.results.state": "State",
  "analysis.results.confidence": "Confidence",
  "analysis.results.score": "Score",
  "analysis.results.evidence": "Content evidence",
  "analysis.results.validator": "Validator",
  "analysis.results.noContentEvidence": "No content hash or validator",
  "analysis.results.noCandidatesPartial":
    "No metadata candidate was found in the examined coverage. The scan was partial and is not exhaustive; this does not mean that no recoverable bytes exist.",
  "analysis.results.noCandidatesComplete":
    "No metadata candidate was found in the completed coverage. This does not prove that no bytes are recoverable through other techniques.",
  "analysis.results.noCandidatesDeepPartial":
    "No metadata or structurally valid JPEG candidate was found in the examined coverage. The deep scan was partial and is not exhaustive; other formats, regions, or techniques may still produce results.",
  "analysis.results.noCandidatesDeepComplete":
    "No metadata or structurally valid JPEG candidate was found in the reported coverage. This does not prove that no bytes are recoverable through other formats, fragmentation, or techniques.",
  "analysis.results.loadMore": "Load more",
  "analysis.results.loadingMore": "Loading",
  "analysis.results.warnings": "Scanner warnings",
  "analysis.results.noWarnings": "The scanner returned no warnings.",

  "scope.volume": "Whole volume",
  "scope.folder": "Verified folder",
  "scanMode.metadata": "Metadata",
  "scanMode.deepJpeg": "Deep JPEG",
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
  "candidate.method.ntfsMetadata": "NTFS metadata",
  "candidate.method.fatMetadata": "FAT metadata",
  "candidate.method.exfatMetadata": "exFAT metadata",
  "candidate.method.carving": "JPEG signature carving",
  "candidate.method.recycleBin": "Recycle Bin",
  "candidate.method.jpegCorroborated": "Corroborated by JPEG validation",
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

  "results.search.label": "Search results",
  "results.search.placeholder": "Name, path, or extension",
  "results.search.submit": "Search",
  "results.filters.title": "Filters",
  "results.filters.extensions": "Extensions",
  "results.filters.extensionSearch": "Filter extensions",
  "results.filters.selectedExtensions": "Selected extensions",
  "results.filters.removeExtension": "Remove extension",
  "results.filters.noExtension": "No extension",
  "results.filters.kinds": "Kinds",
  "results.filters.confidences": "Metadata confidence",
  "results.filters.methods": "Methods",
  "results.filters.states": "States",
  "results.filters.eligibilities": "Recovery eligibility",
  "results.filters.score": "Recovery score",
  "results.filters.minScore": "Minimum score",
  "results.filters.maxScore": "Maximum score",
  "results.filters.selectedOnly": "Show selected only",
  "results.eligibility.complete": "Complete recovery",
  "results.eligibility.bestEffort": "Partial recovery",
  "results.eligibility.ineligible": "Ineligible",
  "results.summary.filtered": "Filtered results",
  "results.summary.visible": "Visible on this page",
  "results.summary.loading": "Loading real results",
  "results.summary.empty": "No result matches the current filters.",
  "results.summary.error": "Results could not be loaded.",
  "results.summary.retry": "Try again",
  "results.table.extension": "Extension",
  "results.table.eligibility": "Recovery",
  "results.table.warnings": "Warnings",
  "results.selection.filtered": "Select all filtered results",
  "results.selection.row": "Select this result",
  "results.selection.summary": "Selection summary",
  "results.selection.files": "Files",
  "results.selection.directories": "Folders",
  "results.selection.bytes": "Logical bytes",
  "results.selection.bestEffort": "Partial items",
  "results.selection.conflicts": "Conflicted items",
  "results.selection.ineligible": "Ineligible items",
  "results.selection.clearMatching": "Clear matching",
  "results.selection.clearAll": "Clear selection",
  "results.selection.recover": "Recover selected",
  "results.selection.ineligibleBlocked":
    "Remove ineligible items before continuing.",
  "results.selection.updating": "Updating selection",
  "results.pagination.previous": "Previous page",
  "results.pagination.next": "Next page",
  "results.pagination.page": "Page",

  "restore.dialog.title": "Recover selected items",
  "restore.dialog.close": "Close recovery",
  "restore.destination.selecting": "Waiting for the Windows selection",
  "restore.destination.title": "Safe destination",
  "restore.destination.selected": "Authorized destination",
  "restore.destination.volume": "Volume",
  "restore.destination.filesystem": "File system",
  "restore.destination.freeBytes": "Free space",
  "restore.destination.change": "Choose another destination",
  "restore.setup.title": "Choose where to recover",
  "restore.setup.body":
    "Use an NTFS folder on another physical disk. The destination is revalidated before any file is created.",
  "restore.setup.review": "Review plan",
  "restore.policy.title": "Incomplete-file policy",
  "restore.policy.completeOnly": "Complete items only",
  "restore.policy.completeOnlyBody":
    "Remove every best-effort item from the selection, or choose “Fill gaps with zeros and create a map” to continue.",
  "restore.policy.zeroFillAndMap": "Fill gaps with zeros and create a map",
  "restore.policy.zeroFillAndMapBody":
    "Recovers readable content, fills unavailable ranges with zeros, and records those exact ranges in the sidecar file.",
  "restore.policy.consent":
    "I understand that partial files might not open or represent the original content.",
  "restore.review.title": "Review before starting",
  "restore.review.destination": "Destination",
  "restore.review.items": "Items",
  "restore.review.files": "Files",
  "restore.review.directories": "Folders",
  "restore.review.bytes": "Logical bytes",
  "restore.review.bestEffort": "Partial items",
  "restore.review.collision": "Collisions",
  "restore.review.collisionRename":
    "Rename without replacing existing files",
  "restore.review.digest": "Plan identity",
  "restore.review.start": "Start recovery",
  "restore.progress.title": "Recovery in progress",
  "restore.progress.label": "Recovery progress",
  "restore.progress.items": "Items processed",
  "restore.progress.bytes": "Bytes written",
  "restore.progress.completed": "Completed",
  "restore.progress.failed": "Failed",
  "restore.progress.cancelled": "Cancelled",
  "restore.progress.current": "Current item",
  "restore.progress.cancelling": "Cancellation requested",
  "restore.progress.cancel": "Cancel recovery",
  "restore.progress.warnings": "Warnings",
  "restore.finish.title": "Recovery result",
  "restore.finish.status": "Final status",
  "restore.finish.completed": "Job completed",
  "restore.finish.failed": "Job ended with a failure",
  "restore.finish.cancelled": "Job cancelled",
  "restore.finish.itemsCompleted": "Completed items",
  "restore.finish.itemsFailed": "Failed items",
  "restore.finish.itemsCancelled": "Cancelled items",
  "restore.finish.published": "Published items",
  "restore.finish.partial": "Partial items",
  "restore.finish.manifestSha256": "Manifest SHA-256",
  "restore.finish.reconciliation": "Manifest reconciliation",
  "restore.finish.durable": "Completed and persisted",
  "restore.finish.needsReconciliation": "Needs reconciliation",
  "restore.finish.openDestination": "Open destination folder",
  "restore.finish.done": "Done",
  "restore.error.title": "Recovery could not continue",
  "restore.error.retryDestination": "Choose the destination again",
  "restore.trackingLost.title": "Recovery outcome unknown",
  "restore.trackingLost.body":
    "The recovery outcome could not be confirmed. The last known state may be stale. Closing this dialog does not cancel the job or confirm that it finished.",

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
    "Detects real mounted local volumes without elevation, accepts an optional folder in metadata mode, offers a bounded deep JPEG search, and presents actionable results with selection, filters, sorting, and pagination.",
  "help.safety.title": "Always read-only",
  "help.safety.body":
    "The interface queries inventory without elevation. When a scan starts, the UAC broker only revalidates the selected identity and performs bounded reads; it does not write, lock, dismount, format, or execute discovered content.",
  "help.restore.title": "Safe recovery",
  "help.restore.body":
    "Selected items can be recovered only to an authorized NTFS folder on another physical disk. The application renames collisions without replacing existing files; incomplete items require explicit consent to fill unavailable ranges with zeros and produce the sidecar map.",
  "help.unavailable.title": "Current limits",
  "help.unavailable.body":
    "Recovery never writes to the source and does not restore to the original path or the same physical disk. The application never opens, previews, or executes recovered content, cannot resume jobs after it closes, and rejects locked, remote, or uncertain-identity destinations.",
  "help.interpretation.title": "Folder scope and ancestry",
  "help.interpretation.body":
    "A folder is a filter applied after the volume metadata scan. Matches have proven ancestry; unknown candidates stay separate and are never presented as belonging to the folder.",

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
  "error.SCAN_MODE_UNSUPPORTED":
    "The selected technique is not compatible with this file system or scope.",
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
  "error.RESULT_QUERY_INVALID":
    "The filters or sort order could not be applied safely.",
  "error.RESULT_CURSOR_STALE":
    "This page expired because the results changed. The query was restarted.",
  "error.RESULT_SELECTION_STALE":
    "The selection changed in another operation. Review the items before continuing.",
  "error.RESULT_SELECTION_INVALID":
    "The received selection does not belong to these scan results.",
  "error.RESTORE_DESTINATION_INVALID":
    "Choose an authorized, writable local NTFS folder on another physical disk.",
  "error.RESTORE_DESTINATION_LIMIT":
    "Too many destination selections are in progress. Wait and try again.",
  "error.RESTORE_DESTINATION_EXPIRED":
    "The destination authorization expired. Choose the folder again.",
  "error.RESTORE_DIFFERENT_DISK_REQUIRED":
    "The destination must be on a different physical disk from the source.",
  "error.RESTORE_SOURCE_CHANGED":
    "The source identity changed. Run a new scan before recovering.",
  "error.RESTORE_SELECTION_STALE":
    "The selection changed after the plan was created. Review it and create another plan.",
  "error.RESTORE_SELECTION_EMPTY":
    "Select at least one eligible item.",
  "error.RESTORE_ITEM_INELIGIBLE":
    "The selection contains items that cannot be recovered.",
  "error.RESTORE_PARTIAL_POLICY_REQUIRED":
    "Explicitly confirm the incomplete-file policy.",
  "error.RESTORE_PLAN_INVALID":
    "The recovery plan is not valid for this scan.",
  "error.RESTORE_PLAN_LIMIT":
    "Too many recovery plans are in progress. Wait and try again.",
  "error.RESTORE_PLAN_EXPIRED":
    "The plan expired. Review the selection and create another plan.",
  "error.RESTORE_JOB_LIMIT":
    "A recovery job is already active. Wait for it to finish.",
  "error.RESTORE_JOB_NOT_FOUND":
    "The recovery job is no longer available.",
  "error.RESTORE_JOB_NOT_COMPLETE":
    "The folder can be opened only after the job finishes.",
  "error.RESTORE_INTERNAL":
    "An internal failure prevented the requested operation from being confirmed or completed.",
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
