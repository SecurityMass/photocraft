---
title: Como instalar no Mac
description: Instalar o app, dar a ele um nome e um ícone, autorizar a primeira abertura e conferir o checklist de teste.
---

Este app serve para uma coisa só: abrir uma imagem, desenhar por cima à mão livre e exportar o resultado em PNG. Siga os passos na ordem. Você não precisa de conhecimento técnico.

## O que você precisa ter

- O arquivo `.dmg` e o arquivo `personalize.sh`, que vêm juntos no download.
- O nome que o app vai ter e a imagem que vai virar o ícone. A pessoa que preparou o app envia isso para você à parte, junto com o comando pronto.

Coloque `personalize.sh` e a imagem do ícone na mesma pasta, por exemplo em **Transferências** (Downloads).

## 1. Baixe o artefato e abra o `.dmg`

1. Baixe o arquivo pelo link que você recebeu. Ele vem compactado em `.zip`.
2. Dê dois cliques no `.zip` para abrir.
3. Dê dois cliques no arquivo que termina em `.dmg`. Uma janela se abre com o app `teste` e um atalho chamado **Applications**.

## 2. Arraste o app para Aplicativos

1. Arraste o ícone `teste` para o atalho **Applications**.
2. Espere a cópia terminar.
3. Feche a janela do `.dmg` e ejete o disco `teste` na barra lateral do Finder.

## 3. Rode o script no Terminal

1. Abra o **Terminal**. Ele fica em Aplicativos, na pasta Utilitários.
2. Digite `cd ` com um espaço no final, arraste para a janela do Terminal a pasta onde estão `personalize.sh` e a imagem, e aperte Enter.
3. Cole o comando que você recebeu e aperte Enter. Ele tem este formato:

   ```
   bash personalize.sh <nome> <imagem>
   ```

4. Ao terminar, o Terminal mostra uma linha começando com `Pronto:`. O app agora tem o novo nome e o novo ícone na pasta Aplicativos.

Se aparecer uma mensagem começando com `erro:`, ela diz o que faltou. As causas mais comuns são a imagem estar em outra pasta ou o app não ter sido arrastado para Aplicativos.

## 4. Autorize a primeira abertura

O Mac bloqueia a primeira abertura porque o app não é assinado por uma conta de desenvolvedor Apple. A autorização é manual e só precisa ser feita uma vez.

1. Na pasta Aplicativos, dê dois cliques no app. Uma mensagem avisa que ele não pode ser aberto. Clique em **Concluído** ou **OK**.
2. Abra **Ajustes do Sistema** e entre em **Privacidade e Segurança**.
3. Desça até a parte de **Segurança**. Clique em **Abrir Mesmo Assim**, ao lado do nome do app.
4. Confirme e digite a senha do Mac, se ela for pedida.

## Como usar

- **Abrir imagem:** escolhe a imagem. Você também pode soltar a imagem na janela do app.
- **Caneta** e **Borracha:** a borracha só apaga o que você desenhou, nunca a imagem.
- **Cor** e **Espessura:** valem para o próximo traço.
- **Desfazer** e **Refazer:** também funcionam com `Cmd+Z` e `Shift+Cmd+Z`.
- **Zoom:** use os botões `−` e `+`, ou `Cmd+=`, `Cmd+-` e `Cmd+0`.
- **Mover vista:** arraste a imagem. Segurar a barra de espaço faz o mesmo sem trocar de ferramenta.
- **Corrigir uma forma:** desenhe uma linha, um círculo, uma elipse, um retângulo ou um triângulo, e no fim do traço segure parado por meio segundo, sem soltar. O traço vira a forma perfeita. Um `Cmd+Z` volta ao traço à mão. Se o app não reconhecer a forma, o traço fica como está.
- **Exportar PNG:** salva a imagem com o desenho, no tamanho original da imagem.

## Checklist de teste

Marque cada item e anote o que não funcionou. Antes de rodar o script:

- [ ] Depois de arrastar para Aplicativos, o app aparece como `teste`, com o ícone original do PhotoCraft.

Depois de rodar o script:

- [ ] O app aparece com o nome escolhido e a imagem escolhida, no Finder e no Dock.
- [ ] O nome escolhido aparece no título da janela.
- [ ] O app abre depois da autorização em **Abrir Mesmo Assim**.
- [ ] O mouse desenha.
- [ ] A mesa digitalizadora desenha, e a espessura varia com a pressão da caneta.
- [ ] Segurar parado por cerca de meio segundo corrige uma linha.
- [ ] Segurar parado corrige um círculo ou uma elipse.
- [ ] Segurar parado corrige um retângulo.
- [ ] Segurar parado corrige um triângulo.
- [ ] Soltar sem segurar deixa o traço como foi desenhado.
- [ ] `Cmd+Z` desfaz a correção e volta ao traço original. Outro `Cmd+Z` remove o traço.
- [ ] A borracha não apaga a imagem.
- [ ] O PNG exportado abre corretamente, na resolução original da imagem.
- [ ] O menu do Mac mostra só as funções do app: abrir, exportar, desfazer, refazer, zoom e sair.
