# UNDELETE MASTER
## Documento Mestre de SPEC-DRIVEN DEVELOPMENT e Prompt de Execução para o Codex

**Produto:** Undelete Master  
**Plataforma inicial:** Windows 10/11 x64, com Windows 11 como baseline principal  
**Documento:** Especificação mestre única, pronta para ser fornecida ao Codex  
**Data-base:** 29 de julho de 2026  
**Idioma do produto:** português do Brasil por padrão, com inglês dos Estados Unidos incluído  
**Idioma do código e da documentação técnica:** inglês; manter também `README.pt-BR.md` e textos de interface localizados  

---

# 0. COMO USAR ESTE DOCUMENTO

Este documento é, ao mesmo tempo:

1. a definição oficial do produto;
2. o prompt mestre de execução para o Codex;
3. a especificação funcional e não funcional;
4. o contrato de arquitetura, segurança e qualidade;
5. o plano de testes e validação;
6. a definição objetiva de “concluído”.

Forneça **este documento inteiro** ao Codex, na raiz do repositório, com o nome:

```text
UNDELETE_MASTER_CODEX_MASTER_SPEC.md
```

O Codex deverá lê-lo integralmente antes de alterar qualquer arquivo. Ele não deverá parar após criar documentação, protótipos ou telas estáticas: deverá continuar até entregar o repositório funcional, compilável, testado e documentado, respeitando todos os gates descritos aqui.

---

# 1. DIRETRIZ MESTRA AO CODEX

Você é o agente principal de engenharia responsável por projetar, implementar, testar, revisar, empacotar e documentar a primeira versão de produção do **Undelete Master**.

Sua missão é criar um aplicativo local para Windows capaz de localizar arquivos e pastas apagados em discos internos, externos e imagens de disco, avaliar honestamente a possibilidade de recuperação, permitir filtragem e seleção granular e restaurar somente os itens explicitamente escolhidos pelo usuário.

## 1.1 Regras de execução inegociáveis

1. **Leia este documento inteiro antes de implementar.**
2. **Inspecione primeiro o repositório e o ambiente.** Não presuma que ele está vazio, não substitua código útil sem análise e não invente ferramentas que não estejam disponíveis.
3. **Faça inventário de agents, subagents, skills, plugins e MCP servers realmente disponíveis.** Use todos os que forem úteis e confiáveis; não use recursos irrelevantes apenas para “marcar presença”.
4. **Delegue trabalho independente a subagents especializados**, preferencialmente em paralelo para pesquisa, revisão, testes e análise. Evite edições concorrentes nos mesmos arquivos. Quando houver escrita paralela, atribua worktrees, branches ou diretórios claramente não sobrepostos.
5. **Crie primeiro a documentação spec-driven exigida**, revise-a internamente, estabeleça uma matriz de rastreabilidade e depois implemente. Não pare ao final da documentação.
6. **Não faça perguntas ao usuário sobre decisões já resolvidas por esta especificação.** Na presença de detalhe secundário não definido, escolha a alternativa mais segura, simples, testável e coerente, registre a decisão em ADR e prossiga.
7. **Nunca declare o projeto concluído com TODOs, stubs, mocks em caminhos de produção, funções vazias, telas falsas ou testes ignorados sem justificativa aprovada.**
8. **Não prometa recuperação impossível.** Dados sobrescritos, criptografados sem a chave ou eliminados pelo comportamento do dispositivo não podem ser recriados por software.
9. **O scanner deve ser somente-leitura em relação à fonte.** Nenhuma rotina de varredura poderá emitir comandos de escrita, formatação, reparo, trim, exclusão, lock/dismount automático ou alteração de metadados na fonte.
10. **Nunca execute testes destrutivos em discos físicos reais.** Use somente imagens sintéticas, VHD/VHDX descartáveis rigorosamente identificados e ambientes de CI isolados.
11. **Nunca execute, importe, habilite macros ou abra de forma privilegiada um arquivo recuperado.** Arquivos recuperados são conteúdo não confiável.
12. **O aplicativo final deve funcionar localmente e não exigir nuvem, conta, login ou telemetria.**
13. **A interface principal não deve executar elevada.** Elevação via UAC ocorrerá apenas para um helper mínimo, separado e restrito à leitura bruta autorizada.
14. **Use versões estáveis atuais das dependências no momento da implementação e fixe lockfiles.** Não copie números de versões potencialmente obsoletos deste documento.
15. **Prefira dependências MIT, Apache-2.0 ou BSD.** Qualquer dependência copyleft, binário redistribuído ou codec de licença especial exige análise formal de licença e ADR antes de entrar no produto.
16. **Antes do status final, execute todos os builds, linters, testes, fuzz smoke tests, E2E, verificações de segurança e geração de instaladores previstos nesta especificação.**
17. **Entregue evidências.** O relatório final deve conter comandos executados, resultados, artefatos, hashes, cobertura relevante, limitações conhecidas e qualquer gate que não tenha sido atendido.
18. **Não confunda “nome encontrado” com “conteúdo recuperável”.** Todo resultado deve separar evidência de metadados, disponibilidade dos bytes, validação estrutural e confiança no caminho original.

## 1.2 Resultado final esperado

Ao terminar, o repositório deverá conter, no mínimo:

- aplicativo desktop completo;
- motor de leitura e recuperação em Rust;
- frontend Tauri + React + TypeScript;
- helper elevado somente-leitura;
- worker isolado para validação e preview;
- suporte de produção a NTFS, FAT12/16/32 e exFAT;
- modo de carving por assinaturas para espaço não alocado e fontes RAW;
- leitura de imagens RAW `.img` e `.dd`;
- arquitetura extensível para VHD/VHDX e outros sistemas de arquivos;
- filtros, seleção, preview seguro, restauração e relatórios;
- sessões retomáveis;
- documentação spec-driven completa;
- testes unitários, integração, propriedade, fuzz, E2E e acessibilidade;
- fixtures determinísticas e resultados por hash;
- instalador Windows e pacote portátil;
- SBOM, avisos de terceiros e relatório final de conclusão.

---

# 2. VERDADE TÉCNICA E PROMESSA DO PRODUTO

## 2.1 O que o produto pode prometer

O Undelete Master poderá:

- localizar registros apagados ainda presentes em estruturas do sistema de arquivos;
- reconstruir nomes, caminhos e extents quando houver evidência suficiente;
- copiar bytes que ainda permanecem acessíveis na mídia;
- identificar arquivos por assinatura quando os metadados já foram perdidos;
- estimar recuperabilidade com critérios transparentes;
- validar estruturalmente diversos formatos sem executá-los;
- recuperar arquivos completos ou parciais, deixando clara a diferença;
- gerar uma cópia derivada reparada somente quando o reparo for determinístico, rastreável e separado da extração original;
- restaurar somente os itens explicitamente selecionados.

## 2.2 O que o produto nunca deverá prometer

O Undelete Master não deverá afirmar que:

- recupera qualquer arquivo apagado independentemente do estado da mídia;
- recria bytes que já foram sobrescritos;
- reverte TRIM, garbage collection ou secure erase;
- quebra BitLocker, EFS ou qualquer criptografia;
- recupera genericamente qualquer formato por carving sem conhecer sua estrutura;
- garante que um arquivo “encontrado” abrirá;
- garante consistência perfeita ao escanear o volume de sistema enquanto o Windows continua escrevendo nele;
- substitui laboratório especializado quando há falha física, ruído mecânico, firmware danificado ou degradação severa.

## 2.3 Mensagem central ao usuário

Use linguagem honesta e simples:

> “Encontrar um registro apagado não significa que todo o conteúdo ainda exista. O Undelete Master analisa os bytes disponíveis, conflitos de alocação e a estrutura do arquivo para indicar a chance real de recuperação.”

Ao selecionar um disco, exibir também:

> “Pare de usar o disco de origem e salve os arquivos recuperados em outro disco físico. Novas gravações podem sobrescrever dados que ainda seriam recuperáveis.”

Para SSDs ou dispositivos com TRIM:

> “Em SSDs, o TRIM pode tornar setores apagados indisponíveis rapidamente. O tempo desde a exclusão, sozinho, não determina se o arquivo pode ser recuperado.”

---

# 3. VISÃO DO PRODUTO

## 3.1 Nome e posicionamento

**Nome:** Undelete Master  
**Tagline recomendada:** **Encontre. Avalie. Recupere.**  
**Proposta:** recuperação local de arquivos com interface atraente, operação segura e avaliação de integridade baseada em evidências.

## 3.2 Objetivos

- Tornar recuperação de dados compreensível para usuários comuns.
- Manter profundidade técnica suficiente para uso avançado e forense.
- Minimizar o risco de o próprio aplicativo destruir dados recuperáveis.
- Tratar resultados em grande escala sem travar a interface.
- Mostrar por que um arquivo é considerado excelente, parcial ou irrecuperável.
- Ser determinístico, testável, auditável e extensível.
- Preparar o núcleo para futuras implementações de macOS e outros ambientes sem fingir que os sistemas móveis oferecem o mesmo acesso bruto ao armazenamento.

## 3.3 Não objetivos da versão 1.0

- recuperação física de unidades defeituosas em laboratório;
- bypass de criptografia;
- reparo automático do sistema de arquivos de origem;
- kernel driver próprio;
- escrita bruta para “reativar” registros apagados;
- RAID distribuído, Storage Spaces complexo, LVM ou arrays proprietários, salvo quando expostos pelo Windows como volume legível normal;
- recuperação profunda de metadados apagados de ReFS sem implementação comprovada e fixtures suficientes;
- APFS, HFS+, ext4 ou sistemas móveis como suporte de produção;
- nuvem, backup remoto, login, assinatura ou telemetria obrigatória;
- execução de arquivos recuperados;
- promessa de suportar toda assinatura existente no mundo.

Sistemas de arquivos não suportados deverão ser detectados e oferecidos apenas em **modo carving/RAW**, com limitação explícita.

---

# 4. PERSONAS E PRINCIPAIS CENÁRIOS

## 4.1 Usuário doméstico

Apagou fotos, documentos ou vídeos e precisa de um fluxo guiado, sem jargão excessivo.

## 4.2 Profissional de TI

Precisa de filtros, logs, imagens de disco, retomada de sessão, hashes e detalhes técnicos.

## 4.3 Investigador ou auditor autorizado

Precisa de leitura somente, rastreabilidade, origem dos resultados, extents, hashes e exportação estruturada.

## 4.4 Cenários essenciais

1. selecionar HD interno, SSD, pendrive, cartão ou unidade USB;
2. selecionar uma partição específica ou todo o disco físico;
3. executar varredura rápida ou profunda;
4. criar imagem primeiro e escanear a imagem;
5. filtrar resultados por uma ou várias extensões encontradas;
6. selecionar arquivos por checkbox;
7. selecionar uma pasta apagada sem restaurar automaticamente todo o conteúdo histórico;
8. visualizar informações e preview seguro;
9. restaurar em uma pasta de recuperação;
10. restaurar no caminho original, aceitando conscientemente o risco;
11. retomar varredura após fechamento ou desconexão;
12. recuperar arquivo parcialmente e receber mapa das regiões ausentes;
13. trabalhar com milhões de candidatos sem carregar tudo na memória da UI.

---

# 5. ESCOPO DE PLATAFORMA

## 5.1 Sistemas operacionais

- alvo principal de release: Windows 11 x64;
- compatibilidade funcional: Windows 10 22H2 x64, desde que WebView2 e pré-requisitos estejam disponíveis;
- preparar compilação ARM64, sem torná-la gate obrigatório da primeira release;
- não depender de WSL para executar o aplicativo;
- builds e testes nativos no Windows.

## 5.2 Tipos de fonte

Suportar:

- discos físicos internos;
- discos físicos externos USB/Thunderbolt quando apresentados pelo Windows;
- volumes e partições montadas;
- mídia removível;
- imagens RAW `.img`, `.dd`, `.raw`;
- imagens segmentadas RAW como extensão posterior, se implementadas e testadas;
- VHD/VHDX em modo somente-leitura por adapter separado, caso a implementação e os testes estejam completos.

Detectar e explicar:

- disco do sistema;
- boot/system volume;
- HDD, SSD ou tipo desconhecido;
- capacidade de TRIM quando consultável;
- BitLocker desbloqueado, bloqueado ou indeterminado;
- disco básico, volume que atravessa múltiplos discos ou layout não suportado;
- MBR ou GPT;
- setor lógico e físico, incluindo 512n, 512e e 4Kn;
- sistema de arquivos;
- possível falha/erro de leitura.

## 5.3 Sistemas de arquivos

### Produção na versão 1.0

- NTFS;
- FAT12;
- FAT16;
- FAT32;
- exFAT.

### Suporte limitado

- ReFS: detecção e leitura/carving somente; qualquer recuperação de metadados apagados será marcada como experimental até existir cobertura ampla e fixtures independentes;
- sistema desconhecido ou corrompido: carving por assinatura e análise RAW;
- BitLocker: somente após o usuário desbloquear o volume por mecanismos normais do Windows; o aplicativo não solicitará, transmitirá ou armazenará a chave.

### Limitações relevantes

