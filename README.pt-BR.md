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

Documentação: [portal](docs/README.md) · [catálogo de funcionalidades](docs/FEATURES.md) ·
[galeria de capturas reais do desktop](docs/screenshots/README.md)

![Desktop real do Undelete Master detectando volumes conectados no Windows](docs/screenshots/01-connected-volumes.png)

*Executável empacotado atual, inventário real de volumes montados, sem dados
mock. Consulte a [proveniência das capturas](docs/screenshots/README.md).*

## Início rápido no Windows

### Pré-requisitos

- Windows 10 22H2 ou Windows 11 x64, com Microsoft Edge WebView2 disponível.
- Um volume local real, montado e em NTFS ou FAT12/16/32. NTFS é obrigatório
  para limitar a análise a uma pasta e para o destino da recuperação.
- Permissão para aprovar o UAC do Windows ao iniciar a análise. O desktop
  continua sem elevação; somente o broker fixo e apenas de leitura é elevado.
- Para recuperar arquivos, uma pasta NTFS local e gravável em exatamente um
  disco físico comprovadamente diferente de todos os discos da origem.

Antes da análise, pare de usar a origem tanto quanto for possível. Qualquer
gravação posterior do Windows ou de outro aplicativo pode sobrescrever o
conteúdo apagado, embora o Undelete Master abra a origem somente para leitura.

### Analisar e recuperar

1. Inicie `undelete-master-desktop.exe` com o irmão fixo
   `undelete-master-broker.exe` na mesma pasta. Não execute o desktop como
   Administrador.
2. Em **Análise**, atualize o inventário real, selecione o volume montado e, em
   NTFS, escolha opcionalmente uma pasta. O caminho permanece no código nativo
   e funciona como filtro de resultados com ancestralidade comprovada.
3. Use **Metadados** para a varredura do sistema de arquivos compatível ou
   **Profunda para JPEG** para a análise mais lenta do volume NTFS inteiro nas
   regiões que o bitmap comprova estarem livres.
4. Inicie a tarefa e aprove o UAC do broker. Durante a enumeração mensurável da
   MFT, a tela mostra contagens reais, percentual, tempo decorrido e estimativa
   baseada no ritmo observado. Fases sem total confiável permanecem claramente
   indeterminadas; o aplicativo não inventa progresso.
5. Pesquise, filtre e ordene os resultados limitados. Marque um candidato pelo
   checkbox da linha ou use a opção do cabeçalho/seleção de correspondências.
6. Confira o resumo e clique em **Recuperar selecionados**. Autorize uma pasta
   NTFS em outro disco físico, confirme eventual recuperação de melhor esforço
   e acompanhe o progresso nativo por itens/bytes até o manifesto final.

O desktop não possui seletor de arquivo-imagem por decisão de produto. A CLI
separada aceita arquivos-imagem locais comuns para fluxos headless e de
engenharia.

## Mapa de capacidades

| Área | Comportamento atual |
| --- | --- |
| Descoberta | Volumes locais reais e montados no Windows; inventário sem elevação |
| Metadados | NTFS e FAT12/16/32, com limites e evidência parcial explícita |
| Escopo de pasta | Opcional em NTFS; inclui somente candidatos de ancestralidade comprovada |
| Carving profundo | Somente JPEG contíguo estruturalmente válido, no volume NTFS inteiro e apenas em regiões que o `$Bitmap` comprova livres |
| Resultados | Busca nativa, filtros por extensão/evidência, ordenação estável, páginas por cursor e seleção individual/em massa |
| Recuperação | Planos de conteúdo elegíveis e evidência JPEG implementada; saída transacional preservando a árvore em outro disco físico NTFS |
| CLI headless | Análise somente leitura de arquivo-imagem local comum com JSON sanitizado |
| Sem suporte | exFAT, ReFS, carving de todos os formatos ou fragmentado, RAW de sistema danificado, partição desmontada e disco físico inteiro |

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
- Eventos nativos das fases e cronômetro decorrido. A enumeração da MFT
  fornece contagens concluído/total, percentual medido e ETA baseada no ritmo
  quando o total é confiável; reconstrução de namespace, classificação de
  candidatos e carving continuam indeterminados quando não há total seguro.
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
retomada após reiniciar, layouts alternativos de destino, pausar/retomar ou
cancelar cooperativamente a análise e instalador assinado também estão
ausentes. Fases nativas, tempo decorrido e percentual/ETA medidos da MFT estão
implementados; fases posteriores sem total seguro permanecem indeterminadas.
Trabalhos de restauração possuem, separadamente, progresso nativo por
itens/bytes e cancelamento cooperativo.
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

