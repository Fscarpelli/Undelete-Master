# Undelete Master — desktop real-only

Aplicativo Tauri 2 + React para analisar imagens locais comuns usando o
scanner Rust real do workspace.

## Escopo atual

- seleciona `.img`, `.dd`, `.raw` ou `.bin` pelo diálogo nativo controlado pelo
  Rust;
- mantém o caminho fora do WebView;
- abre a origem somente para leitura;
- mostra o resumo real de partições, volumes, contagens de metadados candidatos
  e avisos;
- falha de forma fechada quando aberto apenas no navegador.

Esta versão não acessa discos físicos, não restaura arquivos e não oferece
prévia, sessões, carving, exFAT, percentual, pausa ou cancelamento de uma
análise em andamento. Recursos sem backend seguro não aparecem na interface.

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

O build Vite isolado serve para validar a interface fechada do navegador. A
análise de arquivos funciona exclusivamente dentro do runtime Tauri.