- EFS pode permitir recuperar bytes cifrados, mas o arquivo somente será utilizável se o certificado/chave privada do usuário ainda existir;
- imagem física de disco BitLocker permanece cifrada; leitura do volume já desbloqueado pode expor uma visão lógica decifrada, que deverá ser rotulada corretamente;
- mídia montada e ativa pode mudar durante a varredura.

---

# 6. TERMINOLOGIA E TAXONOMIA DE RESULTADOS

Use conceitos distintos no modelo de dados e na UI:

- **Source:** disco, volume ou imagem analisada.
- **Candidate:** item potencialmente recuperável.
- **Metadata evidence:** nome, caminho, tamanho, datas, ID, atributos e extents encontrados em metadados.
- **Content evidence:** bytes ainda legíveis e sua relação com o candidato.
- **Carved candidate:** item encontrado por assinatura, geralmente sem nome/caminho original confiável.
- **Extent:** faixa física/lógica de bytes associada ao conteúdo.
- **Conflict:** faixa que já está alocada a outro arquivo ativo ou a outro candidato incompatível.
- **Read error:** faixa que não pôde ser lida.
- **Missing range:** parte esperada do arquivo que não pôde ser reconstruída.
- **Structural validation:** confirmação de que o conteúdo obedece à estrutura conhecida do formato.
- **Metadata confidence:** confiança no nome, caminho, datas e tamanho.
- **Recoverability:** avaliação da disponibilidade e coerência do conteúdo.
- **Original extraction:** cópia byte a byte produzida a partir da evidência disponível.
- **Repaired derivative:** arquivo derivado, separado e documentado, que recebeu reparo determinístico.

Estados mínimos de candidato:

```text
exact-evidence
likely-complete
complete-unvalidated
structurally-valid
partial
conflicted
read-error
zeroed-or-trimmed
overwritten
metadata-only
unknown
```

Não use “100% recuperável” sem uma referência de hash pré-existente. Em dados reais, o máximo deverá ser apresentado como **“Excelente — forte evidência de conteúdo completo”**, não como certeza matemática de identidade.

---

# 7. REQUISITOS FUNCIONAIS

Todos os requisitos devem receber IDs estáveis na documentação gerada e aparecer na matriz de rastreabilidade.

## 7.1 Primeiro uso, autorização e segurança

### FR-001 — Consentimento e autorização

Antes da primeira varredura, o usuário deverá confirmar que tem autorização para analisar o dispositivo ou imagem.

### FR-002 — Aviso de preservação

Explicar que o uso continuado da fonte pode sobrescrever dados e recomendar desconexão/uso como unidade secundária.

### FR-003 — Sem escrita na fonte

A UI deverá mostrar um selo persistente **“Fonte aberta somente para leitura”** quando essa condição estiver verificada.

### FR-004 — Modo simples e avançado

- **Modo Guiado:** linguagem simples, escolhas recomendadas e poucos controles.
- **Modo Avançado:** seleção de partição, regiões, assinatura, tamanho de bloco, política de erros, exportações e detalhes forenses.

## 7.2 Inventário de dispositivos

### FR-010 — Enumerar fontes

Listar discos físicos, volumes e imagens adicionadas, com atualização automática em hot-plug.

### FR-011 — Cartões de dispositivo

Cada cartão deve mostrar:

- modelo e fabricante quando disponíveis;
- tipo de conexão;
- capacidade total;
- partições/volumes;
- letra e rótulo;
- sistema de arquivos;
- HDD/SSD/desconhecido;
- status BitLocker;
- sistema/boot;
- saúde/erros conhecidos;
- número do disco físico;
- aviso quando múltiplas letras pertencem ao mesmo disco físico.

### FR-012 — Identidade estável

Guardar identidade por combinação robusta de serial, unique ID, layout, volume GUID e tamanho, sem depender apenas da letra de unidade.

### FR-013 — Atualização e desconexão

Se a fonte desaparecer, pausar com segurança, preservar checkpoint e oferecer retomada após reconexão da mesma identidade.

## 7.3 Seleção e preparação da varredura

### FR-020 — Seleção do disco/volume

O usuário seleciona uma fonte antes do início. Para disco físico, pode escolher todas ou algumas partições reconhecidas.

### FR-021 — Pasta de trabalho

Antes de iniciar, selecionar ou confirmar onde serão gravados:

- banco da sessão;
- checkpoints;
- thumbnails;
- arquivos temporários;
- relatórios;
- arquivos recuperados, se já definido.

### FR-022 — Detectar disco físico de destino

Mapear a pasta de trabalho e o destino de restauração ao(s) disco(s) físico(s), não apenas à letra.

### FR-023 — Prevenção de sobrescrita

- bloquear por padrão pasta de trabalho no mesmo disco físico da fonte;
- bloquear restauração no mesmo disco enquanto a varredura estiver ativa;
- depois da varredura, permitir restauração no mesmo disco somente em modo avançado, com confirmação digitada e aviso de que outros arquivos podem ser destruídos;
- nunca ocultar esse risco.

### FR-024 — Disco do sistema

Se a fonte contiver o Windows ativo:

- mostrar alerta de que o sistema continua escrevendo;
- recomendar “Criar imagem primeiro” em outro disco;
- exigir destino em outro disco físico;
- marcar a sessão como `live-system-source`;
- não alegar consistência forense perfeita.

## 7.4 Modos de varredura

### FR-030 — Varredura rápida

Executar:

- descoberta da Lixeira do Windows, quando aplicável;
- análise de metadados apagados do sistema de arquivos;
- reconstrução de nomes, caminhos e extents;
- avaliação de disponibilidade por bitmap/alocação;
- validação rápida opcional por amostragem.

### FR-031 — Varredura profunda

Além da rápida:

- percorrer metadados brutos, inclusive registros não ativos;
- analisar espaço não alocado;
- executar carving por assinaturas;
- analisar slack relevante com limites;
- tentar reconstrução limitada de fragmentos;
- validar candidatos de maior confiança e os que o usuário abrir/selecionar;
- oferecer modo “volume inteiro” apenas quando filesystem estiver danificado ou o usuário ativar análise RAW avançada.

### FR-032 — Criar imagem primeiro

Criar imagem somente-leitura da fonte para outro disco:

- operação retomável;
- mapa de setores com erro;
- hash SHA-256 do fluxo lido e do arquivo final quando possível;
- manifesto de origem, geometria, horário e erros;
- leitura sequencial e política “gentle” para mídia instável;
- nunca escrever na fonte;
- depois, escanear a imagem.

### FR-033 — Abrir imagem existente

Permitir selecionar `.img`, `.dd` ou `.raw`, detectar partições e escanear sem UAC quando possível.

### FR-034 — Regiões de análise

No modo avançado, permitir selecionar:

- metadados apenas;
- espaço não alocado;
- slack;
- partição inteira;
- faixa de offset específica válida.

Validar todos os limites e alinhamentos.

## 7.5 Progresso e controle

### FR-040 — Dashboard em tempo real

Mostrar:

- fase atual;
- bytes processados e total estimado;
- throughput;
- candidatos encontrados;
- candidatos por qualidade;
- erros de leitura;
- tempo decorrido;
- ETA somente quando estatisticamente razoável;
- atividade atual sem expor dados sensíveis desnecessários.

### FR-041 — Pausar, retomar e cancelar

- pausa cooperativa e rápida;
- retomada sem repetir todo o trabalho;
- cancelamento seguro;
- persistência do checkpoint;
- confirmação antes de excluir uma sessão incompleta.

### FR-042 — Resultados durante a varredura

Permitir navegar resultados já persistidos enquanto a varredura continua, sem bloquear o pipeline.

### FR-043 — Limites e recursos

Permitir perfil de uso:

- Econômico;
- Balanceado;
- Máximo desempenho;
- Mídia instável/gentle.

## 7.6 Descoberta de arquivos e pastas

### FR-050 — Qualquer extensão via metadados

Se os metadados fornecerem extents, copiar qualquer sequência de bytes independentemente da extensão. Não usar whitelist de extensão para excluir candidatos.

### FR-051 — Carving baseado em formatos conhecidos

O carving deve usar plugins declarativos/compilados e deixar claro que formatos sem assinatura ou estrutura distinguível não podem ser encontrados genericamente sem metadados.

### FR-052 — Pastas apagadas

Mostrar pastas apagadas como itens navegáveis e recuperáveis. Selecionar uma pasta não deverá, por padrão, selecionar silenciosamente todos os seus antigos conteúdos.

### FR-053 — Seleção de descendentes

Oferecer ação explícita:

> “Selecionar descendentes recuperáveis encontrados”

Exibir contagem e impacto antes de aplicar.

### FR-054 — Orphans

Candidatos sem caminho completo deverão aparecer em agrupamento claro, como:

```text
Itens órfãos / MFT record 12345
Carved / JPEG / offset 0x...
```

### FR-055 — Mesclagem de evidências

Quando o mesmo conteúdo for detectado por metadados e carving, mesclar métodos/evidências em um candidato, evitando duplicidade. Se houver conflito real, manter variantes separadas e explicar.

## 7.7 Tabela de resultados

### FR-060 — Escala

A tabela deve ser virtualizada e consultar o backend com paginação/cursor, filtros e ordenação server-side. Não enviar milhões de linhas ao frontend.

### FR-061 — Colunas

Incluir, configuráveis:

- checkbox;
- ícone ou thumbnail;
- nome;
- caminho original;
- extensão declarada;
- tipo detectado por conteúdo;
- tamanho;
- data de exclusão estimada, quando possível;
- datas originais;
- método de descoberta;
- recuperabilidade;
- confiança de metadados;
- validação;
- regiões ausentes/conflitantes;
- observações.

### FR-062 — Filtro por extensão

Gerar dinamicamente a lista das extensões encontradas, com contagem. Permitir marcar uma ou várias extensões e combinar com outros filtros.

### FR-063 — Outros filtros

- categoria;
- qualidade;
- método de descoberta;
- tamanho;
- intervalo de data;
- caminho;
- validado/não validado;
- completo/parcial;
- possui preview;
- somente selecionados.

### FR-064 — Busca

Busca por nome, caminho, extensão, tipo, record ID e hash quando disponível.

### FR-065 — Seleção persistente

A seleção por checkbox deve persistir através de paginação, ordenação, filtros e retomada da sessão.

### FR-066 — Barra de seleção

Mostrar quantidade selecionada, tamanho estimado, tamanho legível, espaço necessário, quantidade parcial e conflitos.

### FR-067 — Árvore e lista

Alternar entre:

- lista plana;
- árvore por caminho original;
- agrupamento por tipo;
- agrupamento por recuperabilidade.

## 7.8 Detalhes, validação e preview

### FR-070 — Painel de detalhes

Mostrar:

- todas as evidências;
- extents;
- mapa visual de regiões disponíveis, conflitantes, zeroed e com erro;
- nome/caminho alternativos;
- assinatura encontrada;
- status do validator;
- cálculo explicável da recuperabilidade;
- hash, se calculado;
- conteúdo hexadecimal limitado e seguro.

### FR-071 — Preview seguro

Gerar preview em worker isolado. Nunca carregar diretamente no processo privilegiado.

Suportar, conforme formato e segurança:

- imagens rasterizadas/decodificadas com limites;
- texto com encoding detectado e limite;
- metadados de áudio/vídeo;
- propriedades de documentos;
- estrutura de arquivos compactados sem extração ilimitada;
- metadados PE de executáveis, sem executar;
- hex para desconhecidos.

### FR-072 — Sem execução

Não oferecer botão “Executar”. A ação de abrir arquivo restaurado no aplicativo associado deverá ficar fora do fluxo de preview, exigir restauração concluída e exibir alerta para executáveis, scripts, documentos com macro e arquivos potencialmente perigosos.

### FR-073 — Validação sob demanda e em lote

Permitir validar:

- item aberto;
- itens selecionados;
- candidatos acima de um limiar;
- todos, com aviso de custo.

### FR-074 — Reparos derivados

Quando houver reparo seguro:

- preservar a extração original;
- criar arquivo separado com sufixo apropriado;
- registrar transformação, plugin, versão e resultado;
- nunca alegar que bytes inventados pertenciam ao original;
- permitir desligar reparos.

## 7.9 Restauração

### FR-080 — Destinos

Oferecer:

1. **Pasta de recuperação recomendada**, em outro disco físico;
2. **Caminho original**, com salvaguardas;
3. para pasta de recuperação: preservar hierarquia original, achatar ou agrupar por tipo.

### FR-081 — Reconstrução mínima de pastas

Se o caminho original não existir, criar somente as pastas ancestrais necessárias aos itens selecionados. Não restaurar automaticamente outros arquivos que existiam historicamente naquela pasta.

Exemplo: se apenas `C:\Projetos\2024\Contrato.docx` foi selecionado e toda a árvore foi apagada, criar `C:\Projetos\2024` e restaurar somente `Contrato.docx`.

### FR-082 — Pasta selecionada isoladamente

Se o usuário selecionar somente uma pasta apagada, criar a pasta vazia. Conteúdos só serão restaurados quando explicitamente selecionados.

### FR-083 — Conflitos de nome

Políticas:

- renomear automaticamente — padrão;
- ignorar;
- substituir — nunca padrão e sempre confirmado;
- decidir individualmente.

Nunca substituir silenciosamente arquivo ativo.

