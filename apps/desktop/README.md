# Undelete Master — desktop para volumes conectados

Aplicativo Tauri 2 + React que detecta os armazenamentos locais conectados ao
Windows e usa o scanner Rust real do workspace.

## Escopo atual

- inventaria volumes locais montados sem elevação e os apresenta em grupos
  lógicos honestos;
- permite escolher um volume compatível para análise;
- em NTFS, permite selecionar uma pasta opcional pelo diálogo nativo;
- mantém caminhos nativos fora do WebView e envia somente identidades opacas;
- mantém a interface sem elevação e eleva apenas o broker temporário de leitura;
- apresenta candidatos reais em um workspace com busca explícita, facets
  dinâmicas, filtros, ordenação, páginas limitadas por cursor e seleção nativa;
- restaura somente candidatos selecionados para uma pasta NTFS autorizada em
  outro disco físico comprovado, preservando a árvore e renomeando colisões sem
  substituir arquivos existentes;
- mostra progresso e cancelamento vindos do trabalho nativo, além de resultado
  terminal, sidecar parcial, manifesto e abertura somente da pasta de destino;
- falha de forma fechada quando aberto apenas no navegador;
- não possui seletor de imagem, dados mock, fallback demonstrativo ou progresso
  inventado.

Os cartões iniciais não alegam conhecer o número do disco físico. A origem
efetivamente aberta é um volume montado e compatível, nunca `PhysicalDriveN`;
somente o broker elevado resolve e valida os extents físicos. O filtro de pasta
analisa os metadados do volume e só apresenta candidatos cuja pertença à pasta
NTFS pode ser comprovada.

A restauração baseada em metadados independe da extensão quando existe um plano
de conteúdo utilizável. O carving profundo continua orientado por plugins e,
hoje, cobre somente JPEG contíguo em espaço NTFS comprovadamente livre.

Esta versão não oferece prévia, execução de conteúdo, exFAT, ACL/EFS/ADS,
sessões persistentes, retomada de trabalhos, layouts de restauração alternativos,
unidade desmontada, volume multidisco ou varredura do disco físico inteiro. A
análise continua sem percentual, ETA ou cancelamento; o progresso e o
cancelamento implementados pertencem apenas ao trabalho de restauração. A
aceitação empacotada com origem e destino em discos físicos reais diferentes
permanece não verificada.

O contrato e os limites do fluxo estão em
[SDD-020](../../docs/specs/020-actionable-results-and-transactional-restore.md)
e [ADR-0025](../../docs/adr/0025-native-result-query-and-selection-authority.md)
até [ADR-0028](../../docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md).

## Comandos

```bash
pnpm install
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm desktop:dev
pnpm desktop:build
```

`desktop:dev` e `desktop:build` compilam o broker Windows irmão antes do
desktop. O build Vite isolado valida somente a interface fechada do navegador;
inventário e análise funcionam exclusivamente dentro do runtime Tauri.
