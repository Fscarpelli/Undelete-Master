# Undelete Master — desktop para volumes conectados

Aplicativo Tauri 2 + React que detecta armazenamentos locais reais no Windows,
mantém a interface sem elevação e usa o scanner Rust do workspace por meio de
um broker temporário, elevado e estritamente somente leitura.

Documentação do produto: [portal](../../docs/README.md) ·
[catálogo de funcionalidades](../../docs/FEATURES.md) ·
[galeria de capturas reais](../../docs/screenshots/README.md)

> A galeria separa uma captura do executável atual das capturas históricas que
> registram problemas e não devem ser apresentadas como imagens finais.

## Pré-requisitos no Windows

- Windows 10 22H2 ou Windows 11 x64 e Microsoft Edge WebView2.
- Volume local montado em NTFS ou FAT12/16/32 para análise.
- Aprovação do UAC somente ao iniciar a leitura protegida. O processo do
  desktop não deve ser executado como Administrador.
- Para recuperar, pasta NTFS local e gravável em exatamente um disco físico
  comprovadamente diferente do disco ou discos que sustentam a origem.

Antes de analisar, reduza ao mínimo o uso da unidade de origem. O app não grava
nela, mas outras gravações do sistema podem sobrescrever dados apagados.

## Uso

1. Mantenha `undelete-master-desktop.exe` e
   `undelete-master-broker.exe` juntos na mesma pasta e abra somente o desktop.
2. Em **Análise**, atualize o inventário e selecione um volume compatível.
3. Para NTFS, opcionalmente selecione uma pasta pelo diálogo nativo. O WebView
   recebe apenas uma identidade opaca, nunca o caminho escolhido.
4. Escolha **Metadados** ou, somente para o volume NTFS inteiro, **Profunda para
   JPEG**. A segunda opção examina apenas regiões comprovadamente livres.
5. Inicie a análise e aprove o UAC do broker. Na fase mensurável da MFT, a
   interface mostra registros concluídos/total, percentual real, tempo decorrido
   e ETA baseada no ritmo. Fases sem total confiável usam indicador
   indeterminado; não existe percentual fictício.
6. Nos resultados, pesquise, filtre e ordene; marque uma linha pelo checkbox ou
   use a seleção em massa. A seleção nativa permanece ao mudar página, consulta,
   filtro ou ordenação durante o processo atual.
7. Clique em **Recuperar selecionados**, autorize uma pasta NTFS em outro disco
   físico, revise eventual consentimento de melhor esforço e acompanhe o
   progresso real por itens/bytes até o manifesto terminal.

O app nunca abre, pré-visualiza ou executa o conteúdo recuperado. A ação final
abre somente a pasta do trabalho concluído.

## Capacidades atuais

- inventário real de volumes locais montados sem UAC;
- metadados NTFS e FAT12/16/32, com cobertura e parcialidade explícitas;
- escopo opcional de pasta NTFS por identidade e ancestralidade comprovadas;
- carving limitado de JPEG contíguo e estruturalmente válido no volume NTFS
  inteiro, somente em regiões livres segundo um `$Bitmap` validado;
- fases nativas, cronômetro, percentual e ETA da MFT quando mensurável;
- busca, facets de extensão, filtros de evidência, ordenação estável,
  paginação por cursor, checkbox individual e seleção em massa;
- restauração transacional dos candidatos elegíveis, preservando a árvore,
  sanitizando caminhos e renomeando colisões sem substituir arquivos;
- consentimento explícito para resultado parcial, sidecar de faixas preenchidas
  com zero, progresso/cancelamento nativos da recuperação e manifesto JSON;
- falha fechada no navegador e ausência de imagem picker, mock, demonstração,
  resultado ou progresso fabricado.

Um arquivo de qualquer extensão pode ser candidato por metadados se existir um
plano de conteúdo utilizável. Isso não equivale a carving de todos os formatos
nem garante que os bytes ainda estejam intactos.

## Limites atuais

Não há suporte a exFAT, ReFS, carving fragmentado ou de todos os formatos,
análise RAW de sistema danificado, partição desmontada, volume multidisco,
rede, unidade virtual/incerta ou varredura do disco físico inteiro. O app não
possui prévia, sessões persistentes, retomada após reiniciar, pausa/retomada ou
cancelamento cooperativo do scan, restauração de ACL/EFS/ADS, layout alternativo
de destino ou instalador assinado.

Somente a enumeração MFT possui total confiável para percentual e ETA; namespace,
classificação e deep JPEG podem permanecer indeterminados. A recuperação tem
progresso e cancelamento próprios. A aceitação empacotada de uma recuperação
governada entre dois discos físicos reais ainda não foi comprovada.

Veja a lista completa em
[limitações conhecidas](../../docs/specs/015-known-limitations.md).

## Configurações e Ajuda

- **Configurações:** idioma `pt-BR`/`en-US`, tema do sistema/escuro/claro e
  movimento reduzido. A persistência é local e a tela avisa quando falha.
- **Ajuda:** modos disponíveis, garantia somente leitura, motivo do UAC, regras
  do destino, limites e interpretação do filtro de pasta.

As preferências visuais não alteram o scanner, a pontuação nem as evidências.

## Solução de problemas

- **`UAC_CANCELLED`:** nenhuma origem foi aberta; inicie novamente e aprove
  somente o broker irmão confiável.
- **`SOURCE_GONE`:** a identidade escolhida desapareceu. Reconecte, aguarde a
  montagem pelo Windows, atualize a lista e selecione de novo.
- **`SOURCE_IO`:** uma leitura ou consulta somente leitura falhou apesar de a
  identidade continuar presente. Verifique cabo, case USB e estado da unidade.
  Bridges USB antigos recebem fallback somente para os códigos específicos de
  consulta de alinhamento não suportada; falhas reais de I/O continuam fechadas.
- **`SCAN_INTERNAL`:** o scanner parou sem fabricar resultados. Atualize e tente
  uma vez; se repetir, registre código, modo, filesystem e avisos não sensíveis.
- **Destino recusado:** outra letra de unidade pode estar no mesmo disco físico.
  Escolha pasta NTFS local, gravável, de disco único e comprovadamente diferente.
- **Antivírus:** executáveis locais atuais não são assinados. Não desative a
  proteção permanentemente nem declare falso positivo sem análise. Autorize
  somente artefato compilado por você ou verificado de modo independente.

## Desenvolvimento

Na raiz do repositório, instale as dependências do frontend e execute os gates:

```powershell
Set-Location apps/desktop
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Para executar ou gerar os dois binários irmãos:

```powershell
pnpm desktop:dev
pnpm desktop:build
```

`desktop:dev` e `desktop:build` compilam primeiro o broker Windows. O Vite
isolado valida somente a interface fechada do navegador; inventário, scan e
recuperação funcionam exclusivamente no runtime Tauri.

O contrato e os limites estão em
[SDD-018](../../docs/specs/018-windows-volume-and-folder-scan.md),
[SDD-019](../../docs/specs/019-ntfs-coverage-and-jpeg-deep-scan.md),
[SDD-020](../../docs/specs/020-actionable-results-and-transactional-restore.md)
e [ADR-0023](../../docs/adr/0023-windows-read-only-broker-and-folder-scope.md)
até [ADR-0028](../../docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md).

## Branch canônica

`main` é a única branch canônica publicada no GitHub.