### FR-084 — Operação transacional

Para cada arquivo:

1. criar nome temporário `.umrecovering` no destino;
2. copiar e mapear extents;
3. calcular hash;
4. flush e validação;
5. renomear atomicamente quando possível;
6. registrar manifesto;
7. limpar temporário somente quando seguro.

### FR-085 — Arquivos parciais

Permitir recuperar, com consentimento, usando uma das políticas:

- preservar comprimento e preencher gaps com zero;
- truncar no primeiro gap quando adequado;
- exportar segmentos separados;
- gerar sidecar `.um.json` com mapa de ranges.

A política deverá ser sugerida pelo validator do formato, não aplicada cegamente.

### FR-086 — Metadados de destino

Preservar, quando possível:

- nome;
- timestamps;
- atributos comuns;
- hierarquia.

ACLs, owner, EFS e alternate data streams devem ser opt-in e somente quando o destino suportar. Não restaurar ACL antiga por padrão, pois pode tornar o arquivo inacessível ou reintroduzir permissões inseguras.

### FR-087 — Manifesto de recuperação

Gerar JSON e opção CSV contendo:

- source/session ID;
- item ID;
- caminho original e final;
- método;
- extents;
- gaps/conflitos;
- score e explicação;
- validação;
- hashes;
- erros;
- derivação/reparo;
- horários.

### FR-088 — Retomada

Restaurações grandes devem ser retomáveis sem duplicar arquivos já concluídos e verificados.

## 7.10 Sessões e relatórios

### FR-090 — Sessão persistente

Salvar sessão em SQLite em disco diferente da fonte, com migrations e journal apropriado.

### FR-091 — Abrir sessão

Listar sessões recentes, status, fonte, último checkpoint e disponibilidade do dispositivo.

### FR-092 — Exportar/importar sessão

Formato `.umscan` versionado, contendo banco, manifesto e configurações, sem incluir conteúdo recuperado por padrão.

### FR-093 — Relatório final

Gerar relatório legível e exportável com:

- fonte;
- modo;
- duração;
- bytes lidos;
- erros;
- candidatos;
- qualidade;
- selecionados/restaurados;
- falhas;
- hashes;
- limitações.

## 7.11 Privacidade e preferências

### FR-100 — Operação local

Nenhum conteúdo, nome, path, hash ou telemetria sai do computador por padrão.

### FR-101 — Logs redigidos

Logs normais não devem conter conteúdo de arquivos. Paths completos e nomes poderão ser mascarados por preferência.

### FR-102 — Limpeza de sessão

Permitir apagar sessão e thumbnails. Explicar que “secure delete” não pode ser garantido em SSDs.

### FR-103 — Idiomas

- pt-BR padrão quando o Windows estiver em português;
- en-US completo;
- arquitetura i18n sem strings hard-coded.

---

# 8. ARQUITETURA DE ALTO NÍVEL

## 8.1 Stack obrigatória recomendada

Use:

- **Tauri 2** para desktop shell;
- **React** e **TypeScript strict** para UI;
- **Rust stable** para engine, parsers, IPC e helpers;
- **SQLite** para sessão/resultados;
- **Windows APIs oficiais** via crate `windows` ou bindings equivalentes auditáveis;
- toolchain Node com lockfile e package manager escolhido uma única vez;
- CSS tokens e componentes próprios/acessíveis, evitando dependência visual excessiva.

Justificativa: Rust oferece controle de baixo nível e segurança de memória; Tauri permite uma interface moderna com backend Rust e mantém caminho futuro para outras plataformas. O motor de recuperação não deverá depender do DOM nem da camada Tauri.

## 8.2 Separação de processos

### Processo A — `UndeleteMaster.exe`

- Tauri + React;
- executa sem elevação;
- coordena sessão e UI;
- não recebe handle bruto gravável;
- não analisa conteúdo hostil no processo principal.

### Processo B — `undelete-master-broker.exe`

- helper elevado sob demanda;
- manifesto `requireAdministrator` ou elevação por `runas`;
- abre fontes autorizadas somente com permissões de leitura;
- fornece leitura por offset via IPC;
- não contém comandos de restore;
- não aceita path arbitrário vindo da UI;
- encerra quando o app/Job Object termina.

### Processo C — `undelete-master-worker.exe`

- worker não privilegiado e restrito;
- valida, extrai metadados e gera previews;
- AppContainer ou restricted token/low integrity com Job Object;
- sem network capabilities;
- limites de CPU, memória, tempo, quantidade de arquivos e expansão;
- reiniciável após crash sem derrubar a sessão.

### Processo D — `undelete-master-restore-helper.exe` opcional

Somente se destinos protegidos exigirem elevação:

- separado do broker de leitura;
- escopo de path explicitamente aprovado;
- operações de arquivo comuns, nunca raw disk write;
- comandos allowlisted;
- sem acesso ao handle da fonte, exceto stream já fornecido de forma controlada.

## 8.3 Invariantes de arquitetura

- `SourceReader` é somente leitura.
- Nenhum crate de filesystem parser escreve na fonte.
- Restore recebe um stream lógico de candidato e um destino; nunca recebe autorização para mutar metadados da fonte.
- A UI nunca calcula diretamente offsets ou envia paths de device arbitrários.
- Todos os números de offset/tamanho usam inteiros adequados, aritmética checada e validação contra o tamanho da fonte.
- O scanner principal permanece funcional sem internet.
- Parsers são independentes da UI e testáveis sobre buffers/imagens.

## 8.4 Estrutura do repositório

Criar monorepo semelhante a:

```text
/
├─ AGENTS.md
├─ PLANS.md
├─ README.md
├─ README.pt-BR.md
├─ SECURITY.md
├─ PRIVACY.md
├─ CONTRIBUTING.md
├─ LICENSE
├─ THIRD_PARTY_NOTICES.md
├─ Cargo.toml
├─ Cargo.lock
├─ package.json
├─ <node-lockfile>
├─ apps/
│  └─ desktop/
│     ├─ src/
│     ├─ src-tauri/
│     └─ tests/
├─ crates/
│  ├─ core/
│  ├─ protocol/
│  ├─ io-common/
│  ├─ io-windows/
│  ├─ partition/
│  ├─ fs-common/
│  ├─ fs-ntfs/
│  ├─ fs-fat/
│  ├─ fs-exfat/
│  ├─ carving/
│  ├─ validation/
│  ├─ repair/
│  ├─ restore/
│  ├─ session-db/
│  ├─ imaging/
│  ├─ elevated-broker/
│  ├─ sandbox-worker/
│  ├─ restore-helper/
│  ├─ cli/
│  └─ fixture-builder/
├─ fixtures/
│  ├─ manifests/
│  ├─ small/
│  └─ fuzz-corpus/
├─ tests/
│  ├─ integration/
│  ├─ e2e/
│  ├─ performance/
│  └─ security/
├─ docs/
│  ├─ specs/
│  ├─ adr/
│  ├─ architecture/
│  ├─ threat-model/
│  ├─ testing/
│  ├─ evidence/
│  ├─ traceability-matrix.md
│  ├─ risk-register.md
│  ├─ recovery-limitations.md
│  └─ completion-report.md
├─ scripts/
├─ .agents/
│  └─ skills/
└─ .github/
   └─ workflows/
```

A estrutura pode ser ajustada por ADR, mas deve preservar separação clara de responsabilidades.

## 8.5 Interfaces centrais

Defina traits equivalentes a:

```rust
pub trait SourceReader: Send + Sync {
    fn identity(&self) -> &SourceIdentity;
    fn len(&self) -> u64;
    fn sector_layout(&self) -> SectorLayout;
    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError>;
    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome;
}

pub trait FileSystemScanner: Send + Sync {
    fn probe(&self, source: &dyn SourceReader, region: Region) -> Result<ProbeResult>;
    fn scan_metadata(&self, ctx: &ScanContext, sink: &dyn CandidateSink) -> Result<()>;
    fn allocation_map(&self, ctx: &ScanContext) -> Result<AllocationMap>;
}

pub trait CarverPlugin: Send + Sync {
    fn descriptor(&self) -> &CarverDescriptor;
    fn find_headers(&self, block: &[u8], absolute_offset: u64, sink: &mut dyn HeaderSink);
    fn assemble(&self, ctx: &CarveContext, hit: HeaderHit) -> Result<CarvedCandidate>;
}

pub trait Validator: Send + Sync {
    fn supports(&self, detected_type: &DetectedType) -> bool;
    fn validate(&self, input: &ValidationInput, limits: &ValidationLimits) -> ValidationReport;
}

pub trait RepairProvider: Send + Sync {
    fn supports(&self, report: &ValidationReport) -> bool;
    fn create_derivative(&self, input: &RepairInput) -> Result<RepairReport>;
}
```

Os nomes podem variar, mas as fronteiras não.

## 8.6 IPC

Requisitos:

- named pipes locais com ACL limitada ao SID do usuário atual;
- handshake com nonce criptograficamente aleatório e versão de protocolo;
- comandos allowlisted e serialização com limites rígidos;
- tamanho máximo por mensagem;
- leituras brutas em chunks alinhados e limitados;
- cancelamento e timeout;
- validação de source ID emitido pelo próprio broker;
- nenhum device path arbitrário;
- nenhum segredo na command line;
- parent PID/session binding;
- proteção contra replay na sessão;
- logs de auditoria sem conteúdo sensível;
- testes de spoofing, malformed frames e privilege boundary.

Comandos aproximados do broker:

```text
ListSources
OpenSource(source_inventory_id, expected_identity)
ReadAt(source_handle_id, offset, length)
QueryHealth(source_handle_id)
CloseSource(source_handle_id)
Shutdown
```

Não deve existir `WriteAt`, `Format`, `Trim`, `Delete`, `Lock`, `Dismount` ou comando genérico de `DeviceIoControl` exposto pelo IPC.

## 8.7 Banco da sessão

Tabelas mínimas:

```text
schema_migrations
scan_sessions
sources
source_regions
partitions
file_systems
candidates
candidate_names
candidate_paths
extents
extent_conflicts
carve_hits
validations
repairs
selections
restore_jobs
restore_items
events
checkpoints
errors
```

Princípios:

- IDs estáveis UUID/ULID ou equivalente;
- offsets/tamanhos em 64 bits;
- migrations versionadas e testadas;
- transações curtas;
- índices para filtros e ordenação;
- FTS somente se necessário e sem duplicar dados excessivamente;
- cursor pagination;
- escrita em batches;
- WAL quando seguro no disco de trabalho;
- recuperação após crash;
- nenhum banco na fonte.

## 8.8 Arquitetura futura

Manter `SourceReader`, parsers, carving, validators e restore independentes do Windows. O adapter Windows ficará em `io-windows`. Futuras plataformas precisarão de adapters próprios e sistemas de arquivos adicionais.

Não prometa que Android/iOS poderão fazer raw scan do armazenamento interno: os sandboxes móveis podem impedir esse acesso. O núcleo será reutilizável onde o sistema operacional fornecer acesso legítimo à mídia ou a imagens.


---

# 9. AQUISIÇÃO E I/O NO WINDOWS

## 9.1 Inventário nativo

Usar APIs oficiais do Windows sempre que possível, incluindo equivalentes a:

- `CreateFileW` para volumes e `\\.\PhysicalDriveN`;
- `DeviceIoControl`;
- `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` para mapear volume a disco físico;
- `IOCTL_DISK_GET_DRIVE_GEOMETRY_EX` e layout de partição;
- `IOCTL_STORAGE_QUERY_PROPERTY` para propriedades, bus type, alignment, seek penalty e TRIM;
- volume GUID paths e APIs de volume;
- SetupAPI/CM APIs quando úteis para hot-plug e identidade;
- APIs oficiais de BitLocker/WMI ou ferramentas do sistema somente para consulta de status, sem capturar chaves.

Não dependa exclusivamente de WMI para o caminho crítico de leitura. WMI pode enriquecer o inventário, mas o engine deve validar as informações com handles reais e control codes.

## 9.2 Abertura da fonte

O broker deverá:

- exigir elevação apenas quando necessária;
- abrir com `GENERIC_READ` ou acesso zero para consulta;
- usar `OPEN_EXISTING`;
- compartilhar leitura/escrita conforme exigido pela API para volumes montados, sem conceder escrita ao próprio processo;
- usar I/O overlapped quando benéfico;
- respeitar alinhamento de I/O não cacheado;
- nunca pedir `GENERIC_WRITE` para scan;
- nunca passar handle gravável ao processo principal;
- nunca chamar `FSCTL_LOCK_VOLUME` ou `FSCTL_DISMOUNT_VOLUME` no fluxo normal de scan.

Qualquer função FFI `unsafe` deve ficar isolada no crate `io-windows`, com:

- safety comments;
- validação de ponteiros e buffers;
- testes de limites;
- wrappers seguros;
- revisão específica do security agent.

Aplicar `#![forbid(unsafe_code)]` nos crates que não precisam de FFI.

## 9.3 Leitura por blocos

- tamanho de bloco adaptativo e alinhado;
- leitura principalmente sequencial em HDD;
- paralelismo limitado em SSD;
- backpressure entre I/O, parsing e persistência;
- buffer pool com orçamento máximo;
- cancel token verificado frequentemente;
- retries limitados;
- em erro, reduzir progressivamente o bloco até o setor mínimo quando apropriado;
- registrar bad ranges e prosseguir;
- não repetir indefinidamente leituras que estressam mídia defeituosa.