## Configurações e Ajuda no aplicativo

**Configurações** altera imediatamente o idioma (`pt-BR` ou `en-US`), o tema
(sistema, escuro ou claro) e a preferência de movimento reduzido. As escolhas
ficam no dispositivo quando o armazenamento local está disponível; a tela avisa
se elas valem somente para o processo atual. Essas opções não mudam a evidência
da análise ou recuperação.

**Ajuda** explica os modos de análise, a fronteira somente leitura/UAC, as
regras do destino, operações ausentes e a interpretação da ancestralidade de
pasta. Para evidência técnica e o estado exato dos requisitos, consulte o
[portal da documentação](docs/README.md) e a
[matriz de rastreabilidade](docs/traceability-matrix.md).

## Solução de problemas

| Sintoma ou código | Significado e ação segura |
| --- | --- |
| UAC cancelado / `UAC_CANCELLED` | Nenhuma origem foi aberta. Inicie de novo e aprove somente o broker irmão fixo, assinado ou compilado por você e confiável. Não execute o desktop como Administrador. |
| `SOURCE_GONE` | A revalidação nativa não encontra mais a identidade escolhida. Reconecte, espere o Windows montar, atualize a lista e selecione novamente. Isso não significa que a unidade tenha zero candidatos. |
| `SOURCE_IO` | O volume continuou identificado, mas uma leitura limitada ou consulta obrigatória somente leitura falhou. Confira cabo, case USB, estado da unidade no Windows e erros de leitura; tente novamente somente com a origem estável. Bridges USB antigos usam fallback restrito apenas quando o Windows declara a consulta de alinhamento não suportada; outros erros de I/O continuam falhando de forma fechada. |
| `SOURCE_IDENTITY_CHANGED` | A identidade montada mudou. Atualize e selecione conscientemente o volume outra vez. |
| `SCAN_INTERNAL` | O scanner real interrompeu por estado indisponível ou invariável interna e não fabricou candidatos. Atualize e tente uma vez; se repetir, guarde o código, modo, sistema de arquivos e avisos não sensíveis para o relato do bug. |
| Destino recusado | Escolha pasta NTFS local e gravável em exatamente um disco físico conhecido e diferente da origem. Outra letra não basta quando duas partições estão no mesmo disco; destinos de rede, virtuais, compostos ou incertos são recusados. |
| Antivírus colocou o `.exe` em quarentena | Builds locais atuais não são assinados e podem não ter reputação. Não desative a proteção permanentemente nem presuma falso positivo. Restaure/autorize somente artefato compilado por você ou verificado de modo independente, mantenha os dois executáveis irmãos juntos e prefira uma futura versão assinada. |

Um candidato ou pontuação alta é evidência, não promessa. Bytes
sobrescritos, descartados por TRIM, criptografados sem chave ou fisicamente
ilegíveis não podem ser recriados pelo aplicativo.

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

- [Portal da documentação](docs/README.md)
- [Catálogo completo de funcionalidades](docs/FEATURES.md)
- [Galeria de capturas reais do desktop](docs/screenshots/README.md) — uma
  captura atual empacotada e evidências históricas de defeitos bem identificadas
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

`main` é a única branch canônica publicada no GitHub. O histórico concluído
é integrado em `main` por fast-forward sempre que possível; nenhum README ou
captura deve direcionar o usuário para branch temporária.
