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
- mostra candidatos reais em páginas limitadas, com avisos e resultados de
  ancestralidade desconhecida separados;
- falha de forma fechada quando aberto apenas no navegador;
- não possui seletor de imagem, dados mock, fallback demonstrativo ou progresso
  inventado.

Os cartões iniciais não alegam conhecer o número do disco físico. A origem
efetivamente aberta é um volume montado e compatível, nunca `PhysicalDriveN`;
somente o broker elevado resolve e valida os extents físicos. O filtro de pasta
analisa os metadados do volume e só apresenta candidatos cuja pertença à pasta
NTFS pode ser comprovada.

Esta versão não restaura arquivos e não oferece prévia, carving, exFAT,
sessões persistentes, pausa, cancelamento, unidade desmontada, volume multidisco
ou varredura do disco físico inteiro.

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