## 9.4 Mídia instável

O perfil **Mídia instável/gentle** deverá:

- privilegiar imagem sequencial antes da análise;
- usar uma passagem principal;
- reduzir seeks;
- limitar retries;
- registrar setores problemáticos;
- permitir retomada;
- pausar e recomendar laboratório profissional quando a taxa de erro ou sinais de falha ultrapassarem limiar configurado.

Nunca afirmar que leitura intensiva é inofensiva a uma unidade fisicamente defeituosa.

## 9.5 Fonte ativa e consistência

Registrar:

- volume serial/identity no início e no fim;
- tamanho/layout;
- estado do journal quando aplicável;
- alterações detectadas;
- se a fonte estava montada/ativa;
- se o scan foi feito sobre imagem estática.

Se a fonte mudar durante o scan, não descartar automaticamente o trabalho, mas marcar a sessão e candidatos afetados como potencialmente inconsistentes.

## 9.6 BitLocker

- detectar proteção e lock state;
- se bloqueado, instruir o usuário a desbloquear no Windows;
- não criar UI para coletar recovery key na primeira versão;
- não registrar segredo;
- após unlock, atualizar inventário e abrir o volume lógico;
- rotular claramente se a aquisição é lógica decifrada ou física cifrada.

## 9.7 Segurança de testes de dispositivo

Criar guardas obrigatórios no test harness:

- negar `\\.\PhysicalDrive0` e qualquer dispositivo não allowlisted;
- exigir marker de VHD de teste e identity esperada;
- recusar teste raw quando não estiver em CI VM isolada ou variável explícita de segurança;
- verificar tamanho máximo do device de teste;
- nunca executar `diskpart clean`, `format`, trim ou dismount em fonte não efêmera;
- fazer teardown idempotente apenas do VHD criado pelo próprio teste.

---

# 10. ENGINE DE PARTIÇÕES

## 10.1 MBR

Implementar e testar:

- assinatura;
- quatro entradas primárias;
- partições estendidas/EBR com prevenção de loop;
- offsets em LBA;
- validação contra tamanho da fonte;
- partições sobrepostas e corrompidas;
- recovery scanning opcional quando a tabela estiver danificada.

## 10.2 GPT

Implementar e testar:

- protective MBR;
- header primário e backup;
- CRC de header e entries;
- GUIDs, nomes UTF-16 e attributes;
- escolha segura entre cópia primária e backup;
- detecção de inconsistências;
- limites e sobreposição.

## 10.3 Layouts não simples

- volumes spanning/multi-disk: detectar; somente suportar se o `SourceReader` composto puder mapear todos os extents de forma comprovada;
- dynamic disks/Storage Spaces: não interpretar por heurística perigosa; detectar e falhar com mensagem clara ou operar sobre volume lógico exposto pelo Windows;
- hardware RAID apresentado como um único device pode ser tratado como uma fonte, sem tentar interpretar o array interno;
- offsets sempre relativos ao source/region explicitamente definido.

---

# 11. ENGINE NTFS

## 11.1 Objetivo

Recuperar candidatos por análise bruta do NTFS, sem depender apenas de APIs que retornam registros ativos.

## 11.2 Boot sector e parâmetros

Validar e extrair:

- OEM ID;
- bytes por setor;
- setores por cluster;
- tamanho do cluster;
- total de setores;
- LCN de `$MFT` e `$MFTMirr`;
- tamanho de file record;
- tamanho de index buffer;
- serial do volume;
- bounds e coerência.

Não confiar em campos sem validação cruzada.

## 11.3 MFT

Implementar:

- leitura bruta de `$MFT`;
- fallback limitado com `$MFTMirr` quando pertinente;
- parsing de records `FILE`;
- Update Sequence Array/fixups;
- flags in-use/directory;
- sequence number;
- base record e extension records;
- record number;
- atributos residentes e não residentes;
- atributo `$ATTRIBUTE_LIST`;
- recuperação de records marcados livres;
- identificação de record reutilizado;
- parsing resiliente de record parcialmente corrompido.

## 11.4 Atributos necessários

No mínimo:

- `$STANDARD_INFORMATION`;
- `$FILE_NAME` em todos os namespaces relevantes;
- `$DATA` unnamed e named;
- `$ATTRIBUTE_LIST`;
- `$INDEX_ROOT`;
- `$INDEX_ALLOCATION`;
- `$BITMAP`;
- `$EA`/`$EA_INFORMATION` apenas se necessários;
- reparse data somente como metadado, sem seguir links durante restore.

## 11.5 Data runs

Implementar com aritmética checada:

- VCN/LCN;
- signed relative offsets;
- sparse runs;
- múltiplos runs;
- extents em extension records;
- comprimento lógico, alocado e inicializado;
- truncamento e inconsistências;
- mapping para byte offsets da região.

Testar runlists malformados, overflow, negative relative LCN inválido e loops.

## 11.6 Resident, sparse, compressed e ADS

- conteúdo residente na MFT;
- sparse files com ranges lógicos zero;
- NTFS compression/LZNT1 com decompressor testado e limites;
- alternate data streams como streams separados associados ao arquivo;
- UI mostra ADS e restaura somente com opt-in;
- EFS marcado como cifrado, sem alegar preview ou uso sem chave.

## 11.7 Disponibilidade de clusters

Usar `$Bitmap` e evidências adicionais para classificar cada extent:

- livre no snapshot de análise;
- atualmente alocado;
- parcialmente conflitante;
- fora do volume;
- leitura com erro;
- zeroed/suspeita de TRIM;
- desconhecido.

Uma faixa atualmente alocada não prova que todos os bytes mudaram, mas é forte evidência de risco. Nunca rotular automaticamente como intacta.

## 11.8 Reconstrução de path

- usar parent file reference + sequence;
- detectar parent record reutilizado;
- escolher namespace Win32 adequado;
- preservar nomes alternativos como evidência;
- impedir ciclos;
- separar path reconstruído confiável de path inferido;
- colocar órfãos em agrupamento estável;
- não transformar um parent apagado em restauração em massa.

## 11.9 Fontes de enriquecimento

Usar, quando implementado e testado:

- `$UsnJrnl` para nomes/alterações e contexto temporal;
- directory indexes e `$I30` slack;
- `$LogFile` somente como módulo experimental bem isolado;
- Lixeira do Windows (`$I`/`$R`) para original path e deletion time.

Essas fontes enriquecem evidências; não devem substituir a análise de conteúdo.

## 11.10 Datas de exclusão

NTFS geralmente não fornece uma “data de exclusão” universal no record. Exibir somente quando derivada de fonte específica, por exemplo Lixeira ou journal, com campo de origem e nível de confiança. Caso contrário, mostrar “não determinada”.

## 11.11 Candidatos NTFS

Cada candidato deverá registrar:

- MFT record + sequence;
- flags e estado;
- nomes alternativos;
- path e confiança;
- timestamps;
- logical/allocated size;
- streams;
- extents;
- allocation status por range;
- evidência de compressão/sparse/EFS;
- método e warnings.

---

# 12. ENGINE FAT12/16/32

## 12.1 Estruturas

Implementar:

- BPB e variantes;
- cálculo de região reservada, FATs, root directory e data region;
- FAT12 packed entries;
- FAT16/FAT32 entries;
- múltiplas FATs e divergências;
- FAT32 FSInfo como hint, nunca única verdade;
- diretório root fixo em FAT12/16;
- cluster chains com loop detection.

## 12.2 Entradas apagadas

- detectar first byte `0xE5`;
- parsing de short names;
- LFN entries e checksum;
- recuperar primeiro caractere quando a cadeia LFN permitir;
- caso contrário, usar marcador como `_` e preservar incerteza;
- timestamps e tamanho;
- first cluster;
- deleted directories.

## 12.3 Cluster chain após exclusão

Como a FAT chain pode ter sido limpa:

1. tentar cadeia ainda presente e coerente;
2. tentar recuperação contígua pelo tamanho, apenas em clusters livres;
3. para fragmentação, usar heurística limitada e format-aware;
4. evitar busca combinatória sem limite;
5. registrar todas as suposições;
6. reduzir score quando a sequência for inferida.

## 12.4 Diretórios

- reconstruir árvore com entries apagadas;
- tratar `.` e `..` com cuidado;
- detectar cycles/corruption;
- seleção de pasta segue a regra granular do produto.

---

# 13. ENGINE exFAT

Implementar e testar:

- main e backup boot regions;
- checksum;
- cluster heap;
- FAT;
- allocation bitmap;
- upcase table;
- entry sets de file, stream extension e file name;
- flag in-use/inactive;
- secondary count e checksums;
- `NoFatChain` e arquivos contíguos;
- chains fragmentadas;
- nomes Unicode;
- timestamps e time-zone fields;
- diretórios apagados;
- extents e disponibilidade pelo bitmap.

Para entry set parcial ou corrompido, preservar evidência e reduzir confiança, sem inventar nome/tamanho.

---

# 14. CARVING E DETECÇÃO DE CONTEÚDO

## 14.1 Princípios

- priorizar espaço não alocado quando o bitmap for confiável;
- oferecer whole-region scan somente em modo RAW/damaged;
- ler sequencialmente e processar em pipeline;
- suportar assinaturas que atravessam fronteiras de bloco;
- limitar look-behind/look-ahead;
- deduplicar ranges;
- resolver overlaps;
- registrar offset absoluto e partição;
- nunca classificar por extensão apenas.

## 14.2 Arquitetura de plugins

Na versão 1.0:

- validators/carvers de código devem ser compilados e assinados junto com o app;
- custom signatures do usuário serão **declarativas**, em JSON/YAML validado;
- não carregar DLL arbitrária ou script do usuário no processo;
- schema de assinatura deve limitar regex, offsets, tamanho máximo e footer search;
- qualquer futura API de plugin de código deve executar fora do processo e ter threat model próprio.

## 14.3 Formatos iniciais de alto valor

Implementar detecção/carving/validação proporcional à estrutura para, no mínimo:

### Imagens

- JPEG/JFIF/Exif;
- PNG;
- GIF;
- BMP;
- TIFF;
- WebP;
- HEIF/HEIC por estrutura ISO BMFF quando tecnicamente/licenciadamente adequado.

### Documentos e dados

- PDF;
- ZIP;
- DOCX, XLSX, PPTX;
- ODT, ODS, ODP;
- OLE Compound File para DOC/XLS/PPT/MSI e outros;
- RTF;
- TXT/CSV;
- XML/HTML/JSON;
- SQLite.

### Áudio e vídeo

- MP3;
- WAV/RIFF;
- FLAC;
- OGG;
- MP4/MOV/M4V;
- AVI;
- MKV/WebM.

### Arquivos e e-mail

- 7z;
- RAR, respeitando licença e sem código proprietário não permitido;
- GZIP/TAR;
- EML/MBOX;
- PST/OST apenas se existir parser seguro, licenciado e amplamente testado; caso contrário, signature detection e status limitado.

### Executáveis

- PE/EXE/DLL;
- scripts por conteúdo textual;
- nunca executar.

A lista é baseline, não desculpa para ignorar outros formatos recuperáveis por metadados.

## 14.4 Fragmentação no carving

Carving contíguo não recupera genericamente arquivos fragmentados. Implementar apenas heurísticas limitadas e explicáveis, por exemplo:

- continuidade estrutural de chunks/boxes/pages;
- busca em janela limitada de clusters livres;
- validação incremental;
- beam search com limites estritos;
- timeout e orçamento por candidato.

Nunca declarar arquivo completo apenas porque header e footer foram encontrados em grande intervalo.

## 14.5 Custom signatures

UI avançada permite criar/importar assinatura com:

- nome do tipo;
- extensões sugeridas;
- magic bytes e máscaras;
- offset relativo;
- footer opcional;
- max size;
- alignment;
- regras simples de tamanho;
- validator genérico opcional.

Validar o schema e mostrar que assinatura personalizada não garante integridade.

---

# 15. VALIDAÇÃO, PREVIEW E REPARO

## 15.1 Princípios

- todo input é hostil;
- worker sem privilégio e sem rede;
- leitura bounded;
- timeouts;
- memory/CPU quotas;
- proteção contra decompression bombs;
- profundidade de recursão limitada;
- nenhuma macro;
- nenhum shell;
- nenhum codec externo não revisado no processo principal;
- crash de validator não encerra scan.

## 15.2 Validadores mínimos

### JPEG

- markers;
- segment lengths;
- SOS/EOI;
- decode bounded;
- dimensões;
- truncamento.

### PNG

- signature;
- chunk ordering;
- lengths;
- CRC;
- IHDR/IEND;
- decode bounded.

### ZIP e OOXML/ODF

- local headers e central directory;
- CRC quando possível;
- entries duplicadas/path traversal;
- compression ratio e size limits;
- arquivos obrigatórios como `[Content_Types].xml` para OOXML;
- reconhecer macro-enabled e não executar.

### PDF

- header;
- objetos/trailer;
- xref/xref streams;
- incremental updates;
- EOF;
- limites de objetos/streams;
- preview apenas se renderer isolado e licenciado estiver aprovado; caso contrário, exibir metadados/estrutura.

### OLE Compound File

- header;
- sector chains;
- DIFAT/FAT/miniFAT;
- directory entries;
- loops e bounds;
- identificar tipo provável.

### SQLite

- header;
- page size;
- page bounds;
- estrutura básica;
- `PRAGMA integrity_check` somente em cópia sandboxed quando seguro;
- não abrir banco original como writable.

### Áudio/vídeo

- parse de container;
- box/chunk lengths;
- indexes quando presentes;
- duração plausível;
- codecs apenas como metadado, salvo decoder sandboxed aprovado.

### PE

- DOS/PE headers;
- section table;
- bounds;
- imports/metadata como texto;
- nunca carregar DLL nem executar entry point.

### Texto

- encoding;
- proporção de caracteres válidos;
- NUL/controle;
- preview truncado;
- detecção de conteúdo binário.

### Desconhecido

- tamanho e ranges disponíveis;
- padrões zero;
- entropy como evidência auxiliar, não veredito;
- magic database local;
- hash sob demanda.

## 15.3 Resultado da validação

Cada report deve conter:

```text
validator_id
validator_version
status
confidence
format_detected
extension_match
structure_checks[]
errors[]
warnings[]
missing_ranges[]
preview_capability
repair_capability
resource_usage
```

## 15.4 Preview

- thumbnails persistidos somente no disco de trabalho;
- limpeza configurável;
- limite de tamanho/dimensões;
- nenhum link externo navegável;
- HTML/texto escapado;
- CSP restritiva;
- URLs `file:` não arbitrárias;
- não permitir drag-out de arquivo temporário não finalizado;
- hex viewer com janela paginada, não carregar arquivo inteiro.

## 15.5 Reparo

Reparo deve ser conservador e separado.

Casos possíveis, somente com testes:

- reconstrução de central directory ZIP a partir de local headers íntegros;
- reconstrução de xref PDF quando objetos suficientes existirem;
- correção de comprimentos/chunks quando o valor puder ser derivado sem inventar conteúdo;
- extração de entries válidas de container parcialmente danificado;
- salvamento de frames/páginas/objetos recuperáveis como derivados separados.

Sempre gerar:

- original extraction;
- derivative;
- provenance report;
- hash de ambos;
- lista exata de transformações.

---

# 16. MODELO DE RECUPERABILIDADE

## 16.1 Separar três avaliações

A UI deverá mostrar separadamente:

1. **Recuperabilidade do conteúdo — 0 a 100**;
2. **Confiança dos metadados — Alta/Média/Baixa**;
3. **Validação estrutural — Aprovada/Parcial/Falhou/Não disponível/Não executada**.

O caminho/nome não deve aumentar artificialmente o score do conteúdo.

## 16.2 Componentes do score de conteúdo

Calcular e armazenar componentes explicáveis, não apenas número final:

- disponibilidade dos extents;
- conflito com alocação atual;
- ranges ausentes;
- read errors;
- continuidade/fragmentação;
- evidência de zero/TRIM;
- coerência de tamanho;
- validação estrutural;
- checksum/CRC interno;
- ambiguidade da reconstrução.

## 16.3 Regras de teto

Exemplos obrigatórios, ajustáveis por ADR e testes:

- conteúdo totalmente legível, extents não conflitantes e validação forte: 85–99;
- conteúdo totalmente legível, sem validator disponível: máximo 84;
- cadeia inferida com ambiguidade relevante: máximo 74;
- qualquer range conhecido ausente: máximo 69;
- conflito/sobrescrita relevante: máximo 49;
- maior parte zeroed/TRIM ou indisponível: máximo 10;
- somente metadados sem conteúdo: 0.

Nunca usar 100 em dados reais sem hash original confiável conhecido e comparado.

## 16.4 Labels

- **Excelente** — 85–99;
- **Boa** — 70–84;
- **Parcial** — 40–69;
- **Baixa** — 11–39;
- **Irrecuperável pelos dados disponíveis** — 0–10.

O usuário poderá abrir “Por que esta nota?” e ver as evidências.

## 16.5 Confiança de metadados

Alta:

- record consistente;
- parent sequence válido;
- nome/path reconstruído sem inferência relevante.

Média:

- parte do path inferida;
- parent apagado mas coerente;
- Lixeira/journal fornece complemento.

Baixa:

- record reutilizado;
- nome parcial;
- carved sem path;
- associação ambígua.

## 16.6 Zero/TRIM

Não inferir TRIM apenas porque um bloco contém zeros; dados legítimos também podem ser zero. Registrar:

- “zeroed” quando observado;
- “TRIM provável” somente se dispositivo/volume suporta o comportamento e o padrão/evidência corroborar;
- “indeterminado” quando não for possível distinguir.

---

# 17. PIPELINE DE SCAN

## 17.1 Fluxo

```text
Preflight
→ Inventory snapshot
→ Read-only source open
→ Partition discovery
→ Filesystem probe
→ Metadata scan
→ Candidate normalization
→ Allocation/conflict analysis
→ Optional unallocated carving
→ Validation queue
→ Deduplication/merge
→ Score update
→ Session finalization
```

## 17.2 Pipeline com backpressure

Usar bounded queues:

- reader;
- block classifier;
- filesystem parser;
- carver;
- candidate normalizer;
- DB writer;
- validation scheduler.

Nenhum produtor pode crescer memória ilimitadamente. Persistir batches e checkpoints.

## 17.3 Checkpoints

Checkpoint deve incluir:

- source identity;
- region/partition;
- phase;
- último offset seguro;
- parser cursor;
- carver overlap tail;
- plugin versions/config;
- DB transaction watermark;
- allocation map version;
- errors.

Retomada deve verificar identidade e configuração antes de continuar.

## 17.4 Deduplicação

- interval tree/range index para overlaps;
- fingerprint parcial apenas como hint;
- hash completo sob demanda;
- merge metadata + carved quando conteúdo/ranges e estrutura coincidirem;
- nunca mesclar somente pelo nome;
- manter provenance de todos os métodos.

## 17.5 Priorização de validação

Validar primeiro:

- item aberto pelo usuário;
- itens selecionados;
- candidatos de alto valor/configuração;
- candidatos que definem score ambíguo;
- previews visíveis.

Não validar automaticamente cada byte de milhões de candidatos se isso inviabilizar o scan; oferecer validação completa posterior.

---

# 18. RESTORE ENGINE EM DETALHE

## 18.1 Planejamento

Antes de iniciar:

- revalidar source identity;
- revalidar destination identity;
- garantir que origem e destino não violam política;
- calcular espaço necessário, incluindo temporários;
- detectar filesystem do destino e limitações;
- resolver nomes inválidos/reservados;
- produzir plano imutável versionado.

## 18.2 Sanitização de path

Impedir:

- `..` e traversal;
- absolute path injetado por candidato;
- UNC inesperado;
- ADS via `:` não explicitamente autorizado;
- device names `CON`, `PRN`, `AUX`, `NUL`, `COM1` etc.;
- trailing dots/spaces problemáticos;
- path fora do root escolhido;
- symlink/reparse traversal no destino.

Usar handles e verificações de path final para garantir contenção.

## 18.3 Original path

“Restaurar no local original” significa:

- criar diretórios normais necessários;
- gravar uma nova cópia do arquivo;
- não editar MFT/FAT/exFAT para reativar entries;
- não reconstruir automaticamente conteúdo não selecionado;
- não sobrescrever item ativo sem consentimento.

## 18.4 Leitura e escrita

- ordenar extents para reduzir seeks quando restaurar muitos itens, sem quebrar prioridade do usuário;
- suportar resident data;
- decomprimir NTFS quando necessário;
- materializar sparse ranges corretamente no destino compatível ou escrever zeros;
- tratar gaps conforme política;
- calcular SHA-256 durante stream;
- opcionalmente reler destino para verificar write hash;
- flush antes do rename final;
- manter journal da operação.

## 18.5 Cancelamento e falhas

- cancelamento não apaga arquivos concluídos;
- temporários incompletos recebem status claro;
- retomada valida tamanho/hash antes de prosseguir;
- erro em um item não cancela todos, salvo falha da fonte/destino;
- mostrar resumo por item.

## 18.6 Restore no mesmo disco

Somente após scan concluído e com confirmação digitada, por exemplo:

```text
EU ENTENDO QUE POSSO SOBRESCREVER OUTROS ARQUIVOS
```

Além disso:

- mostrar que outra partição/letra do mesmo disco físico também é arriscada;
- recomendar selecionar outro disco;
- registrar o override no manifesto;
- recalcular scores de candidatos ainda não restaurados quando possível, pois a escrita pode invalidá-los;
- nunca permitir same-disk em modo Guiado.

---

# 19. UX/UI E DESIGN SYSTEM

## 19.1 Direção visual

A experiência deverá ser elegante, moderna e cativante, sem aparência de brinquedo ou painel genérico.

Direção:

- fundo midnight/navy profundo;
- superfícies com profundidade suave e transparência controlada;
- acentos cyan, violeta, verde e coral para estados;
- tipografia `Segoe UI Variable`/system stack;
- ícones consistentes, como Lucide, com licença revisada;
- gradientes discretos;
- iluminação/halo apenas em áreas focais;
- alto contraste e estados claros;
- tema escuro e claro;
- densidade confortável e compacta.

Tokens sugeridos podem ser ajustados após teste de contraste:

```text
background: #080D18
surface-1:  #0F1728
surface-2:  #151F34
text:       #F4F7FC
muted:      #99A7BE
cyan:       #38D5FF
violet:     #806BFF
green:      #42DEA0
amber:      #FFBC55
coral:      #FF647C
```

Não usar somente cor para transmitir estado.

## 19.2 Movimento

- animações de 150–300 ms em transform/opacity;
- transições de fase do scan;
- pulse discreto para atividade;
- progress visuals fluidos, sem fingir precisão;
- respeitar `prefers-reduced-motion`;
- nenhuma animação que reduza legibilidade da tabela;
- nenhuma atualização visual por item que cause jank em milhões de resultados.

## 19.3 Navegação principal

```text
Home / Sources
Scan setup
Live scan
Results
Restore
Sessions
Settings
Help & limitations
```

## 19.4 Home / Sources

- hero compacto com tagline;
- botão “Atualizar dispositivos”;
- cards de discos;
- ação “Abrir imagem de disco”;
- sessões recentes;
- alertas importantes;
- nenhum dado técnico excessivo no primeiro nível.

## 19.5 Scan setup

Wizard de uma página ou etapas curtas:

1. fonte;
2. modo;
3. pasta de trabalho/destino recomendado;
4. tipos prioritários opcionais — nunca limitam metadata recovery;
5. revisão de segurança;
6. iniciar.

O usuário poderá escolher tipos prioritários para validação/carving, mas “todos” permanece disponível.

## 19.6 Live scan

- anel/linha de progresso por fase;
- cartões de métricas;
- gráfico simples de throughput sem sobrecarregar;
- feed de descobertas agregado, não nomes piscando incessantemente;
- botões pause/cancel;
- resultados parciais;
- indicadores de source read-only e working disk.

## 19.7 Results workspace

Layout:

```text
[filters/sidebar] [virtualized result table] [details/preview drawer]
```

- sidebar recolhível;
- chips de extensão com counts e multi-select;
- seleção por checkbox;
- header sticky;
- column chooser;
- resize/reorder;
- keyboard navigation;
- context menu acessível;
- preview drawer redimensionável;
- seleção sticky no rodapé.

## 19.8 Restore flow

- resumo dos selecionados;
- destino com badge “outro disco físico” ou warning;
- estrutura de pastas;
- política de conflitos;
- parciais/reparos;
- espaço livre;
- confirmação;
- progresso por arquivo e total;
- relatório final com “Abrir pasta”, não “Executar arquivo”.

## 19.9 Acessibilidade

- WCAG 2.2 AA onde aplicável;
- navegação integral por teclado;
- focus rings visíveis;
- labels e descriptions;
- screen reader announcements agregados para progresso;
- contraste testado;
- zoom 100–200%;
- high contrast mode do Windows;
- reduced motion;
- áreas clicáveis adequadas;
- textos de erro que expliquem ação.

## 19.10 Responsividade desktop

- mínimo útil 1100×700;
- comportamento aceitável em 1366×768;
- otimização para 1920×1080 e telas maiores;
- suporte a scaling 125%, 150% e 200%;
- múltiplos monitores e mudanças de DPI.

## 19.11 Microcopy

Evitar jargão sem explicação. Exemplos:

- “Registros encontrados” em vez de “arquivos recuperados” antes da validação;
- “Forte evidência de conteúdo completo” em vez de “100% garantido”;
- “Parte do conteúdo pode ter sido sobrescrita”;
- “O caminho original foi reconstruído com baixa confiança”;
- “Este arquivo foi encontrado por assinatura; o nome original não está disponível”.

---

# 20. PERFORMANCE, ESCALA E RESILIÊNCIA

## 20.1 Metas iniciais

Tratar como metas a medir, não como números inventados:

- UI responsiva com 1 milhão de candidatos persistidos;
- cancelamento percebido em até 2 segundos na maioria das operações;
- memória do frontend em idle preferencialmente abaixo de 250 MB;
- orçamento padrão do engine abaixo de 1 GB, ajustável;
- throughput de deep sequential scan próximo ao limite saudável da fonte, com meta inicial ≥70% do sequential read medido no mesmo ambiente, excluindo validação pesada;
- banco sem degradação catastrófica ao crescer;
- startup sem reprocessar sessões inteiras.

Se metas forem alteradas, registrar benchmark e ADR.

## 20.2 Estratégias

- virtualização de linhas;
- server-side filter/sort;
- prepared statements;
- batched inserts;
- indexes medidos;
- bounded queues;
- pooled aligned buffers;
- task scheduling por perfil da mídia;
- thumbnails lazy;
- hashes sob demanda;
- cancellation cooperativa;
- evitar serialização JSON massiva entre Rust e frontend;
- eventos agregados em intervalos razoáveis.

## 20.3 HDD versus SSD

- HDD: poucos readers e leitura sequencial;
- SSD: concorrência limitada, medida por benchmark;
- removable USB: detectar throughput real e ajustar;
- nunca saturar fila de I/O a ponto de tornar cancelamento lento.

## 20.4 Banco grande

Testar:

- 100 mil, 1 milhão e volume maior simulado;
- filtros combinados;
- seleção persistente;
- atualização de scores;
- export CSV/JSON streaming;
- migração;
- crash/recovery.

## 20.5 Reconnect

- monitorar arrival/removal;
- pausar imediatamente em device removal;
- não reutilizar letter como prova de identidade;
- reabrir somente após identity match;
- revalidar offsets/layout;
- permitir abandonar sessão preservando resultados existentes.


---

# 21. SEGURANÇA, PRIVACIDADE E THREAT MODEL

## 21.1 Ativos protegidos

- integridade da fonte;
- arquivos ativos da fonte;
- dados ainda recuperáveis;
- conteúdo recuperado;
- privilégios administrativos;
- sessão e manifests;
- identidade/paths do usuário;
- supply chain do aplicativo;
- confiabilidade dos resultados.

## 21.2 Adversários e falhas consideradas

- filesystem deliberadamente malformado;
- imagem de disco criada para explorar o parser;
- integers causando overflow/underflow;
- offsets fora do buffer;
- decompression bomb;
- ZIP path traversal;
- PDF/media/codec malicioso;
- executável recuperado malicioso;
- spoofing de named pipe;
- processo local tentando usar o broker privilegiado;
- command injection;
- symlink/reparse point no destino;
- DLL search order hijacking;
- dependência comprometida;
- plugin/MCP não confiável durante o desenvolvimento;
- source desconectada/substituída;
- record reuse levando a path falso;
- score enganoso;
- crash e corrupção da sessão;
- escrita acidental na fonte.

## 21.3 Controles obrigatórios

### Boundary de privilégio

- main app unelevated;
- helper com comandos mínimos;
- token/ACL/session binding;
- UAC somente quando necessário;
- no generic command execution;
- source allowlist gerada pelo próprio broker;
- nenhuma escrita raw.

### Segurança de memória

- Rust safe por padrão;
- `unsafe` isolado e revisado;
- checked arithmetic;
- parsers cursor-based com bounds;
- fuzz contínuo;
- timeouts e quotas.

### Tauri/WebView

- CSP restritiva;
- sem remote content;
- navigation allowlist vazia ou estrita;
- Tauri capabilities mínimas por window;
- commands explícitos, tipados e validados;
- sem shell plugin genérico;
- sem abertura arbitrária de URL/path;
- no devtools em release, salvo build especial;
- atualizar WebView2/runtime conforme política suportada.

### Worker sandbox

- AppContainer sem capabilities de rede, preferencialmente;
- fallback documentado: restricted token + low integrity + Job Object + no inherited handles + quotas;
- temp directory própria;
- arquivo individual ou handle somente-leitura;
- kill on timeout;
- crash isolation;
- input/output protocol bounded.

### Restore

- path containment;
- no follow reparse points;
- collision policy;
- temp + atomic rename;
- destination revalidation;
- source/destination disk identity check.

### Supply chain

- lockfiles;
- version pinning;
- review de atualização;
- `cargo audit`/RustSec;
- `cargo deny` para licenças, bans e fontes;
- npm audit e/ou OSV scanner;
- secret scanning;
- dependency review;
- SBOM CycloneDX ou SPDX;
- reproducible build guidance;
- signed tags/releases;
- Authenticode quando certificado disponível;
- hashes publicados.

## 21.4 Threat model formal

Criar `docs/threat-model/THREAT_MODEL.md` com:

- diagramas de data flow;
- trust boundaries;
- STRIDE por componente;
- risco, impacto, probabilidade, mitigação, teste e owner;
- abuso de IPC;
- parser hostile input;
- restore path attacks;
- supply-chain;
- residual risks.

Criar testes ou evidências para cada mitigação crítica.

## 21.5 Privacidade

- sem telemetry por padrão;
- sem upload de crash dump;
- crash reports locais e opt-in para exportação manual;
- paths redigíveis;
- previews e thumbnails locais;
- documentação clara de onde os dados ficam;
- botão para limpar sessão;
- não armazenar BitLocker keys, passwords ou file contents no log;
- clipboard usado somente por ação explícita.

## 21.6 Segurança de MCP, plugins e agents durante o desenvolvimento

- usar apenas MCP servers oficiais ou explicitamente confiáveis;
- iniciar GitHub MCP em read-only e menor toolset possível;
- não fornecer secrets além do escopo necessário;
- revisar plugin hooks antes de habilitar;
- não instalar plugin apenas pelo nome sem verificar publisher/repo/licença;
- não permitir que MCP execute comandos destrutivos no host;
- approvals para operações sensíveis;
- registrar no relatório quais MCPs/skills/plugins foram realmente usados.

---

# 22. OBSERVABILIDADE E DIAGNÓSTICO

## 22.1 Logging

Use `tracing` ou equivalente com:

- correlation IDs;
- session/source/job IDs;
- níveis configuráveis;
- rotação e limite de tamanho;
- redaction;
- nenhum conteúdo binário;
- nenhum secret;
- timestamps monotônicos e wall-clock quando útil.

## 22.2 Eventos de auditoria

Registrar:

- source opened/closed;
- read-only flags efetivos;
- scan config;
- override de risco;
- validation/repair;
- restore plan;
- result per item;
- errors;
- cancellation;
- source change/removal;
- version dos plugins.

## 22.3 Diagnóstico exportável

Gerar bundle manual, com preview antes de exportar:

- logs redigidos;
- versão do app;
- OS/build;
- feature flags;
- dependency versions relevantes;
- session schema;
- erros;
- sem incluir arquivos recuperados ou nomes completos por padrão.

---

# 23. SPEC-DRIVEN DEVELOPMENT DOCUMENTATION

Antes da implementação substancial, criar e revisar os seguintes documentos. Eles são source of truth versionado.

## 23.1 Documentos obrigatórios

```text
docs/specs/000-product-vision.md
docs/specs/001-functional-requirements.md
docs/specs/002-non-functional-requirements.md
docs/specs/003-domain-model.md
docs/specs/004-architecture.md
docs/specs/005-windows-io-and-privilege-model.md
docs/specs/006-partition-and-filesystem-engines.md
docs/specs/007-carving-validation-repair.md
docs/specs/008-session-data-model.md
docs/specs/009-restore-semantics.md
docs/specs/010-ux-ui-and-accessibility.md
docs/specs/011-security-and-privacy.md
docs/specs/012-test-and-validation-plan.md
docs/specs/013-build-release-and-signing.md
docs/specs/014-localization.md
docs/specs/015-known-limitations.md
docs/traceability-matrix.md
docs/risk-register.md
PLANS.md
```

## 23.2 Conteúdo de cada requisito

Cada requisito deve ter:

```text
ID
Title
Rationale
Priority
Source
Preconditions
Behavior
Error behavior
Security implications
Observability
Acceptance criteria
Test IDs
Implementation links
Status
```

## 23.3 ADRs mínimos

Criar ADRs para:

1. Tauri + React + Rust;
2. process separation e UAC;
3. read-only source invariant;
4. IPC protocol;
5. SQLite e session schema;
6. NTFS raw parsing versus APIs de active records;
7. filesystem plugin architecture;
8. carving plugin architecture;
9. sandbox strategy;
10. recoverability scoring;
11. same-physical-disk policy;
12. dependency/license policy;
13. installer/update strategy;
14. ReFS scope;
15. Windows 10/11 support matrix;
16. image formats;
17. test fixtures e external forensic corpora.

## 23.4 Matriz de rastreabilidade

Mapear:

```text
Requirement → Design section → Code module → Unit tests → Integration tests → E2E scenario → Evidence artifact
```

Nenhum requisito Must/High pode ficar sem teste ou justificativa formal.

## 23.5 Risk register

Campos:

- risk ID;
- descrição;
- categoria;
- trigger;
- likelihood;
- impact;
- mitigation;
- contingency;
- test/evidence;
- residual risk;
- owner;
- status.

## 23.6 Change control

Quando uma decisão divergir desta especificação:

1. criar ADR;
2. explicar por que;
3. avaliar segurança, UX, testes e compatibilidade;
4. atualizar requirement e traceability;
5. obter revisão independente de subagent;
6. prosseguir sem simplesmente apagar o requisito.

## 23.7 Spec gates internos

### Gate S0 — Discovery

- repo/environment inventory;
- agents/skills/MCP inventory;
- assumptions list;
- risk preliminar.

### Gate S1 — Requirements

- requirements completos e testáveis;
- termos definidos;
- non-goals claros;
- acceptance criteria.

### Gate S2 — Architecture and threat model

- process boundaries;
- data flow;
- raw I/O;
- security;
- ADRs;
- test strategy.

### Gate S3 — Implementation readiness

- interfaces;
- schema;
- fixtures;
- build setup;
- vertical slice plan.

### Gate S4 — Release readiness

- traceability completa;
- tests pass;
- security review;
- installer;
- known limitations;
- completion report.

Esses gates são revisões internas; não pare para pedir aprovação do usuário entre eles.

---

# 24. ORQUESTRAÇÃO DE AGENTS, SKILLS, MCP E PLUGINS

## 24.1 Inventário obrigatório

No início, executar e registrar:

- agents/subagents disponíveis;
- skills globais e do repo;
- plugins instalados;
- MCP servers conectados e tools expostas;
- permissões/sandbox;
- toolchains Rust/Node/Windows;
- Visual Studio Build Tools/WebView2;
- CI/repository state.

Salvar em:

```text
docs/evidence/environment-inventory.md
```

Não afirmar uso de ferramenta ausente.

## 24.2 Subagents recomendados

Criar ou invocar, conforme disponibilidade:

### `product-spec-agent`

Revisa requisitos, ambiguidades, UX e traceability.

### `windows-storage-agent`

Pesquisa APIs oficiais, device enumeration, raw I/O, privileges, BitLocker e hardware behavior.

### `ntfs-agent`

Especifica/implementa parser NTFS e fixtures.

### `fat-exfat-agent`

Especifica/implementa FAT/exFAT.

### `carving-validation-agent`

Assinaturas, validators, repair e dedupe.

### `desktop-ux-agent`

Design system, React/Tauri UI, acessibilidade e i18n.

### `security-agent`

Threat model, IPC, sandbox, dependency e review de unsafe.

### `qa-fuzz-agent`

Fixtures, property tests, fuzz, corpora e failure injection.

### `performance-agent`

I/O pipeline, SQLite, million-row UI e benchmarks.

### `release-agent`

CI, installer, signing, SBOM e artifacts.

### `independent-reviewer-agent`

Não implementa o módulo revisado. Procura falhas, inconsistências, overclaims e gaps de teste.

## 24.3 Regras de paralelismo

- paralelizar exploração, documentação, revisão e execução de testes;
- não deixar múltiplos agents editar os mesmos arquivos sem worktree/ownership;
- cada agent retorna resumo, decisões, files e test evidence;
- agente principal integra;
- independent reviewer revisa após integração;
- conflitos não podem ser resolvidos apagando silenciosamente trabalho.

## 24.4 Skills

Usar skills existentes úteis. Se não existirem, criar skills locais focadas em workflows repetíveis, por exemplo:

```text
.agents/skills/spec-authoring/
.agents/skills/adr-review/
.agents/skills/windows-raw-io-safety/
.agents/skills/filesystem-parser-testing/
.agents/skills/rust-fuzzing/
.agents/skills/tauri-e2e/
.agents/skills/accessibility-audit/
.agents/skills/release-evidence/
```

Cada skill terá `SKILL.md`, scripts/references somente quando necessários. Não duplicar toda a especificação dentro de skills.

## 24.5 AGENTS.md

Criar `AGENTS.md` curto e durável com:

- comandos de build/test;
- source read-only invariant;
- proibição de testes destrutivos;
- code style;
- ownership de diretórios;
- docs/traceability update rule;
- no TODO/stub completion rule;
- security review para `unsafe` e IPC;
- localization rule;
- final evidence requirements.

Criar `AGENTS.md` aninhados apenas onde regras específicas forem necessárias, por exemplo em `crates/io-windows`, `crates/fs-ntfs` e `tests/`.

## 24.6 MCP servers recomendados

Usar se realmente configurados ou configuráveis com segurança:

### Microsoft Learn MCP

Para Win32, storage, BitLocker, filesystem e Windows security. Endpoint oficial disponível na documentação Microsoft. Usar como fonte primária.

### GitHub MCP oficial

- inicialmente read-only;
- pesquisar issues/releases de dependências;
- criar PR/issues apenas quando repositório e credenciais estiverem corretamente configurados;
- mínimo toolset e token scope.

### OpenAI/Codex docs

Para AGENTS, skills, subagents, MCP, permissions e configuração atual.

### Playwright MCP

Útil para inspeção visual web/component preview, acessibilidade e screenshots locais. Não substituir o E2E real do executável Tauri.

### Figma MCP

Somente se já existir workspace/projeto e acesso configurado. Não bloquear a implementação por ausência de Figma.

## 24.7 Plugins

- inventariar antes de usar;
- preferir oficiais ou publisher confiável;
- revisar hooks;
- registrar versão e finalidade;
- não habilitar plugin que amplie privilégios sem necessidade;
- nenhum plugin pode dispensar testes locais.

## 24.8 Fallback

Se MCP/skill/plugin recomendado não estiver disponível:

- usar documentação oficial acessível por meios permitidos;
- não interromper o projeto;
- registrar indisponibilidade;
- não inventar resultado da ferramenta.

---

# 25. PLANO DE IMPLEMENTAÇÃO EM VERTICAL SLICES

O trabalho pode ser organizado internamente em slices, mas a entrega ao usuário é única e completa.

## Slice 0 — Foundation and safety

- repo/toolchains;
- docs/specs/ADRs;
- CI inicial;
- core types;
- source read-only abstractions;
- synthetic image reader;
- test guards;
- design system skeleton.

Gate: source reader tests e proibição de write demonstrada.

## Slice 1 — Image source + NTFS metadata recovery

- raw image open;
- GPT/MBR;
- NTFS boot/MFT;
- deleted records;
- candidate DB;
- basic results UI;
- exact fixture recovery by hash.

Gate: recuperar bytes esperados de fixture determinística.

## Slice 2 — Windows devices and elevated broker

- inventory;
- UAC helper;
- read IPC;
- source identity;
- hot-plug;
- security tests.

Gate: broker não possui write command e rejeita spoofing/malformed request.

## Slice 3 — Restore

- selection;
- destination safety;
- path reconstruction;
- transactional copy;
- manifests;
- original path semantics.

Gate: restore só selecionado e same-disk policy testada.

## Slice 4 — FAT/exFAT

- parsers;
- deleted entries;
- contiguous/limited fragmented recovery;
- fixtures/hash tests.

## Slice 5 — Deep carving and validators

- unallocated map;
- signature pipeline;
- baseline formats;
- sandbox worker;
- preview;
- scoring.

## Slice 6 — Scale and resilience

- checkpoint/resume;
- removal/reconnect;
- million-row DB/UI;
- performance profiles;
- bad-sector injection.

## Slice 7 — Production UX and release

- full UX;
- i18n;
- accessibility;
- installer/portable;
- SBOM/signing path;
- security review;
- docs and completion evidence.

Cada slice deve incluir tests e docs no mesmo change. Não criar “testes depois”.

---

# 26. ESTRATÉGIA DE TESTES

## 26.1 Pirâmide

### Rust unit tests

- every parser primitive;
- bounds;
- data runs;
- checksums;
- path sanitation;
- score rules;
- IPC codec;
- DB queries;
- restore planning.

### Property tests

Usar `proptest` ou equivalente para invariantes:

- parser nunca lê fora do buffer;
- offsets resultantes permanecem na source region;
- encode/decode IPC roundtrip;
- path sanitization sempre permanece no root;
- range merge não perde/duplica bytes;
- pagination estável;
- score dentro de bounds e tetos.

### Fuzzing

Usar `cargo-fuzz`/libFuzzer para:

- MBR/GPT;
- NTFS boot record;
- MFT record/fixups;
- NTFS attributes/runlists;
- FAT directory entries/chains;
- exFAT entry sets;
- carving signatures;
- validators;
- IPC frames;
- session import;
- restore manifest/path.

Manter corpus e regressions.

### Integration tests

- synthetic images;
- end-to-end scan to DB;
- restore to temp destination;
- hash compare;
- pause/resume;
- crash/reopen;
- read errors;
- corrupted metadata.

### Frontend tests

- Vitest ou equivalente;
- component behavior;
- filters;
- selection;
- i18n;
- error states;
- accessibility.

### Tauri tests

- Tauri mock runtime para commands/component integration;
- WebdriverIO com `@wdio/tauri-service` para o aplicativo real;
- screenshots/evidence;
- frontend/backend logs.

### Security tests

- IPC unauthorized client;
- malformed/oversized messages;
- path traversal;
- reparse point race;
- zip bomb;
- parser timeout;
- worker escape assumptions;
- Tauri command allowlist;
- source write attempt must fail by design;
- DLL search path.

### Performance tests

- sequential throughput;
- scan pipeline;
- 1M candidates;
- filter/sort latency;
- memory;
- cancellation;
- DB export;
- reconnect.

## 26.2 Fixture builder

Criar `fixture-builder` determinístico com manifest de verdade:

```text
fixture_id
filesystem
sector_size
cluster_size
files_before_delete[]
delete_order
post_delete_writes[]
expected_candidates[]
expected_paths[]
expected_extents[]
expected_content_sha256[]
expected_missing_ranges[]
expected_quality_bounds
```

## 26.3 Casos NTFS mínimos

- resident file;
- nonresident contiguous;
- nonresident fragmented;
- MFT record deleted;
- MFT record reused;
- parent deleted;
- parent reused;
- nested deleted folders;
- multiple names/namespaces;
- `$ATTRIBUTE_LIST`;
- extension records;
- sparse;
- compressed;
- ADS;
- EFS marker;
- Unicode/emoji/long name;
- zero-length;
- >4 GB logical simulated;
- partial overwrite first/middle/end;
- allocated conflict;
- all-zero content;
- bad sector;
- corrupt fixup;
- corrupt runlist;
- orphan;
- Lixeira pair.

## 26.4 FAT/exFAT mínimos

- short name;
- LFN;
- lost first char;
- contiguous deleted file;
- fragmented chain retained;
- chain cleared;
- deleted directory;
- nested folders;
- FAT copies disagree;
- cluster loop;
- exFAT NoFatChain;
- exFAT fragmented;
- inactive entry set partial;
- allocation conflict;
- Unicode.

## 26.5 Carving mínimos

Para cada formato baseline:

- complete contiguous;
- truncated;
- false header;
- overlap;
- boundary-crossing signature;
- corrupt length;
- huge declared size;
- fragmented when supported;
- embedded file;
- active-file duplicate;
- validator pass/fail.

## 26.6 Falhas e resiliência

- source removed mid-read;
- destination full;
- destination removed;
- process crash after checkpoint;
- worker crash;
- DB locked/corrupt copy;
- permission denied;
- UAC canceled;
- BitLocker locked;
- identity mismatch on reconnect;
- source changed;
- millions of rows;
- cancellation at every pipeline stage.

## 26.7 Corpora independentes

Usar, conforme licença e disponibilidade:

- NIST CFReDS deleted-file datasets;
- NIST/CFTT material relacionado;
- Digital Corpora disk images;
- fixtures próprias publicáveis;
- The Sleuth Kit como oracle de desenvolvimento/differential testing, sem incorporá-lo ao binário de produção sem análise de licença.

Resultados externos devem ser versionados por dataset ID/hash e não substituir fixtures determinísticas próprias.

## 26.8 Differential testing

Comparar candidatos e extrações com ferramentas forenses maduras:

- nomes/records;
- extents;
- content hashes;
- casos de divergência.

Divergência não significa automaticamente que o Undelete Master está errado; investigar e documentar.

## 26.9 Mutation testing

Aplicar em módulos críticos, pelo menos:

- score;
- path sanitation;
- range mapping;
- restore selection;
- IPC authorization;
- key parser logic.

A meta é demonstrar que testes detectam mudanças de comportamento relevantes.

## 26.10 Cobertura

Cobertura numérica não substitui qualidade. Ainda assim:

- relatório por crate/module;
- alta cobertura em parser primitives, restore safety e IPC;
- branches de erro exercitados;
- qualquer exclusão justificada.

---

# 27. TESTES DE UX E ACESSIBILIDADE

## 27.1 Fluxos E2E obrigatórios

1. primeiro uso e consentimento;
2. selecionar imagem fixture;
3. scan rápido;
4. filtro de duas extensões;
5. seleção persistente após ordenar;
6. preview seguro;
7. selecionar arquivo em pasta apagada;
8. restore apenas desse arquivo e ancestors;
9. conflito de nome;
10. arquivo parcial;
11. pause/resume;
12. sessão reaberta;
13. warning same physical disk;
14. BitLocker locked mock;
15. source removal/reconnect mock;
16. reduced motion;
17. keyboard-only;
18. screen-reader semantics;
19. 200% scaling;
20. million-row virtualized table.

## 27.2 Visual regression

- screenshots estáveis por viewport/theme;
- tolerância controlada;
- revisão de estados vazios, loading, error, partial, offline e success;
- não usar snapshot visual como única prova funcional.

## 27.3 Accessibility automation

- axe-core em components e flows;
- sem violations critical/serious abertas;
- teste manual de teclado;
- Narrator smoke test em Windows;
- contrast checks;
- focus order.

---

# 28. CI/CD, BUILD E RELEASE

## 28.1 CI em pull requests

Executar:

- format check Rust/TS;
- clippy com warnings como erro;
- TypeScript strict;
- ESLint;
- unit/property tests;
- integration fixtures pequenas;
- frontend tests;
- security/dependency scans;
- license checks;
- build debug/release smoke;
- traceability drift check.

## 28.2 Nightly/extended

- fuzz por tempo definido;
- corpora externos;
- performance;
- million-row;
- E2E completo;
- VHD isolated tests;
- Windows version matrix;
- mutation subset;
- SBOM.

## 28.3 Release artifacts

Produzir, conforme suportado e testado:

```text
UndeleteMaster-Setup-x64.exe
UndeleteMaster-x64.msi
UndeleteMaster-Portable-x64.zip
SHA256SUMS.txt
SBOM.cdx.json
THIRD_PARTY_NOTICES.md
RELEASE_NOTES.md
KNOWN_LIMITATIONS.md
COMPLETION_REPORT.md
```

## 28.4 Signing

- assinar executáveis, helpers e installers com Authenticode quando certificado estiver configurado;
- usar timestamp confiável;
- verificar assinatura no pipeline;
- se certificado não estiver disponível, produzir build de desenvolvimento/RC claramente identificado como não assinado;
- nunca declarar distribuição pública production-ready sem resolver signing gate.

## 28.5 Installer

- instalar main app e helpers corretos;
- main app não requer admin para abrir;
- helper eleva somente ao iniciar raw scan;
- uninstall não apaga sessões do usuário sem consentimento;
- opção de atalhos;
- WebView2 bootstrap conforme política do Tauri/Windows;
- upgrade preserva session migrations;
- rollback/teste de upgrade.

## 28.6 Updates

V1 não deve exigir network. Atualização automática, se implementada:

- opt-in;
- signed manifests;
- assinatura verificada;
- rollback;
- sem telemetry;
- desabilitada até threat model e E2E completos.

Manual signed installer é aceitável para a primeira release.

---

# 29. CRITÉRIOS DE ACEITE DE PRODUTO

Cada cenário deve ter test ID e evidence.

## AC-001 — Seleção de fonte

Dado um PC com discos internos e USB, quando o usuário abre Sources, então os devices são distinguidos por identidade, capacidade, volumes e tipo, sem confundir letras do mesmo disco.

## AC-002 — Fonte somente-leitura

Ao iniciar scan raw, o broker abre a fonte sem write access. Um teste de arquitetura e um teste runtime comprovam que não existe operação de escrita disponível.

## AC-003 — Mesmo disco físico

Quando a pasta de trabalho ou destino está em outra letra, mas no mesmo disco físico, o app identifica o risco e aplica a política correta.

## AC-004 — NTFS completo

Em fixture NTFS com arquivo apagado não sobrescrito, o app encontra o candidate e a extração tem SHA-256 exatamente igual ao expected manifest.

## AC-005 — NTFS parcial

Em fixture com parte sobrescrita, o app não rotula como completo, registra missing/conflicting ranges e exporta partial conforme a política escolhida.

## AC-006 — Pasta apagada

Ao selecionar somente um arquivo dentro de árvore apagada, o restore cria apenas os ancestors necessários e o arquivo selecionado.

## AC-007 — Pasta vazia

Ao selecionar somente uma pasta apagada, cria a pasta vazia e nenhum antigo descendente.

## AC-008 — Multi-extension filter

O usuário seleciona duas ou mais extensões encontradas e a tabela mostra a união correta, mantendo seleção anterior.

## AC-009 — Qualquer extensão via metadata

Um arquivo com extensão desconhecida, mas extents válidos no metadata, pode ser extraído sem precisar de carver específico.

## AC-010 — Carved candidate

Arquivo sem metadata, mas com formato baseline, é encontrado por assinatura, recebe nome gerado, offset e path confidence baixa.

## AC-011 — False positive

Header isolado sem estrutura válida não aparece como “Excelente”; validator reduz/rejeita o candidate.

## AC-012 — SSD/TRIM

Quando a fonte indica SSD/TRIM e o conteúdo está zeroed, a UI explica a limitação e não promete recuperação.

## AC-013 — BitLocker locked

Volume bloqueado é detectado e não é escaneado. O app orienta unlock pelo Windows sem solicitar/store key.

## AC-014 — Pausa/retomada

Scan pausado e app fechado retoma do checkpoint sem duplicar candidates ou corromper DB.

## AC-015 — Disconnect/reconnect

Device removido pausa. Device diferente na mesma letra é rejeitado. A mesma identity reconectada permite retomada.

## AC-016 — Bad sectors

Erros de leitura são registrados por range, o scan prossegue quando seguro e candidates afetados recebem status adequado.

## AC-017 — Preview isolation

Um validator que crasha é encerrado no worker e não derruba o app nem eleva código.

## AC-018 — Malicious archive

Path traversal e decompression bomb são bloqueados por limites e sandbox.

## AC-019 — Collision

Arquivo ativo no destino nunca é substituído silenciosamente; a política padrão produz novo nome.

## AC-020 — Same-disk restore

No modo Guiado, é bloqueado. No modo Avançado, somente após scan, confirmação digitada e registro no manifesto.

## AC-021 — Million-row UI

Com 1 milhão de candidates, filtros, scroll e seleção permanecem funcionais dentro das metas definidas e sem carregar todas as rows na UI.

## AC-022 — Accessibility

Fluxo principal pode ser concluído somente com teclado, sem violações automatizadas critical/serious.

## AC-023 — Localization

Nenhuma string principal fica hard-coded; pt-BR e en-US completos, com fallback definido.

## AC-024 — Session export

`.umscan` exporta/importa versão válida e rejeita arquivo malformado sem crash/path traversal.

## AC-025 — Restore verification

Arquivo restaurado tem write hash coerente, manifesto e status final; temp files não são confundidos com concluídos.

## AC-026 — No network

O aplicativo executa scan, preview suportado e restore com network desabilitada.

## AC-027 — Source image

Imagem `.dd/.img` funciona sem UAC e passa pelos mesmos parsers.

## AC-028 — FAT/exFAT

Fixtures completas e parciais retornam bytes/score esperados e não entram em loop com cadeia corrompida.

## AC-029 — Original path sanitation

Path malicioso nunca escapa do destino, mesmo com reparse point/race testado.

## AC-030 — Evidence

Cada requisito Must possui link para test e resultado na traceability matrix.

---

# 30. DEFINITION OF DONE

O projeto somente poderá ser declarado concluído quando todos os itens aplicáveis abaixo estiverem atendidos.

## 30.1 Código

- [ ] build Release funciona em Windows x64;
- [ ] main app unelevated;
- [ ] broker read-only;
- [ ] worker sandboxed/restricted;
- [ ] NTFS/FAT/exFAT production paths completos;
- [ ] carving baseline completo;
- [ ] restore transacional;
- [ ] sessões retomáveis;
- [ ] UI final, não protótipo;
- [ ] i18n completa;
- [ ] sem TODO/FIXME bloqueador;
- [ ] sem stub em production path;
- [ ] sem warning de compilação;
- [ ] `unsafe` isolado e revisado.

## 30.2 Testes

- [ ] unit tests passam;
- [ ] property tests passam;
- [ ] integration/hash fixtures passam;
- [ ] E2E Tauri passa;
- [ ] accessibility passa;
- [ ] security tests passam;
- [ ] fuzz smoke não encontra crash;
- [ ] regressions salvas;
- [ ] performance evidence gerada;
- [ ] external corpora executada ou ausência justificada;
- [ ] no destructive real-disk test.

## 30.3 Segurança

- [ ] threat model revisado;
- [ ] IPC review;
- [ ] path containment review;
- [ ] source write invariant comprovado;
- [ ] dependency audit sem critical/high não aceito;
- [ ] license review;
- [ ] SBOM;
- [ ] CSP/capabilities audit;
- [ ] secrets scan;
- [ ] recovered content não executado.

## 30.4 Documentação

- [ ] specs completas;
- [ ] ADRs;
- [ ] traceability 100% para Must;
- [ ] risk register;
- [ ] README e README.pt-BR;
- [ ] build/dev/test guide;
- [ ] recovery limitations;
- [ ] privacy/security;
- [ ] release notes;
- [ ] completion report.

## 30.5 Release

- [ ] EXE installer;
- [ ] MSI;
- [ ] portable package;
- [ ] checksums;
- [ ] signing verificado ou release marcada claramente como unsigned RC;
- [ ] clean-machine installation test;
- [ ] upgrade/uninstall test;
- [ ] artifacts reproduzíveis/documentados.

## 30.6 Honestidade de conclusão

Se qualquer item obrigatório falhar:

- não usar a palavra “concluído” sem qualificação;
- registrar bloqueio e evidência;
- entregar o máximo funcional já validado;
- não mascarar falha com mock, skip ou screenshot;
- não afirmar suporte a formato não testado.

---

# 31. SAÍDA FINAL EXIGIDA DO CODEX

Ao final, responder uma única vez com:

1. resumo do que foi construído;
2. arquitetura final;
3. agents/subagents usados e respectivos resultados;
4. skills, MCP servers e plugins realmente usados;
5. lista de arquivos/documentos principais;
6. comandos de build e execução;
7. comandos de teste e resultados reais;
8. artefatos de release e hashes;
9. segurança e threat model;
10. requisitos/acceptance results;
11. limitações honestas;
12. qualquer gate não atendido;
13. caminho para `docs/completion-report.md`.

Também gerar `docs/completion-report.md` com:

```text
Commit/build ID
Environment
Toolchain versions
Feature matrix
Requirement status
Test commands
Test results
Coverage
Fuzz duration/results
Performance results
Security scan results
Dependency/license results
Installer artifacts/hashes
Known limitations
Open risks
Reproduction steps
```

Não encerrar com “o restante pode ser feito depois” quando ainda houver requisito obrigatório implementável no ambiente atual.

---

# 32. EXEMPLO DE CONTEÚDO DO AGENTS.MD

O Codex deve adaptar, não copiar cegamente:

```markdown
# AGENTS.md — Undelete Master

## Non-negotiable invariants
- Never write to a scan source. Raw source handles are read-only.
- Never run destructive tests against real disks. Use only allowlisted fixtures or disposable VHDs.
- The desktop UI stays unelevated; elevation is restricted to the read-only broker.
- Recovered files are untrusted and must never be executed or previewed in a privileged process.
- No production TODOs, stubs, fake data, skipped critical tests, or unsupported completion claims.

## Required commands before completion
- cargo fmt --check
- cargo clippy --workspace --all-targets --all-features -- -D warnings
- cargo test --workspace --all-features
- frontend lint/typecheck/test commands
- Tauri release build
- E2E and security smoke tests
- dependency/license/SBOM checks

## Change discipline
- Update requirements and traceability with behavior changes.
- Add an ADR for architecture/security deviations.
- Every bug fix needs a regression test.
- Unsafe Windows FFI belongs only in the audited io-windows boundary.
```

---

# 33. PRODUTO: DECISÕES FINAIS JÁ RESOLVIDAS

Para evitar perguntas desnecessárias, considere estas decisões fechadas:

- nome: **Undelete Master**;
- Windows primeiro;
- Tauri 2 + React/TypeScript + Rust;
- pt-BR padrão e en-US;
- local/offline por padrão;
- sem login;
- sem telemetria;
- scanner somente-leitura;
- helper elevado separado;
- destino recomendado em outro disco físico;
- restore no local original permitido somente com salvaguardas;
- restauração granular, nunca restauração silenciosa de todos os antigos conteúdos de uma pasta;
- metadata recovery para qualquer extensão;
- carving somente para formatos reconhecíveis/configurados;
- score explicável e sem garantia falsa;
- NTFS, FAT e exFAT em produção;
- ReFS experimental/carving-only até prova suficiente;
- nenhum kernel driver na V1;
- raw images suportadas;
- UI elegante, animada, acessível e profissional;
- documentação spec-driven obrigatória;
- agents/skills/MCP úteis devem ser usados e auditados;
- nenhum teste destrutivo em hardware real;
- não declarar concluído sem evidências.

---

# 34. REFERÊNCIAS TÉCNICAS PRIMÁRIAS PARA O CODEX

Durante a implementação, consulte as versões atuais das fontes oficiais e registre a data de acesso. Esta lista é ponto de partida, não substitui verificação atualizada.

## Microsoft / Windows

- CreateFile — physical disks and volumes:  
  `https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-createfilew`
- Master File Table:  
  `https://learn.microsoft.com/windows/win32/fileio/master-file-table`
- File management control codes / NTFS control codes:  
  `https://learn.microsoft.com/windows/win32/fileio/file-management-control-codes`
- `FSCTL_GET_NTFS_VOLUME_DATA`:  
  `https://learn.microsoft.com/windows/win32/api/winioctl/ni-winioctl-fsctl_get_ntfs_volume_data`
- `FSCTL_GET_NTFS_FILE_RECORD`:  
  `https://learn.microsoft.com/windows/win32/api/winioctl/ni-winioctl-fsctl_get_ntfs_file_record`
- `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`:  
  `https://learn.microsoft.com/windows/win32/api/winioctl/ni-winioctl-ioctl_volume_get_volume_disk_extents`
- TRIM/Unmap behavior:  
  `https://learn.microsoft.com/windows/win32/w8cookbook/new-api-allows-apps-to-send--trim-and-unmap--hints-to-storage-media`
- BitLocker operations guide:  
  `https://learn.microsoft.com/windows/security/operating-system-security/data-protection/bitlocker/operations-guide`
- `manage-bde -status`:  
  `https://learn.microsoft.com/windows-server/administration/windows-commands/manage-bde-status`
- ReFS overview:  
  `https://learn.microsoft.com/windows-server/storage/refs/refs-overview`

## Tauri

- Architecture:  
  `https://v2.tauri.app/concept/architecture/`
- Security:  
  `https://v2.tauri.app/security/`
- Capabilities:  
  `https://v2.tauri.app/security/capabilities/`
- WebDriver testing:  
  `https://v2.tauri.app/develop/tests/webdriver/`
- Windows installer:  
  `https://v2.tauri.app/distribute/windows-installer/`

## OpenAI / Codex

- Customization overview:  
  `https://developers.openai.com/codex/customization/overview`
- AGENTS.md:  
  `https://developers.openai.com/codex/guides/agents-md`
- Skills:  
  `https://developers.openai.com/codex/skills`
- MCP:  
  `https://developers.openai.com/codex/mcp`
- Subagents:  
  `https://developers.openai.com/codex/subagents`
- Plugins:  
  `https://developers.openai.com/codex/plugins`

## MCP oficiais úteis

- Microsoft Learn MCP Server:  
  `https://github.com/microsoftdocs/mcp`  
  endpoint documentado: `https://learn.microsoft.com/api/mcp`
- GitHub MCP Server:  
  `https://github.com/github/github-mcp-server`

## Testes forenses independentes

- NIST CFReDS:  
  `https://cfreds.nist.gov/`
- NIST — Standardization of File Recovery Classification and Authentication:  
  `https://csrc.nist.gov/pubs/journal/2019/12/standardization-of-file-recovery-classification-au/final`
- Digital Corpora:  
  `https://digitalcorpora.org/`
- The Sleuth Kit:  
  `https://github.com/sleuthkit/sleuthkit`

---

# 35. ORDEM DE INÍCIO OBRIGATÓRIA PARA O CODEX

Ao receber este documento, comece exatamente assim:

1. leia o documento inteiro;
2. inspecione o repo e preserve trabalho existente;
3. inventarie ambiente, agents, skills, plugins, MCP e permissions;
4. consulte fontes oficiais atualizadas para APIs críticas;
5. crie specs, ADRs, threat model, risk register e traceability;
6. faça review paralelo por especialistas;
7. estabeleça fixture builder e safety guards antes de raw device tests;
8. implemente o primeiro vertical slice end-to-end sobre imagem sintética;
9. expanda para broker Windows e filesystems;
10. implemente UI, restore, validators, scale e release;
11. execute todos os gates;
12. faça revisão independente final;
13. gere artifacts e completion report;
14. só então declare o status real.

**Não pare após criar o plano. Não pare após gerar telas. Não pare após compilar. Conclua somente com comportamento comprovado por testes e evidências.**

---

# 36. NOTA FINAL DE INTEGRIDADE DO PRODUTO

O diferencial do Undelete Master não será afirmar que faz milagres. Será fazer uma busca profunda, preservar a fonte, reunir múltiplas evidências, recuperar o máximo tecnicamente possível e dizer com clareza o que está intacto, o que está parcial e o que já não pode ser reconstruído.

A primeira versão deve ser bonita e simples na superfície, mas rigorosa no núcleo. Toda decisão deve favorecer, nesta ordem:

1. preservação dos dados;
2. honestidade do resultado;
3. segurança;
4. correção;
5. testabilidade;
6. usabilidade;
7. desempenho;
8. extensibilidade.

