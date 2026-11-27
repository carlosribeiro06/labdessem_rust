# Auditoria da montagem do problema de otimização — labdessem v0.12.0

| | |
|---|---|
| **Data** | 05/11/2026 |
| **Versão auditada** | branch `develop`, commit `3d72ec6` (v0.12.0) |
| **Caso de referência** | `examples/24Barras` |
| **Escopo** | Leitura dos dados → montagem de cada problema de otimização (LP, MILP, LP-FIXED, LP-CALC-CMO) → solver → escrita dos resultados, nos dois modos de execução e nas 384 combinações de flags |
| **O que foi alterado no repositório** | **nada**. Este arquivo não está versionado; os testes de prova estão em patches à parte (Apêndice B). |

---

## 0. Como ler este relatório

O relatório é longo porque tenta ser autoexplicativo. Uma sugestão de leitura:

1. **Seção 1, resumo executivo (5 minutos).** Os problemas que realmente importam e o que está correto.
2. **Seção 2, como o labdessem monta o problema.** Leia se não estiver com o fluxo do código fresco na cabeça. Os achados citam essas etapas o tempo todo.
3. **Seção 4, tabela de todos os achados** por severidade, com links para a explicação.
4. **Seções 5 a 10, explicação detalhada por domínio.** Cada achado importante tem:
   - "em uma frase";
   - como deveria ser;
   - o que acontece hoje, com links clicáveis para o código;
   - evidência numérica;
   - consequência;
   - correção sugerida (**não implementada**);
   - resultado do teste de prova.
5. **Seção 12, os testes de prova.** O que foi demonstrado por programa, com os números das execuções e como você mesmo pode rodar tudo.
6. **Seção 13, as decisões que dependem de você.** Vários problemas não têm uma correção única; a resposta depende de qual metodologia o projeto quer seguir.
7. **Seção 14, o que foi verificado e está correto.**

Os links para o código (por exemplo [conversion.rs:52](../../crates/labdessem-solver/src/conversion.rs#L52)) abrem o arquivo na linha certa no VSCode.

**Notação.** Quando um mesmo defeito foi achado por mais de um agente, ele aparece com os sinônimos: "RD-01 (= NU-02 = MX-01)". Os prefixos indicam quem achou:

| Prefixo | Domínio |
|---|---|
| TH | térmicas |
| HI | hidráulica |
| FP / EM | FPHA / EngMax |
| RD | rede |
| NU | núcleo |
| MX | matriz de configurações |

Um `R` no número (TH-R01) indica achado novo do **revisor**.

---

## 1. Resumo executivo

### 1.1 A resposta curta

**O fluxo leitura → montagem → solver → saída não tem quebras estruturais.** O que o solver resolve é o que os CSVs mostram:

- o objetivo impresso é recomposto a partir do despacho impresso com erro de **0,0002 R$**;
- os fluxos impressos são recalculados de forma independente com erro de **0,00005 MW**;
- os balanços de energia e de água fecham em todas as 224 execuções que terminaram.

Isso foi verificado nas **384 combinações** de flags.

**Mas há defeitos reais em quatro lugares**, alguns graves:

1. **Dados que entram errado sem aviso.** O leitor aceita e descarta em silêncio dados inconsistentes, e o próprio caso de exemplo tem dois desses problemas.
2. **Detalhes da formulação**, que só aparecem em certas configurações.
3. **Preços (CMB/CMO) com rede.** Um problema de sinal herdado da biblioteca `good_lp` e um problema de escala.
4. **Colunas secundárias da saída**: status ligado/desligado e rótulos de período.

### 1.2 Os problemas que mais importam

| # | Problema | Quando aparece | Por que importa |
|---|---|---|---|
| 1 | **RD-01**: o CMB/CMO sai com a parcela de congestionamento **invertida** quando uma linha fica presa no limite inferior. | `rede=1` com rede congestionada | **Preço errado sem nenhum aviso.** O mesmo problema físico dá CMO 674 ou 69 R$/MWh conforme a linha tenha sido declarada "De→Para" ou "Para→De". A causa é a `good_lp` 1.15.0, que guarda `a ≥ b` como `−a ≤ −b`, e o labdessem lê o dual sem desfazer a inversão. |
| 2 | **HI-01**: no exemplo, as afluências vão para as usinas erradas e **88 % da água some**. | todas | O `OPER_VAZAO.csv` usa os códigos ONS antigos (1/2/4/6) e o `CAD_UHE.csv` foi renumerado para 0..3. O leitor casa só por código e **descarta em silêncio** o que não casa. Os resultados versionados do exemplo estão contaminados. |
| 3 | **TH-01**: com `TON_Residual=1`, o **custo de partida some** da função objetivo. | `UCT=1` + `TON_Residual=1` | A conversão faz `objetivo[col] = coef` em vez de `+=`. O segundo termo da mesma variável (o residual, quase sempre 0) apaga o primeiro (o custo de partida). Partidas viram gratuitas para o MILP. |
| 4 | **FP-02**: FPHA calculada aplica o fator de correção **α duas vezes**. | `FPHA="computed"` | Envelope de geração α²·FPHA em vez de α·FPHA. Um teste existente trava esse comportamento. |
| 5 | **HI-03**: FPHA calculada sem geometria de uma usina, e a usina **gera sem gastar água**. | `FPHA="computed"` | Sem o arquivo de geometria, **todas** as usinas geram de graça, e a execução termina com sucesso. A regra que pegaria isso existe, mas roda antes de a FPHA ser ligada. |
| 6 | **RD-02**: o CMB não é dividido pela duração do período. | `rede=1` (sempre) | Com períodos de 0,5 h, o CMB impresso é **metade** do valor certo. |
| 7 | **RD-04**: duas bases de demanda. O balanço usa `OPER_SBM`, a rede usa `OPER_CARGA_BARRA`. | `rede=1` | No exemplo a diferença é de ~270 MW (20 %), absorvida pela barra swing como uma carga fantasma. Os fluxos ficam distorcidos em até 170 MW. |
| 8 | **TH-03**: unidade que começa no meio de um desligamento é forçada a **repetir a rampa**. | `UCT=1` + `RdownInic=1` | Gera 255 MW mesmo com CVU de 100 mil R$/MWh. Num sub-caso, liga sem pagar partida. |
| 9 | **NU-R02 / NU-R01**: FCF aplicada sobre (V − Vmin) em vez de V absoluto, e unidade dos cortes indefinida. | `FCF=1` | No exemplo a FCF é **inerte**: 20 cortes idênticos, provavelmente 1000× pequenos. Com uma FCF real, o referencial muda qual corte fica ativo e, com ele, as decisões. |

A **seção 4** lista todos os achados: 2 CRÍTICOS, 8 ALTOS, 16 MÉDIOS e cerca de 50 BAIXOS. Vários BAIXOS são defeitos **latentes**, código errado que hoje ninguém usa. O principal deles é o cálculo de offsets de linhas (NU-10), que está errado depois do 2º bloco de restrições.

### 1.3 O que está correto (e importa saber)

- **O CMO sem rede e o valor da água estão certos.** São duais de igualdades, que a `good_lp` não inverte.
- **O CMB está certo quando a linha está no limite superior.** O problema é só o limite inferior.
- **A leitura da solução por posição está certa** para tudo o que é impresso: geração, volumes, fluxos, PiDemanda, PiBalHidr.
- **A lógica de UC térmico está certa**, salvo os dois casos de condição inicial (TH-03, TH-09): trajetórias, tempos mínimos, Pmin/Pmax.
- **O sinal da FCF está certo.** Os coeficientes positivos do exemplo são do dado.
- **O EngMax funciona como desenhado** (ADR-017). As 32 execuções que abortam com `computed + UCT + EngMax` são uma inviabilidade **física** do caso (FURNAS a 0,59 % do volume útil), agravada pela falta de déficit.
- **Nenhuma flag move o custo na direção errada** (752 pares de configurações vizinhas comparados).

### 1.4 Como cada achado foi validado

| Fase | Quem | O que fez |
|---|---|---|
| Leitura | 5 agentes | um por domínio |
| Matriz | 1 agente | 384 configurações e 13 experimentos extras, com um verificador independente de 40 checks |
| Revisão | 5 revisores adversariais | cada um tentou **derrubar** os achados do seu domínio, com reprodução própria e comparação com os manuais do DESSEM |
| Teste | 4 testadores | escreveram testes Rust que **falham hoje** e **passariam com a correção**, cada um com um controle que passa |

**Resultado da fase de teste.** Os 4 testadores escreveram **173 testes** Rust:

| Resultado | Testes |
|---|---|
| testes de defeito que **falham hoje**, como esperado | 87 |
| controles, que **passam** | 75 |
| informativos e sondas | 9 |
| testes de defeito que passam, porque só checam um pré-requisito do NU-13 | 2 |

**Nenhum controle falhou.** Um teste do NU-R03 falha por erro de construção e foi descartado.

Todos os CRÍTICOS e ALTOS foram demonstrados por teste Rust ou por execução do binário (o FP-02 só por execução). A exceção é o NU-R01, que é uma questão de contrato: a execução confirma que a FCF do exemplo é inerte, e o resto depende da decisão D4. Detalhes na **seção 12**.

### 1.5 O que precisa de decisão sua

Treze decisões estão na **seção 13**. As que destravam mais coisas:

- **D1.** Como corrigir o sinal dos duais. Atenção: um simples `cargo update` muda esse comportamento.
- **D2.** Como corrigir o α duplicado.
- **D3.** Qual versão do DESSEM o UCH hidráulico segue. Os dois revisores divergem.
- **D4.** Contrato da FCF: volume absoluto ou útil, e unidade dos cortes.
- **D5.** Base oficial de demanda com rede.

---

## 2. Antes dos achados: como o labdessem monta e resolve o problema

Esta seção existe para que cada achado faça sentido sem precisar abrir o código. Quando um achado diz "o erro está na conversão" ou "o erro está na escrita", ele está se referindo a uma das etapas abaixo.

### 2.1 A cadeia completa, do CSV ao CSV

O labdessem lê um caso, monta um problema de otimização, resolve esse problema uma ou mais vezes e escreve os resultados. Cada etapa tem um dono bem definido:

| # | Etapa | O que faz | Onde fica |
|---|---|---|---|
| 1 | **Leitura** | Lê `study_config.json`, os `CAD_*.csv` (cadastro) e os `OPER_*.csv` (dados operativos). Converte horas em períodos de 0,5 h e associa linhas de arquivos diferentes por código. | [crates/labdessem-io/src/readers/](../../crates/labdessem-io/src/readers/), [study.rs](../../crates/labdessem-io/src/study.rs) |
| 2 | **Sistema** | Guarda tudo numa estrutura única, o `System`: usinas, unidades, barras, linhas, horizonte e flags. A partir daqui ninguém mais lê arquivo. | [crates/labdessem-core/](../../crates/labdessem-core/) |
| 3 | **Indexação** | Decide em que coluna da matriz fica cada variável (offsets de colunas) e em que linha fica cada restrição (offsets de linhas). | [indexing.rs](../../crates/labdessem-model/src/indexing.rs) |
| 4 | **Variáveis** | Cria as variáveis com **nome** (por exemplo `thermal_on[p=Termica-3,u=Termica-3-1,t=6]`) e limites. | [variables.rs](../../crates/labdessem-model/src/variables.rs) |
| 5 | **Objetivo** | Cria uma lista de termos `(nome da variável, coeficiente)`. | [objective.rs](../../crates/labdessem-model/src/objective.rs) |
| 6 | **Restrições** | Cria as restrições lineares, também com nome: balanço de demanda, balanço hídrico, produtividade, FPHA, UC, EngMax, FCF etc. | [constraints/](../../crates/labdessem-model/src/constraints/), [labdessem-uc](../../crates/labdessem-uc/src/) |
| 7 | **Conversão** | Transforma o modelo "com nomes" em vetores numéricos: um mapa `nome → coluna`, o vetor de custos e a matriz de restrições. | [conversion.rs](../../crates/labdessem-solver/src/conversion.rs) |
| 8 | **Solver** | Entrega a matriz ao HiGHS por meio da biblioteca `good_lp`, resolve e devolve primal (valores das variáveis) e dual (preços sombra das restrições). | [backends/highs.rs](../../crates/labdessem-solver/src/backends/highs.rs) |
| 9 | **Rede** (se `rede=1`) | Calcula a PTDF, verifica os fluxos e acrescenta "cortes de fluxo" (restrições de limite de linha) às linhas violadas. No fim, calcula CMB por barra e CMO por submercado a partir dos duais. | [crates/labdessem-network/](../../crates/labdessem-network/src/), [stages/helpers.rs](../../crates/labdessem-simulation/src/stages/helpers.rs) |
| 10 | **Leitura da solução** | Lê os valores por **posição** (offset + índice), usando a indexação da etapa 3. | [accessor.rs](../../crates/labdessem-solver/src/accessor.rs) |
| 11 | **Preparação da saída** | Monta um dicionário `nome → valor` com o que os escritores precisam. Se uma variável não entra nesse dicionário, o escritor não a vê. | [output_prep.rs](../../crates/labdessem-cli/src/output_prep.rs), [conversions.rs](../../crates/labdessem-cli/src/conversions.rs) |
| 12 | **Escrita** | Gera os `resultado_*.csv`. | [crates/labdessem-io/src/output/](../../crates/labdessem-io/src/output/) |

**Ponto de atenção.** Há duas maneiras de achar uma variável, e elas precisam concordar:

- **Por nome**: etapas 4, 5, 6, 7 e 11.
- **Por posição**: etapas 3 e 10.

Vários achados deste relatório são exatamente lugares onde as duas maneiras divergem. É o caso de HI-02 (StatusOn hidráulico) e dos offsets latentes NU-10.

### 2.2 Os modos de execução

O labdessem resolve o mesmo problema várias vezes, mudando a natureza das variáveis de comprometimento (liga/desliga) e o conjunto de restrições de rede. A sequência está em [orchestration.rs](../../crates/labdessem-simulation/src/stages/orchestration.rs).

**`opcao_execucao = 1` (iterativo)**

1. **LP**: problema linear sem as variáveis e restrições de UC térmico, igual ao "PL" da Tab. 27.2 do manual de metodologia do DESSEM. Esse LP roda em laço: resolve, verifica os fluxos, acrescenta cortes nas linhas violadas e resolve de novo até não haver violação ([orchestration.rs:95](../../crates/labdessem-simulation/src/stages/orchestration.rs#L95)).
2. **MILP**: problema inteiro com UC, já com os cortes acumulados ([orchestration.rs:106](../../crates/labdessem-simulation/src/stages/orchestration.rs#L106)).
3. **LP-FIXED**: as decisões de liga/desliga do MILP ficam fixas, e o problema linear roda em laço até a rede ficar viável ([orchestration.rs:169-203](../../crates/labdessem-simulation/src/stages/orchestration.rs#L169-L203)). O laço **não volta ao MILP**: o comprometimento decidido no passo 2 é definitivo.
4. **LP-CALC-CMO**: LP com comprometimento fixo e **todos** os cortes de fluxo, usado para gerar preços (CMO/CMB) e todas as saídas ([orchestration.rs:205-220](../../crates/labdessem-simulation/src/stages/orchestration.rs#L205-L220)).

**Atalho.** Se o MILP não viola nada, ou se precisou de folga, o passo 3 é pulado ([orchestration.rs:121](../../crates/labdessem-simulation/src/stages/orchestration.rs#L121)). Ver MX-10.

**`opcao_execucao = 2` (MILP único)**

1. **MILP** com todos os cortes de fluxo desde o início ([orchestration.rs:260-274](../../crates/labdessem-simulation/src/stages/orchestration.rs#L260-L274)).
2. **LP-CALC-CMO** com o comprometimento do MILP fixo.

**Com `rede = 0`** não há cortes. O balanço é só por submercado, com limites de intercâmbio.

> **Em toda configuração, o que você vê nos CSVs de resultado vem do LP-CALC-CMO.** Se o LP-CALC-CMO estiver certo e a escrita estiver errada, o problema é de saída. Se o LP-CALC-CMO estiver errado, o problema é de modelo.

### 2.3 As flags de `study_config.json`

| Flag | Valores | O que liga |
|---|---|---|
| `opcao_execucao` | 1, 2 | sequência da seção 2.2 |
| `rede` | 0, 1 | rede elétrica DC com PTDF e cortes de fluxo; CMB por barra |
| `UCT` | 0, 1 | unit commitment térmico: variáveis binárias, trajetórias de partida e parada, tempos mínimos |
| `UCH` | 0, 1 | unit commitment hidráulico: binárias por unidade geradora e canalização Pmin/Pmax |
| `TON_Residual` | 0, 1 | custo residual de partida e parada além do fim do horizonte (MM §10.5) |
| `FPHA` | `"constant_productivity"`, `"precomputed"`, `"computed"` | função de produção hidrelétrica: produtividade constante, hiperplanos lidos de `OPER_FPHA.csv` ou hiperplanos calculados pelo próprio labdessem |
| `FCF` | 0, 1 | função de custo futuro (cortes de Benders sobre o volume final) |
| `EngMax` | 0, 1 | limite dinâmico de engolimento máximo, recalculado entre solves conforme o volume (ADR-017) |

São 2 × 2 × 2 × 2 × 2 × 3 × 2 × 2 = **384 combinações**. A matriz da seção 11 rodou todas elas.

### 2.4 Dois conceitos que aparecem muito

**Dual (preço sombra).** Para cada restrição, o solver devolve um número que responde: "se eu relaxar o lado direito desta restrição em 1 unidade, quanto o custo total muda?".

- O dual do balanço de demanda é o **CMO**: quanto custa atender 1 MW a mais.
- O dual do balanço hídrico é o **valor da água**: quanto vale 1 hm³ a mais.

O sinal desse número depende da convenção da biblioteca. Esse detalhe é a causa do achado mais grave do relatório (RD-01).

**Restrição `≥` dentro do `good_lp`.** A biblioteca `good_lp` na versão 1.15.0, travada no `Cargo.lock`, não guarda `a ≥ b` como `a ≥ b`. Ela guarda `−a ≤ −b`. O solver responde com o dual da linha **negada**, e o labdessem lê esse número como se fosse o da linha original. As igualdades (`=`) e as linhas `≤` são enviadas sem essa inversão, por isso o dual delas sai com o sinal esperado. É por isso que o CMO sem rede (dual de uma igualdade) está certo e o CMB com rede congestionada (que soma duais de linhas `≥`) não está.

---

## 3. Como a auditoria foi feita

Você pediu três papéis separados: quem lê, quem revisa e quem testa. Foi assim que o trabalho se organizou:

| Fase | Quem | O que fez | Produto |
|---|---|---|---|
| **1. Leitura** | 5 agentes leitores, um por domínio: térmicas/UCT/TON, hidro/UCH, FPHA/EngMax, rede/duais/CMO, núcleo (indexação, conversão, solver, orquestração, FCF, configuração) | Seguiram o fluxo inteiro, do CSV de entrada até o CSV de saída, para cada dado do domínio. Anotaram toda suspeita com arquivo e linha. | 5 relatórios de leitura com IDs TH-, HI-, FP-/EM-, RD-, NU- |
| **2. Matriz** | 1 agente de teste caixa-preta | Rodou o binário de release nas **384 combinações** de flags e em 24 configurações inválidas ou de borda. Escreveu um verificador independente em Python (`check_outputs.py`, 40 checks) que confere as saídas contra identidades matemáticas do modelo. Fez 13 experimentos com o caso modificado para exercitar caminhos que o exemplo não toca. | relatório da matriz, achados MX-01 a MX-17 |
| **3. Revisão adversarial** | 5 agentes revisores, um por domínio, com o especialista DESSEM nos domínios de modelagem | Tentaram **derrubar** cada achado da fase 1: releram o código, refizeram os experimentos em cópias próprias e compararam com os manuais do DESSEM (MM 22.3.0, MU 22.3.2). Cada achado recebeu um veredito e uma severidade revisada. Também procuraram problemas que os leitores não viram (IDs com `R`, como TH-R01). | 5 relatórios de revisão |
| **4. Testes de prova** | 4 agentes testadores (térmicas; hidro e FPHA; rede; núcleo e FCF), cada um num clone isolado do repositório | Para cada achado confirmado de severidade relevante, escreveram um teste Rust que **falha hoje** e **passaria com a correção**, junto com um teste de controle que passa hoje (para mostrar que o teste não está quebrado por outro motivo). Também rodaram casos modificados com o binário de release. | 173 testes `audit_*` em 4 patches separados, e as saídas das execuções (seção 12) |

### 3.1 Vocabulário dos vereditos

| Veredito | Significado |
|---|---|
| **CONFIRMADO** | O revisor reproduziu o problema e concorda que é defeito. |
| **PARCIAL** | O fato observado é real, mas parte da alegação não se sustenta (por exemplo, é comportamento documentado ou igual ao DESSEM). |
| **REFUTADO** | O revisor mostrou que não é defeito. |
| **INCONCLUSIVO** | Não foi possível decidir com o que existe no repositório e nos manuais. |
| **LATENTE** | O código está errado, mas hoje nenhum caminho de produção consome a parte errada. Vira defeito quando alguém passar a usá-la. |

### 3.2 Escala de severidade

| Severidade | Critério |
|---|---|
| **CRÍTICA** | Resultado principal (despacho, CMO, CMB, custo total) errado **sem nenhum aviso**, em configuração suportada. |
| **ALTA** | Resultado ou dado errado num subconjunto relevante de configurações, ou execução que aborta sem necessidade. |
| **MÉDIA** | Saída secundária errada ou enganosa (por exemplo, coluna de status), ou fragilidade de uso (configuração mal digitada aceita em silêncio). |
| **BAIXA** | Cosmético, documentação, validação de entrada para casos raros, ou defeito latente. |

### 3.3 O que **não** foi feito

- **Nenhuma linha do repositório foi alterada.** `git status` do repositório principal continua limpo. Os testes de prova vivem em clones isolados em `~/.cache/labdessem-audit/` e em arquivos `.patch` que você aplica se quiser.
- **Nenhuma correção foi implementada.** Onde aparece "correção sugerida", é só uma indicação de onde mexer, para você avaliar.
- **O caso `precomputed` não foi exercitado de ponta a ponta**, porque o `OPER_FPHA.csv` do exemplo só tem cabeçalho (MX-06). A lógica foi lida e revisada, mas não executada com dados reais.
- **Períodos de duração variável não foram testados.** Todos os 48 períodos do exemplo têm 0,5 h. Achados que só aparecem com durações diferentes estão marcados como tal.
- **Renováveis e bombeamento não existem no exemplo.** Os termos correspondentes foram lidos no código, mas não exercitados.

### 3.4 Referências cruzadas de IDs

O mesmo defeito às vezes foi encontrado por mais de um agente, com IDs diferentes. Neste relatório cada defeito aparece **uma vez**, com o ID principal e os sinônimos ao lado (por exemplo, "RD-01 = NU-02 = MX-01"). O Apêndice A traz a tabela completa de equivalências.

---

## 4. Todos os achados numa tabela, do mais grave ao menos grave

**Como ler as colunas:**

- **Seção** aponta para a explicação detalhada.
- **Configs** diz em que configuração o problema aparece.
- **Tipo** diz onde está o erro na cadeia da seção 2.1:

| Tipo | Significado |
|---|---|
| entrada | leitura ou validação de dados |
| modelo | formulação do problema |
| preço | duais, CMB ou CMO |
| saída | escrita dos CSVs |
| doc | documentação |

### 4.1 CRÍTICA

| ID (sinônimos) | Seção | Problema | Configs | Tipo | Veredito |
|---|---|---|---|---|---|
| **RD-01** (NU-02, MX-01) | §8 | CMB e CMO com a componente de congestionamento **invertida** quando uma linha está presa no limite inferior. O CMO passa a depender da orientação De/Para das linhas. | `rede=1` com congestionamento | preço | CONFIRMADO (3 provas). O revisor do núcleo deu ALTA; o revisor de rede e a matriz, CRÍTICA. |
| **HI-01** (NU-03, MX-03) | §6 | Afluências casadas só por código. No exemplo, 88 % da água some e o resto vai para a usina errada, **sem aviso**. | todas (dado do exemplo + leitor) | entrada | CONFIRMADO. Os revisores hidro e núcleo deram CRÍTICA; a matriz, ALTA. |

### 4.2 ALTA

| ID (sinônimos) | Seção | Problema | Configs | Tipo | Veredito |
|---|---|---|---|---|---|
| **TH-01** (NU-01) | §5 | Custo de partida apagado pelo custo residual (`=` em vez de `+=` na FO). | `UCT=1` + `TON_Residual=1` | modelo | CONFIRMADO |
| **TH-03** | §5 | Unidade em rampa de desligamento inicial repete a trajetória D (gera à força). Sub-caso: liga sem partida. | `UCT=1` + `RdownInic=1` | modelo | CONFIRMADO |
| **HI-03** (FP-01, NU-04) | §6 | FPHA computed: usina sem planos gera sem água. Sem arquivo de geometria, **todas** geram, e a execução termina com exit 0. | `FPHA=computed` | modelo e entrada | CONFIRMADO |
| **FP-02** | §7 | FPHA computed aplica α duas vezes (envelope α² em vez de α). | `FPHA=computed` | modelo | CONFIRMADO |
| **RD-02** (MX-02, NU-05) | §8 | CMB não dividido por Δt: no exemplo sai pela metade. | `rede=1` (todas) | preço e saída | CONFIRMADO. O revisor do núcleo deu MÉDIA. |
| **RD-04** (MX-04) | §8 | Balanço usa `OPER_SBM`, rede usa `OPER_CARGA_BARRA`. Os ~270 MW de diferença vão para a barra swing. | `rede=1` | entrada e modelo | CONFIRMADO |
| **NU-R02** | §9 | Cortes da FCF aplicados sobre (V − Vmin); o formato e a documentação usam V absoluto. | `FCF=1` | modelo | NOVO, confirmado numericamente |
| **NU-R01** | §9 | Unidade dos cortes indefinida; o exemplo está provavelmente 1000× pequeno, o que deixa a FCF inerte. | `FCF=1` | entrada e doc | NOVO; a severidade depende do contrato |

### 4.3 MÉDIA

| ID (sinônimos) | Seção | Problema | Configs | Tipo | Veredito |
|---|---|---|---|---|---|
| **TH-02** (MX-08, NU-06) | §5 | StatusOn térmico vem da geração, não de u[t]. | `UCT=1` | saída | CONFIRMADO |
| **HI-02** (MX-07, NU-06) | §6 | StatusOn hidráulico lido da coluna errada (0, ou valor da folga). | `UCH=0` | saída | CONFIRMADO |
| **HI-04** (FP-04, NU-15, MX-09) | §6 | Produtividade constante só por usina; unidade gera sem água; capacidade superestimada com unidades heterogêneas. | `FPHA=cp` (default) | modelo e saída | CONFIRMADO / PARCIAL |
| **HI-05** (FP-03) | §6 | Com UCH, unidade desligada turbina, e com FPHA essa água infla a geração das ligadas. | `UCH=1` | modelo | **os revisores divergem** (D3) |
| **HI-07** | §6 | Desvio sem limite e sem custo (373.425 m³/s num período). | `Desvio` preenchido | modelo | CONFIRMADO |
| **FP-05** | §7 | `CAD_FPHA_CONFIG.csv` é lido e ignorado. | `FPHA=computed` | entrada | CONFIRMADO |
| **RD-05** (MX-05) | §8 | Sem variável de déficit: casos apertados abortam sem saída. | todas | modelo | PARCIAL. A matriz deu ALTA. |
| **RD-07** | §8 | Par de submercados não declarado vira intercâmbio ilimitado e gratuito. | `rede=0` | modelo e entrada | PARCIAL |
| **RD-03** | §8 | cmobar com período começando em 0 e sem agregação. | `rede=1` | saída | CONFIRMADO |
| **RD-06** | §8 | Barra swing fora do cmobar e da média do CMO. | `rede=1` | saída e preço | CONFIRMADO |
| **RD-08** | §8 | "periodo original" com período interno. | durações ≠ 0,5 h | saída | CONFIRMADO |
| **RD-15** (TH-07, HI-06) | §8 | Barra da usina só da 1ª unidade, em silêncio. | `rede=1` | entrada | CONFIRMADO |
| **NU-07** (TH-R01, HI-08) | §10 | Nomes duplicados fundem colunas (objetivo até 30× maior). | CAD com nome repetido | entrada e conversão | CONFIRMADO |
| **NU-11** (MX-11) | §10 | `study_config.json`: erro de digitação, chave faltante ou valor 2 aceitos em silêncio. | qualquer | entrada | CONFIRMADO |
| **MX-06** | §10 | `precomputed` inutilizável com o exemplo (um terço da matriz aborta). | `FPHA=precomputed` | dado e cobertura | CONFIRMADO |
| **MX-10** (NU-R06) | §10 | Modo 1 com rede apertada: o MILP decide sem ver a rede e nunca é refeito (2,9× mais folga que o modo 2). | `opcao=1`, `rede=1`, `UCT=1` | algoritmo | CONFIRMADO; causa corrigida pelo revisor |

### 4.4 BAIXA

| Domínio | IDs | Resumo |
|---|---|---|
| Térmicas | TH-04 (só doc), TH-05 (escolha deliberada), TH-06, TH-08, TH-09, TH-10, TH-11, TH-12, TH-R02, TH-R03 | validação de rampas, arredondamento de Ton/Tinic, colunas residuais hipotéticas, contagens em meia hora, código 99 |
| Hidráulica | HI-09, HI-R01, HI-R02, HI-R03, HI-R04 | offsets latentes, usina só reservatório com rede, ciclos de cascata, código desconhecido em conjuntos, documentação |
| FPHA/EngMax | FP-06, FP-07, FP-08, FP-09, FP-10, FP-11, FP-12, EM-04, FP-R01, FP-R02, FP-R03 | volume da FPHA, NaN na geometria, η = 1 implícito, teste que trava o FP-02, documentação |
| Rede | RD-09, RD-10 (MX-13), RD-11, RD-12, RD-13, RD-14 (NU-08), RD-16, RD-17, RD-R01, RD-R02, RD-R03 | Violacao sem tolerância, intercâmbio não gravado, backend latente, versão do good_lp, termos descartados em silêncio |
| FCF | NU-R03, NU-R04, NU-12, MX-14 (parte doc) | associação por posição, α = 0 em silêncio, α não exportado, documentação |
| Núcleo | NU-09, NU-10 (latente; agrupa TH-10, HI-09, FP-10, RD-13 e MX-15), NU-13, NU-14, NU-R05, MX-12, MX-16, MX-17 | status do solver, offsets, relaxação em `Unbounded`, mensagens |

### 4.5 Refutados ou inconclusivos

| ID | Veredito | Por quê |
|---|---|---|
| **EM-01** | REFUTADO como bug | O aborto com computed + UCT + EngMax é o ADR-017. A causa é física: FURNAS a 0,59 % do volume útil dá EngMax 1388 < 1506 m³/s, e não há déficit. |
| **EM-03** | REFUTADO | EngMax 0 para usina sem unidades é documentado e fisicamente correto. |
| **EM-02** | INCONCLUSIVO | O mecanismo existe, mas não foi possível provocar um problema. Todas as saídas vêm do mesmo LP. |
| **TH-04** | PARCIAL (só doc) | LP sem UC no modo 1 é o "PL" do DESSEM (Tab. 27.2). |
| **MX-14** (sinal) | REFUTADO na parte de sinal | O código segue a convenção do DESSEM e do cobre; os π positivos são do dado. |

---

## 5. Térmicas, UCT e TON residual

### 5.0 O mínimo de modelagem para ler esta seção

Com `UCT=1`, cada unidade térmica tem quatro famílias de variáveis por período *t*:

| Símbolo | Nome no código | Significado |
|---|---|---|
| g[t] | `thermal_gen` | geração em MW |
| u[t] | `thermal_on` | 1 se a unidade está **comprometida** (ligada ou em trajetória), 0 se desligada |
| y[t] | `thermal_startup` | 1 se a unidade **parte** em t |
| w[t] | `thermal_shutdown` | 1 se a unidade **desliga** em t |

As unidades grandes não saltam de 0 a Pmin. Elas seguem uma **trajetória de acionamento** A (por exemplo 90 → 180 → 360 MW) e, ao desligar, uma **trajetória de desligamento** D (por exemplo 80 → 0 MW). Essas trajetórias vêm de `CAD_RAMPAS_TERMICAS.csv`. Cada passo vale um período interno de 0,5 h. Durante a trajetória a unidade está comprometida (u=1), mesmo que um passo valha 0 MW.

A **condição inicial** diz como a unidade estava antes do horizonte:

| Campo | Significado |
|---|---|
| `StatusInic` | ligada ou desligada |
| `Tinic` | há quanto tempo ela está nesse estado |
| `GerInic` | geração inicial |
| `RdownInic` | se ela já está no meio de um desligamento |
| `TRampaInic` | quanto da trajetória D já foi percorrido |

O **custo residual de TON** (`TON_Residual=1`, MM §10.5) cobra o que a decisão tomada dentro do horizonte vai custar **depois** dele. Por exemplo, uma unidade que parte em t=46 com Ton de 3 h precisa continuar ligada além do fim do dia, e essa geração forçada tem custo.

---

### TH-01 — Com `TON_Residual=1`, o custo de partida desaparece da função objetivo

> **Severidade: ALTA** · **Veredito: CONFIRMADO** (revisor térmico e revisor do núcleo, que o registrou como NU-01) · **Configs: `UCT=1` e `TON_Residual=1`, os dois modos, com ou sem rede**

**Em uma frase.** Quando o custo residual está ligado, o coeficiente da variável de partida y passa a ser **só** o residual, e o custo de partida da unidade (`CustoPartida` de `CAD_UNID_UTE`) some do problema.

**Como deveria ser.** No DESSEM os dois custos são parcelas **separadas** da função objetivo (MM §17.2 para a partida e MM §10.5 para o residual). Para a mesma variável, o coeficiente final tem de ser a **soma**:

```
custo de y[t] = CustoPartida + CustoResidualPartida(t)
```

**O que acontece hoje.**

1. A função objetivo é montada como uma lista de termos `(variável, coeficiente)`. Para cada unidade e período, [objective.rs:96](../../crates/labdessem-model/src/objective.rs#L96) acrescenta `(y[t], CustoPartida)`.
2. Com `TON_Residual=1`, [objective.rs:110](../../crates/labdessem-model/src/objective.rs#L110) acrescenta **outro** termo para a **mesma** variável: `(y[t], CustoResidualPartida(t))`. Esse termo é acrescentado inclusive quando vale 0.
3. Na conversão para o solver, [conversion.rs:50-54](../../crates/labdessem-solver/src/conversion.rs#L50-L54) percorre a lista e faz `objective[col] = term.coefficient`. É uma **atribuição**, não uma soma, então o último termo apaga o anterior.

```rust
for term in &model.objective.terms {
    if let Some(&col) = var_name_to_col.get(term.variable.as_str()) {
        objective[col] = term.coefficient;   // ← deveria ser +=
    }
}
```

**Resultado.** O coeficiente de y fica igual ao residual. O residual só é positivo perto do fim do horizonte, quando o Ton não cabe mais. Para uma partida no meio do dia, ele é 0, e **a partida sai de graça**.

**Evidência.** O revisor colocou `CustoPartida = 1.000.000` na Termica-3, que parte em p1 no caso base:

| Caso | `TON_Residual` | Despacho da T3 (p1–p5) | Objetivo |
|---|---|---|---|
| r1a | 0 | 90 / 180 / 360 / 560 / 80 | 5.497.639,84 |
| r1b | 1 | 90 / 180 / 360 / 560 / 80 (mesma partida) | **4.497.639,84** |

A unidade parte nos dois casos, mas com `TON_Residual=1` o custo de 1 milhão some do objetivo.

**Consequências.**

- **O custo total reportado fica subestimado** sempre que houver partida com custo.
- **No MILP, a decisão de comprometimento muda.** O otimizador vê partidas gratuitas, então pode ligar e desligar unidades caras com mais frequência do que deveria.
- **O CMO não é afetado diretamente.** No LP-CALC-CMO y está fixo, então a variável não influencia os duais, embora o despacho fixado possa ser outro.

**Alcance maior do que parece.** A atribuição em `conversion.rs:52` é uma armadilha geral: **qualquer** variável que receba dois termos no objetivo perde o primeiro. Hoje o único caso real é este. O TH-R01, mais abaixo, mostra o mesmo mecanismo com unidades duplicadas.

**Correção sugerida (não implementada).** Trocar `=` por `+=` em `conversion.rs:52`. Isso resolve este caso e qualquer duplicidade futura.

**Teste de prova: CONFIRMADO.** O teste `audit_th01_startup_cost_survives_ton_residual` falha hoje: com `TON_Residual=1`, o coeficiente de `thermal_startup[Termica-3,t=1]` sai **−0 em vez de 1.000.000**, e o mesmo vale para as 48 colunas de partida.

Dois controles passam. O sem TON passa. O que repete termos numa **restrição** também passa, o que mostra que o bug está só na FO.

Na execução real, com custo de partida de 1 milhão na Termica-3, o objetivo é 5.497.639,84 com TON=0 e **4.497.639,84** com TON=1, com a mesma partida no período 1. Detalhes na seção 12.

---

### TH-03 — Unidade que começa no meio de um desligamento repete a trajetória de desligamento

> **Severidade: ALTA** · **Veredito: CONFIRMADO, com um sub-caso extra encontrado pelo revisor** · **Configs: `UCT=1` com alguma unidade com `RdownInic=1` em `CAD_UNID_UTE`**

**Em uma frase.** Se uma unidade começa o horizonte já descendo a rampa de desligamento, o modelo não deixa que ela termine o desligamento no ponto certo. Ela é obrigada a **fazer a trajetória D inteira de novo**, gerando energia que ninguém pediu e pagando por isso.

**Como deveria ser.** Considere D = [255, 0] (dois passos de 0,5 h) e uma unidade que já percorreu 1 passo (`TRampaInic = 0,5 h`).

- Falta 1 passo, de 0 MW, em p1.
- A partir de p2 a unidade deveria estar **desligada** (u=0).

É assim no DESSEM: o campo 8 do registro INIT do `operut` ("em rampa de desligamento") significa exatamente isso.

**O que acontece hoje.** As restrições de trajetória estão em [trajectory.rs](../../crates/labdessem-uc/src/thermal/constraints/trajectory.rs). O comportamento atual é:

1. Para os períodos que faltam da rampa inicial (`t < r`, com `r = len(D) − passos já feitos`), as linhas 43-46, 66-69 e 100-103 forçam u[t]=1 e g[t] igual ao restante da rampa. Essa parte está certa.
2. **Nada** força u[r]=0, ou seja, nada obriga a unidade a efetivamente desligar no fim da rampa.
3. A unidade também não consegue desligar em r por conta própria. Desligar em r significa w[r]=1, e as linhas 153-161 aplicam a trajetória D **para trás** a partir de w[r]: "se desligou em r, então em r−1 estava gerando D[último], em r−2 estava gerando D[penúltimo]...". Esses períodos anteriores já estão presos pela condição inicial. A combinação dá um limite superior impossível (0 ≤ −672 no exemplo), então w[r]=1 é inviável.
4. O primeiro desligamento viável é em s ≥ r + len(D), e **a trajetória D inteira se repete**.

**Evidência.** Termica-1 com `GerInic=255`, `StatusInic=1`, `RdownInic=1`, `TRampaInic=0,5`, D=[255, 0]:

| Caso | CVU da T1 | Despacho da T1 (p1 / p2 / p3) | Esperado | Objetivo |
|---|---|---|---|---|
| r3a | 407,34 | 0 / **255** / 0 | 0 / 0 / 0 | 4.549.562,57 |
| r3b | **100.000** | 0 / **255** / 0 | 0 / 0 / 0 | 17.247.626,72 |

No caso r3b a T1 custa 100 mil R$/MWh e **mesmo assim** gera 255 MW em p2. A geração é forçada pelas restrições, não escolhida pelo otimizador.

**Sub-caso, ainda pior.** Se `TRampaInic` for igual ao comprimento total de D, o restante fica vazio (r=0). O validador de [entities/thermal.rs:172](../../crates/labdessem-core/src/entities/thermal.rs#L172) aceita esse valor, porque só rejeita `>`. Nenhuma restrição de condição inicial se aplica, e a unidade, que deveria estar terminando o desligamento, pode ir direto para [Pmin, Pmax] **sem partida e sem custo de partida**. No caso r3c, a T1 foi de 0 a 523 MW em p1.

**Consequências.**

- Energia térmica espúria nos primeiros períodos.
- Custo errado.
- No sub-caso, uma "partida" gratuita que viola a trajetória A e o tempo mínimo desligado.

**Correção sugerida (não implementada).** Quando a unidade está em rampa de desligamento, fixar w[r]=1, inclusive com r=0, e **não** aplicar as restrições `initial_shutdown_*`. Os próprios termos de w[r] já reproduzem o restante da trajetória.

**Teste de prova: CONFIRMADO, inclusive o subcaso.** No crate `labdessem-uc`, a trajetória fisicamente correta (terminar a rampa e ficar em 0) é **rejeitada** pela restrição `thermal_ramp_upper[t=1]`, que vira "0 ≤ −672".

Na execução com CVU de 100.000 R$/MWh, a unidade gera **255 MW** no período 2 e o objetivo sobe 12,75 milhões. O controle sem rampa inicial fica em 0 MW. No subcaso com a rampa já terminada, a unidade salta para 523 MW sem partida. Seção 12.

---

### TH-02 — `StatusOn` e `TempoPermanencia` das térmicas são calculados pela geração, não pelo comprometimento

> **Severidade: MÉDIA** · **Veredito: CONFIRMADO** (também MX-08 na matriz e parte de NU-06) · **Configs: todas com `UCT=1` (96 de 96 execuções)**

**Em uma frase.** O CSV `resultado_termicas.csv` diz que a unidade está desligada sempre que ela gera 0 MW, mesmo quando o otimizador a manteve comprometida, como no último passo da rampa de desligamento.

**Como deveria ser.** `StatusOn` deveria mostrar u[t], que é o que o cabeçalho do CSV promete ("fração do período em que a unidade ficou ligada").

**O que acontece hoje.**

1. [output_prep.rs:38-48](../../crates/labdessem-cli/src/output_prep.rs#L38-L48) coloca no dicionário de saída **só** a geração térmica. O valor de u[t] nunca é copiado. O acessor `thermal_commitment` existe ([accessor.rs:179](../../crates/labdessem-solver/src/accessor.rs#L179)), mas ninguém o chama.
2. O escritor [output/thermal.rs:120-134](../../crates/labdessem-io/src/output/thermal.rs#L120-L134) procura `thermal_on[...]` no dicionário, não encontra e cai no plano B: `StatusOn = 1 se |g| > 1e-8`.
3. `TempoPermanenciaOn/Off` é recalculado a partir desse status errado.

**Evidência (caso base, sem modificação).** Linha `Termica-3;1;6` de `resultado_termicas.csv`:

| Coluna | Impresso | Correto |
|---|---|---|
| GeracaoMW | 0 | 0 |
| StatusOn | **0** | 1 |
| TempoPermanenciaOn | **0** | 6 |

A T3 parte em p1 com Ton = 3 h = 6 períodos. A restrição de tempo mínimo obriga u[1..6]=1. O período 6 é o último passo da rampa D (0 MW), então ela está comprometida.

**Consequência.** O despacho e o custo estão corretos. Errado está só o relato. Quem usar `StatusOn` para contar partidas ou horas ligadas vai errar.

**Teste de prova: CONFIRMADO por execução** (caso base e check T05 da matriz, 96 de 96). O teste Rust do `output_prep` não chegou a ser escrito, porque o testador foi interrompido.

O teste do lado do escritor **passa**: se `thermal_on` fosse entregue, o `StatusOn` sairia certo. Isso confirma que a correção é só no `output_prep.rs`. Seção 12.

---

### 5.x Achados de severidade BAIXA em térmicas

| ID | O que é | Por que é BAIXA | Onde |
|---|---|---|---|
| **TH-04** | O LP do modo 1 não tem variáveis de UC térmico. **O comportamento está correto** e é igual ao "PL" da Tab. 27.2 do DESSEM. Só o documento `iterative-workflow.md:64` está errado: ele diz que o LP relaxa os binários em [0,1]. | Só documentação. O código, o manual de desenvolvimento e um teste já fixam o comportamento certo. | [iterative-workflow.md:64](../../docs/design/iterative-workflow.md#L64) |
| **TH-05** | O residual de **desligamento** é cobrado sobre u[T] de **toda** unidade ligada no fim do horizonte. A eq. 10.22 do DESSEM só cobra de unidade acionada no horizonte com Ton não cumprido. | Divergência **deliberada** (commit `cd9e6c1`) e economicamente coerente, mas não documentada. **Decisão sua:** seguir a eq. 10.22 ou documentar a escolha. | [objective.rs:112-121](../../crates/labdessem-model/src/objective.rs#L112-L121) |
| **TH-06** | Uma unidade ligada fora de rampa que desliga logo em p1 cai de 600 MW a 0 em meia hora, sem fazer a trajetória D. | É a discretização literal da eq. 10.10 do MM. O CSV não traz a trajetória anterior ao horizonte. | [trajectory.rs:153-161](../../crates/labdessem-uc/src/thermal/constraints/trajectory.rs#L153-L161) |
| **TH-07** | `Barra` e `Submercado` de uma usina vêm só da 1ª unidade. Se outra unidade disser outra barra, o valor é descartado sem aviso. | Validação de entrada. O modelo de dados exige valor único. | [readers/thermal.rs:102](../../crates/labdessem-io/src/readers/thermal.rs#L102) |
| **TH-08** | Em `CAD_RAMPAS_TERMICAS`, o `CodigoUTE` é ignorado (a associação é pelo nome) e um `Ipasso` duplicado alonga a trajetória ([255, 255, 0]). No exemplo, as rampas da Termica-3 trazem o código 1, que é o da Termica-2. | Validação de entrada. Hoje funciona porque o nome está certo. | [readers/thermal.rs:51](../../crates/labdessem-io/src/readers/thermal.rs#L51), [:79-80](../../crates/labdessem-io/src/readers/thermal.rs#L79-L80) |
| **TH-09** | O Ton restante é calculado como `ceil(Ton) − ceil(Tinic)`. Arredondar para cima o tempo **já cumprido** é otimista. Com Tinic = 81,75 h e Ton = 82 h, falta 0,25 h, mas o modelo deixa desligar em p1. O certo é `ceil((Ton − Tinic)/dt)`. | Exige `Tinic` que não seja múltiplo de 0,5 h. | [min_up_down.rs:27-37](../../crates/labdessem-uc/src/thermal/constraints/min_up_down.rs#L27-L37) |
| **TH-10** | Offsets de linhas e colunas do UC térmico errados na indexação: colunas reservadas no LP, onde não existem, e o bloco `thermal_min_up` deslocado pelas linhas de condição inicial. | **Latente.** Nenhum código de produção lê esses offsets. Faz parte do NU-10. | [indexing.rs:218-231](../../crates/labdessem-model/src/indexing.rs#L218-L231), [:328-334](../../crates/labdessem-model/src/indexing.rs#L328-L334) |
| **TH-11** | As colunas de custo residual de `resultado_termicas.csv` são **hipotéticas** ("caso a unidade acione no início do período"), e o custo residual **efetivamente pago** não aparece em lugar nenhum. Com `UCT=0`, as colunas saem preenchidas, embora o termo não exista na FO. | O cabeçalho já avisa que é hipotético. Falta o valor efetivo. | [output/thermal.rs:49-60](../../crates/labdessem-io/src/output/thermal.rs#L49-L60) |
| **TH-12** | `TempoPermanencia` é contado em períodos internos de 0,5 h, enquanto a coluna `Periodo` é original. O agregado (unidade 99) mistura a condição inicial de unidades ligadas e desligadas (`any`/`max`). | Só aparece com períodos de 1 h ou com usinas de várias unidades em estados diferentes. | [output/thermal.rs:136-140](../../crates/labdessem-io/src/output/thermal.rs#L136-L140), [:203-211](../../crates/labdessem-io/src/output/thermal.rs#L203-L211) |
| **TH-R01** | **Unidade duplicada** em `CAD_UNID_UTE` não dá erro. As duas linhas geram variáveis com o **mesmo nome**, e a conversão as funde numa coluna só: o balanço conta a injeção duas vezes e a FO uma vez. Resultado: objetivo de **137,8 milhões** contra 4,5 milhões, com uma violação de rede espúria de 133,76 MW na LINHA-10. | É erro de digitação no CAD, mas o efeito é enorme e silencioso. | [conversion.rs:22-26](../../crates/labdessem-solver/src/conversion.rs#L22-L26) |
| **TH-R02** | O código 99 do agregado colide com uma unidade real numerada 99. | Raro. | [output/thermal.rs:280](../../crates/labdessem-io/src/output/thermal.rs#L280) |
| **TH-R03** | Cada passo de trajetória vale 0,5 h, implicitamente. O CSV não tem coluna de tempo (o `rampas.dat` do DESSEM tem). Uma trajetória horária importada ficaria com metade da duração, sem aviso. | Documentação. | [trajectory.rs:131-137](../../crates/labdessem-uc/src/thermal/constraints/trajectory.rs#L131-L137) |

**Verificado e correto em térmicas** (matriz, check T02, 96 execuções):

- trajetórias de partida e parada nível a nível;
- Pmin ≤ g ≤ Pmax fora das trajetórias;
- tempos mínimos ligado e desligado;
- condição inicial (exceto os casos de TH-03 e TH-09);
- custos de partida e parada com `TON_Residual=0`;
- CVU × g × duração na FO.

O custo residual com `TON_Residual=1` fecha exatamente (+12.000,00 R$ = 80 MW × 300 R$/MWh × 0,5 h no experimento `tonres_cmo0`). O que está errado é só a sobreposição do TH-01.

---

## 6. Hidráulica e UCH

### 6.0 O mínimo de modelagem para ler esta seção

Cada usina hidrelétrica tem um reservatório. Cada unidade geradora tem uma turbina. Por período *t* existem:

| Símbolo | Nome no código | Nível | Significado |
|---|---|---|---|
| V[t] | `hydro_volume` | usina | volume no fim do período (hm³) |
| q_u[t] | `hydro_turbining` | unidade | vazão turbinada (m³/s) |
| g_u[t] | `hydro_generation` | unidade | geração (MW) |
| S[t] | `hydro_spillage` | usina | vertimento (m³/s) |
| Desv[t] | `hydro_diversion` | usina | desvio para outra usina (m³/s) |
| on_u[t] | `hydro_on` | unidade | comprometimento, só com `UCH=1` |

**Balanço hídrico** (uma igualdade por usina e período). O fator 0,0036 converte m³/s × h em hm³:

```
V[t] = V[t−1] + (Afl − Ret + Qmontante − Σq − S − Desv − Bomb) × dur × 0,0036
```

**Função de produção.** Diz quanto se gera com a água turbinada. Depende de `FPHA`:

| `FPHA` | Função de produção |
|---|---|
| `constant_productivity` | Σ_u g_u = ρ · Σ_u q_u, uma igualdade **por usina** |
| `precomputed` / `computed` | Σ_u g_u ≤ α · (γ0 + γV·V + γQ·Σq + γS·S), vários hiperplanos por usina. Os γ vêm de `OPER_FPHA.csv` ou são calculados pelo próprio labdessem (seção 7). |

**Afluência natural (Afl).** Vem de `OPER_VAZAO.csv`. Cada linha traz `Codigo`, `Nome`, `Periodo`, `Vazao` e `Retirada`.

---

### HI-01 — As afluências do exemplo vão para as usinas erradas, e o leitor não avisa

> **Severidade: CRÍTICA, com ressalva** (revisor hidro; a matriz classificou como ALTA, MX-03) · **Veredito: CONFIRMADO** (também NU-03 e MX-03) · **Configs: todas as 384, porque o defeito está no leitor e no caso de exemplo**

**Em uma frase.** O `OPER_VAZAO.csv` do exemplo usa os códigos 1, 2, 4 e 6 e o `CAD_UHE.csv` usa 0, 1, 2 e 3. O leitor casa as linhas **só pelo código**, então ITUTINGA recebe a afluência da CAMARGOS, FUNIL-GRANDE recebe a da ITUTINGA, e as afluências de FUNIL-GRANDE e FURNAS são **jogadas fora sem aviso**. Resultado: **88 % da água que entra no sistema desaparece.**

**Como os dados ficaram assim.** O histórico do git mostra um esquecimento:

- No commit `cd9e6c1`, `CAD_UHE` e `OPER_VAZAO` usavam os mesmos códigos 1/2/4/6, que são os códigos ONS das usinas.
- O commit `a34f09f` renumerou `CAD_UHE` e `CAD_CONJ_UHE` para 0..3, inclusive as colunas `Jusante`. No mesmo commit, estendeu `OPER_VAZAO` para 48 períodos **mantendo 1/2/4/6**.
- A coluna `Nome` do `OPER_VAZAO` continua batendo com o cadastro linha a linha. Só o código ficou velho.

**O que o leitor faz.**

1. [readers/hydro.rs:176-190](../../crates/labdessem-io/src/readers/hydro.rs#L176-L190) agrupa as afluências por `row.codigo`, sem conferir se o código existe no cadastro.
2. [readers/hydro.rs:290-297](../../crates/labdessem-io/src/readers/hydro.rs#L290-L297): uma usina do cadastro sem nenhuma linha recebe `vec![0.0; períodos]`, ou seja, afluência zero, em silêncio.
3. Linhas com código desconhecido (4 e 6) nunca são lidas por ninguém. Não há erro nem aviso.
4. O campo `Nome` é lido como `_nome` ([readers/hydro.rs:501](../../crates/labdessem-io/src/readers/hydro.rs#L501)) e descartado.
5. Período duplicado sobrescreve; período faltante vira 0.

Para comparação, `Jusante` e `Desvio` do `CAD_UHE` passam por `optional_hydro_reference_by_code` ([readers/hydro.rs:362-379](../../crates/labdessem-io/src/readers/hydro.rs#L362-L379)), que **rejeita** código desconhecido. A afluência é o único dado hidráulico sem essa proteção.

**O que o modelo recebe:**

| Linha do `OPER_VAZAO` | Afluência / retirada (m³/s) | Para onde vai |
|---|---|---|
| código 1, "CAMARGOS" | 92 / 0,6 | **ITUTINGA** (código 1 no cadastro) |
| código 2, "ITUTINGA" | 0 / 0 | **FUNIL-GRANDE** (código 2 no cadastro) |
| código 4, "FUNIL-GRANDE" | 175 / 3,8 | **descartada** |
| código 6, "FURNAS" | 523 / 11 | **descartada** |

**Efeito medido (período 1, linha de usina `99;99`):**

| Usina | Hoje | Com os códigos corrigidos |
|---|---|---|
| CAMARGOS | afluência 0, geração 0 | afluência 92, geração 8,8 |
| ITUTINGA | afluência 92 (da CAMARGOS), geração 52, vertimento 39,4 | afluência 0, geração 8,8 |
| FUNIL-GRANDE | afluência 0, geração 91,4 | afluência 175, geração 180 |
| FURNAS | afluência 0, geração 1159,86; V48 = 5760,71 | afluência 523, geração 1105,66; V48 = 5820,96 |
| **Objetivo** | 4.497.639,8364 | 4.497.640,3859 |

**Por que o objetivo quase não muda.** No exemplo, o sistema está limitado por capacidade e intercâmbio, não por água, e FURNAS tem reservatório grande. A geração hidráulica **total** por período é a mesma, e CMO, CMB e despacho térmico ficam **idênticos**. Mudam:

- o despacho **por usina**;
- os volumes;
- o valor da água (`PiBalHidr`);
- os fluxos de rede (1584 de 1633 linhas de `resultado_rede.csv` diferentes).

**Ressalva da severidade.** O revisor manteve CRÍTICA porque o caso de referência, que está versionado com os resultados em `examples/24Barras/RESULTADOS/`, produz despacho hidráulico errado na configuração padrão sem nenhum aviso. Se você separar os dois defeitos:

- o **dado** errado do exemplo é corrigido em 1 minuto;
- o **leitor** que descarta e zera em silêncio é ALTA. Esse é o perigo real para decks futuros: qualquer erro de código em `OPER_VAZAO` some.

**Correção sugerida (não implementada).**

- No leitor: rejeitar código desconhecido, como já se faz para `Jusante`, e opcionalmente conferir o `Nome`.
- Exigir uma linha por usina e período, ou ao menos avisar quando faltar.
- Corrigir os códigos do exemplo e regenerar `examples/24Barras/RESULTADOS/`.

**Decisão sua.** Quer validar também o `Nome`? Isso pegaria erros de código, mas exige que os nomes sejam idênticos entre os arquivos.

**Teste de prova: CONFIRMADO.** O teste `audit_hi01_shipped_example_inflow_must_match_oper_vazao_by_plant_name` falha no exemplo versionado:

- CAMARGOS recebe 0 (o arquivo diz 92);
- ITUTINGA recebe 92 (o arquivo diz 0);
- FUNIL e FURNAS recebem 0.

Três outros testes mostram que também são aceitos **em silêncio**, com afluência 0: código inexistente, usina sem nenhuma linha e período faltando. O controle mostra que o leitor do `Jusante` **já** rejeita código desconhecido, então o padrão de validação existe no próprio projeto. Na execução com os códigos corrigidos, a geração média de FUNIL passa de 91 para 180 MW. Seção 12.

---

### HI-03 — FPHA "computed": usina sem geometria gera energia sem gastar água

> **Severidade: ALTA** · **Veredito: CONFIRMADO, pior que o relatado** (também FP-01 e NU-04) · **Configs: `FPHA="computed"` sempre que alguma usina com unidades é "pulada" no cálculo dos hiperplanos**

**Em uma frase.** No modo `computed`, se o labdessem não consegue calcular os hiperplanos de uma usina (falta geometria, erro de ajuste, tabela com volume duplicado), ele só imprime um `Warning` e segue. A usina fica **sem nenhuma restrição ligando geração e água** e gera até o Pmax com turbinamento zero. Se o arquivo de geometria não existir, **todas** as usinas ficam assim, e a execução termina com sucesso.

**Por que isso acontece.** É uma questão de ordem das operações:

1. [study.rs:146](../../crates/labdessem-io/src/study.rs#L146) monta o `System` com `fpha_enabled = config.fpha.is_precomputed()`. No modo `computed` isso dá `false`, porque os planos ainda não existem.
2. [study.rs:244](../../crates/labdessem-io/src/study.rs#L244) chama `system.validate()`. Existe uma regra certa em [entities/hydro.rs:311](../../crates/labdessem-core/src/entities/hydro.rs#L311) ("usina com unidades precisa de pelo menos um segmento FPHA"), mas ela só vale com `fpha_enabled = true` e **por isso não é avaliada**.
3. Só depois disso a CLI calcula os planos ([fpha_integration.rs:82-104](../../crates/labdessem-cli/src/fpha_integration.rs#L82-L104)). Usinas com problema são puladas com `eprintln!`. Sem nenhuma geometria, a função retorna vazio e `Ok`.
4. [main.rs:131-135](../../crates/labdessem-cli/src/main.rs#L131-L135) injeta os planos e liga `system.fpha_enabled = true`, **sem validar de novo**.
5. No modelo, com `fpha_enabled = true`:
   - a restrição de produtividade constante não é criada ([constraint_set.rs:41-50](../../crates/labdessem-model/src/constraints/constraint_set.rs#L41-L50));
   - não há hiperplanos para a usina pulada, então nenhuma linha FPHA é gerada;
   - o acoplamento por unidade g_u ≤ ρmax·q_u é pulado porque ρmax = 0 ([constraints/hydro.rs:283](../../crates/labdessem-model/src/constraints/hydro.rs#L283)).
6. Sobram só os limites 0 ≤ g ≤ Pmax e 0 ≤ q ≤ Qmax, **sem relação entre eles**.

**Evidência.**

| Caso | Mensagem | Resultado |
|---|---|---|
| Geometria da FUNIL-GRANDE removida | `Warning: Plant 2 (FUNIL-GRANDE) has no VHA geometry data, skipping` | FUNIL gera 180 MW com turbinamento 0 nos 48 períodos e verte toda a afluência. Exit 0. |
| Geometria da FURNAS removida | `Warning: Plant 3 (FURNAS) has no VHA geometry data, skipping` | FURNAS gera **1216 MW com Q=0**, e o volume **sobe**. Exit 0. |
| Arquivo `CAD_GEOMETRIA_UHE.csv` ausente | `No VHA geometry data found... Processed 0 plants, 4 skipped` | **Todas** as usinas geram de graça. Objetivo **menor** que o caso com água. Exit 0. |

**Assimetria.** No modo `precomputed`, uma usina sem planos em `OPER_FPHA.csv` **dá erro** ([readers/hydro.rs:298-306](../../crates/labdessem-io/src/readers/hydro.rs#L298-L306)). No `computed`, a mesma situação passa em silêncio.

**Correção sugerida (não implementada).** Duas opções, ambas simétricas ao `precomputed`:

- (a) chamar `system.validate()` de novo depois de injetar os planos;
- (c) fazer `compute_fpha` devolver erro quando uma usina com unidades for pulada.

A terceira opção, (b), seria cair para produtividade constante na usina pulada. Ela exige mudar o `ConstraintSet` para funcionar "por usina", e o revisor não recomenda.

**Teste de prova: CONFIRMADO.** No modelo montado sem a geometria da FUNIL-GRANDE, a usina tem 3 unidades e **0 linhas** ligando geração a água. Um teste informativo confirma que `HydroPlant::validate` rejeitaria o caso, mas ele não é chamado depois da injeção.

Na execução, FUNIL gera **180 MW com turbinamento 0** em 48 de 48 períodos, e a execução termina com sucesso. Seção 12.

---

### HI-02 — Com `UCH=0`, a coluna `StatusOn` das hidrelétricas é lida da posição errada

> **Severidade: MÉDIA** · **Veredito: CONFIRMADO** (também MX-07) · **Configs: todas com `UCH=0` (112 de 112 execuções)**

**Em uma frase.** Sem UCH não existe variável de comprometimento hidráulico, mas o código de saída lê "a coluna do comprometimento" mesmo assim. A posição onde essa coluna estaria é ocupada por **outra coisa** (folgas de rede ou de limites operativos), ou fica fora do vetor e vale 0.

**O que acontece hoje.** É um caso clássico de "por nome × por posição" (seção 2.1):

1. [indexing.rs:233-241](../../crates/labdessem-model/src/indexing.rs#L233-L241): com `UCH=0`, o bloco `hydro_commitment` tem tamanho 0. O offset dele é o **mesmo** do bloco seguinte (`network_flow_slack`).
2. [accessor.rs:206-211](../../crates/labdessem-solver/src/accessor.rs#L206-L211) lê `primal[offset + u·T + t]` sem conferir se o UCH está ligado. Fora do vetor, devolve 0 (`unwrap_or(0.0)`).
3. [output_prep.rs:64-67](../../crates/labdessem-cli/src/output_prep.rs#L64-L67) grava `hydro_on[...]` no dicionário **sempre**.
4. O escritor tem um plano B correto, "status = 1 se gera" ([output/hydro.rs:237-258](../../crates/labdessem-io/src/output/hydro.rs#L237-L258)), mas ele nunca é usado, porque a chave existe no dicionário.

**Evidência.**

- **Caso base.** 816 linhas de unidade, 610 com geração positiva, e **todas as 610 com `StatusOn = 0`**. Por exemplo, CAMARGOS 1-1 em t=1 gera 23 MW e aparece desligada.
- **Com folga ativa.** Num caso com uma restrição operativa `GER ≥ 5000` em FURNAS, CAMARGOS 1-1 aparece com `StatusOn = 3784` e `StatusOff = −3783`. 3784 é o valor da **folga** dessa restrição.

**Consequência.** Só a saída está errada; a otimização não muda. Mas o valor é absurdo quando há folga, e quem consumir a coluna vai concluir que nenhuma unidade gerou.

**Correção sugerida (não implementada).** Em `output_prep.rs`, só inserir `hydro_on` quando o UCH estiver ligado. O plano B do escritor passa a funcionar sozinho.

**Teste de prova: CONFIRMADO.** Com UCH=0, o acessor `hydro_commitment(0,0)` devolve **3784**, que é o valor de outra coluna (a primeira folga dinâmica). Com UCH=1 (controle), ele lê a coluna certa. Na execução, as 608 unidades-período que geram saem todas com `StatusOn = 0`. Seção 12.

---

### HI-04 — Produtividade constante: a usina fecha, mas cada unidade pode gerar sem água

> **Severidade: MÉDIA** · **Veredito: CONFIRMADO** (revisor hidro) / **PARCIAL** (revisor FPHA, como FP-04) (também MX-09 e NU-15) · **Configs: `FPHA="constant_productivity"`, que é o default do exemplo (128 de 128 execuções)**

**Em uma frase.** A restrição de produtividade constante é **uma só por usina**: Σg = ρ·Σq. A usina como um todo fecha, mas a divisão entre unidades é arbitrária. Uma unidade pode aparecer gerando sem turbinar, e outra turbinando sem gerar.

**O que acontece hoje.** [constraints/hydro.rs:309-353](../../crates/labdessem-model/src/constraints/hydro.rs#L309-L353) cria uma linha por (usina, t), somando todas as unidades. Já o modo FPHA tem, **além** do envelope da usina, um acoplamento por unidade g_u ≤ ρmax·q_u ([constraints/hydro.rs:261-306](../../crates/labdessem-model/src/constraints/hydro.rs#L261-L306)). A produtividade constante não tem nada equivalente.

**Evidência.**

- Caso default: 249 linhas de unidade com G > 1 MW e Q = 0. Por exemplo, ITUTINGA 1-1 em t=1 gera 12,5 MW com 0 m³/s.
- FURNAS 2-2 em t=2: g = 0 com q = 189 m³/s.
- **Unidades heterogêneas** (ITUTINGA conjunto 2 com `MaxTurb` reduzido de 65 para 5): a usina passa do máximo físico de 43 MW em 18 períodos, chegando a 46 MW. A unidade 2-1 gera 14 MW com turbinamento 0, embora só possa turbinar 5 m³/s.

**Os dois revisores concordam no fato e divergem no rótulo:**

- **Revisor FPHA:** a igualdade por usina **é** a formulação do DESSEM, porque sem UCH a decisão de geração é por usina. Com unidades homogêneas e limitadas por potência, que é o caso do exemplo, a otimização está certa e o erro fica só na saída por unidade.
- **Revisor hidro:** a documentação do projeto (`fpha-formulation.md:42`) fala em produtividade "da unidade", e o caso heterogêneo mostra superestimação real de capacidade.

**Conclusão prática.**

- **Com unidades homogêneas**, é um problema de **saída** (colunas por unidade sem significado físico).
- **Com unidades heterogêneas**, a capacidade é superestimada.

**Decisão sua.** Escolher entre:

- acrescentar o acoplamento por unidade, simétrico ao modo FPHA;
- ou não imprimir Q e G por unidade quando a função de produção é por usina.

**Teste de prova: CONFIRMADO.** Com produtividade constante, as **17 unidades** ficam sem relação g_u ↔ q_u própria. No modo FPHA (controle), a relação existe. Na execução com unidades heterogêneas, uma unidade limitada a 5 MW gera 14 MW com turbinamento 0. Seção 12.

---

### HI-05 (= FP-03) — Com `UCH=1`, unidade desligada continua turbinando — **os revisores divergem**

> **Severidade: MÉDIA** (os dois revisores) · **Veredito: CONFIRMADO** (revisor hidro) **vs. PARCIAL** (revisor FPHA) · **Configs: `UCH=1`**

**O fato, que ninguém contesta.** A canalização do UCH ([channeling.rs:24-63](../../crates/labdessem-uc/src/hydro/constraints/channeling.rs#L24-L63)) só cria duas restrições por unidade:

```
g_u ≤ Pmax_u · on_u
g_u ≥ Pmin_u · on_u
```

O turbinamento q_u não depende de on_u. Resultado: com `UCH=1` há **96 pares** (unidade, período) com `StatusOn = 0` e turbinamento positivo. Por exemplo, CAMARGOS em t=2:

| Unidade | on | g | q |
|---|---|---|---|
| 1-1 | 0 | 0 | 14,83 |
| 1-2 | 1 | 14,83 | 0 |

**Com FPHA, isso tem efeito no despacho.** A FPHA soma o Q de **todas** as unidades. A água "turbinada" por uma unidade desligada eleva o envelope de geração das unidades ligadas. O revisor FPHA mediu **75 usina-períodos** em que a geração passa do que a FPHA permitiria só com a água das unidades ligadas (por exemplo, CAMARGOS t=4: G = 23,00 MW contra FPHA(Q das ligadas) = 7,88 MW).

**Revisor hidro (bug).**

- A falta de q_u ≤ Qmax_u·on_u é defeito.
- O comentário de [indexing.rs:343](../../crates/labdessem-model/src/indexing.rs#L343) reserva 3 linhas por unidade, uma delas chamada `turb_lower`, o que sugere que a restrição de turbinamento estava prevista.

**Revisor FPHA (decisão de projeto).**

- **O DESSEM retirou esse vínculo de propósito na versão 22.1.** O histórico de versões (HistVer 22.1 #4) diz: "Foi removida a consideração do turbinamento por unidade na representação do UCH. Essa modelagem vinculava o desligamento da unidade a turbinamento nulo, impondo uma limitação de vazão pela turbina e inviabilizando o vertimento pela turbina quando necessário."
- O MM 22.3.0 não tem mais q por unidade nem o limite q_min·u ≤ q ≤ q_max·u. Só o Libs §15.6.3 ainda traz esse limite.
- O comentário `turb_lower` fala de limite **inferior** (Q ≥ Qmin·u), não do limite superior que o revisor hidro propõe. Ele não prova que essa linha estava planejada.
- Impor q_u ≤ Qmax_u·on_u pode tornar o problema **inviável** quando a água precisa passar pela turbina.

**Minha leitura.** Os dois revisores estão certos sobre coisas diferentes:

- O revisor FPHA está certo de que **o modelo atual é o do DESSEM ≥ 22.1** e não deve ser "corrigido" às cegas.
- O revisor hidro está certo de que **a saída por unidade é fisicamente incoerente** e de que, com FPHA, a água das desligadas infla a capacidade das ligadas.

O que falta é uma **decisão registrada** (ADR) sobre qual versão da metodologia o UCH do projeto segue.

**Decisão sua (há 3 opções).**

- (a) **Seguir o DESSEM ≥ 22.1** (manter o modelo), documentar, e tornar a saída por unidade coerente. Por exemplo, imprimir Q só no nível da usina. **Recomendação do revisor FPHA.**
- (b) **Seguir o Libs / DESSEM 22.0.2**: q_u ≤ Qmax_u·on_u, aceitando o risco de inviabilidade.
- (c) A opção (b) com uma variável de "vertimento pela turbina", que é o que motivou a remoção no DESSEM.

**Teste de prova: o fato está CONFIRMADO; a severidade depende da D3.** Das 1632 linhas de canalização hidráulica (2 por unidade-período), **nenhuma** contém o turbinamento. Na execução com UCH=1, aparecem 96 casos de unidade desligada com turbinamento positivo. Seção 12.

---

### HI-07 — Desvio de água sem limite e sem custo

> **Severidade: MÉDIA** (revisor hidro elevou de BAIXA) · **Veredito: CONFIRMADO** · **Configs: casos com `Desvio` preenchido no `CAD_UHE`; o exemplo não tem**

**Em uma frase.** Quando uma usina tem destino de desvio, a variável de desvio não tem limite superior nem custo. O otimizador pode despejar o reservatório inteiro em meia hora.

**O que acontece hoje.**

- [variables.rs:149-153](../../crates/labdessem-model/src/variables.rs#L149-L153): limite superior `None` quando há destino de desvio.
- [objective.rs:49-65](../../crates/labdessem-model/src/objective.rs#L49-L65): o vertimento e o turbinamento têm penalidade, mas o desvio não tem custo.
- O documento `docs/design/hydro-modeling.md:98` diz "Diversion is bounded by the plant's diversion capacity". Esse campo não existe no `CAD_UHE`.

**Evidência.**

- **CAMARGOS com `Desvio = 3` (FURNAS).** No período 48, o desvio chega a **373.424,7 m³/s**, e o volume vai de 792 a 120 hm³ em 0,5 h: o reservatório inteiro é despejado em FURNAS. O incentivo vem do coeficiente FCF positivo da CAMARGOS (ver NU-FCF).
- **Mesmo com `FCF=0`**, o desvio chega a 738 m³/s num período, acima do engolimento de 214 m³/s. Ele substitui o vertimento, que é penalizado.

**Correção sugerida (não implementada).**

- Uma coluna de capacidade de desvio no `CAD_UHE`, como o documento promete.
- Ou uma penalidade de desvio igual à do vertimento.

**Decisão sua.** Qual das duas.

**Teste de prova: CONFIRMADO.** O limite superior da variável de desvio é **None**. Na execução, CAMARGOS desvia 373.424,7 m³/s num período e o volume cai de 792 para 120 hm³ em meia hora. Seção 12.

---

### 6.x Achados de severidade BAIXA em hidráulica

| ID | O que é | Por que é BAIXA | Onde |
|---|---|---|---|
| **HI-06** | A barra da usina vem só do 1º conjunto. Um conjunto em outra barra é ignorado: os resultados ficam idênticos, sem aviso. | O modelo de dados tem uma barra por usina, e as térmicas fazem igual (TH-07). Falta validar. | [readers/hydro.rs:323-328](../../crates/labdessem-io/src/readers/hydro.rs#L323-L328) |
| **HI-08** | Nome de usina repetido funde colunas por nome (mesmo mecanismo do TH-R01). Com ITUTINGA renomeada para CAMARGOS, a CAMARGOS real some. | Exige dado malformado. | [conversion.rs:22-26](../../crates/labdessem-solver/src/conversion.rs#L22-L26), [system.rs:68-71](../../crates/labdessem-core/src/system/system.rs#L68-L71) |
| **HI-09** | Offsets de restrições hidráulicas divergem da ordem real: turbinamento intercalado contra blocos, produtividade por usina contra por unidade, canalização 2 contra 3, acoplamento FPHA que pula unidades. | **Latente.** Só `demand_balance` e `hydro_balance` são lidos, e ficam antes. Faz parte do NU-10. | [indexing.rs:283-307](../../crates/labdessem-model/src/indexing.rs#L283-L307), [:343](../../crates/labdessem-model/src/indexing.rs#L343) |
| **HI-R01** | Com `rede=1`, usina só com reservatório (sem unidades) é rejeitada com `missing Barra`, embora não injete nada na rede. | Falha ruidosa, mas rejeita um deck válido. | [readers/common.rs:115-120](../../crates/labdessem-io/src/readers/common.rs#L115-L120) |
| **HI-R02** | Ciclo de cascata (A → B → A) não é detectado. A água recircula. | Dado malformado. O revisor do núcleo classificou o ciclo do exemplo sintético como aceitável. | [system.rs](../../crates/labdessem-core/src/system/system.rs) (validação hidráulica) |
| **HI-R03** | Código desconhecido em `CAD_CONJ_UHE` é descartado e a usina perde as unidades em silêncio. Com `rede=1` o erro aparece como um enganoso `missing Barra`. | Mesma família do HI-01. Não ocorre no exemplo. | [readers/hydro.rs:193-215](../../crates/labdessem-io/src/readers/hydro.rs#L193-L215) |
| **HI-R04** | `docs/design/hydro-modeling.md` descreve campos que não existem: turbinamento e vertimento iniciais, turbinamento mínimo, capacidade de vertedouro, capacidade de desvio, eficiência de bombeamento. A fórmula do desvio também está errada. | Documentação. | [hydro-modeling.md](../../docs/design/hydro-modeling.md) |

**Verificado e correto em hidráulica.** O verificador da matriz passou nas 224 execuções:

- balanço hídrico com erro ≤ 1e-6 hm³;
- topologia de montante (turbinamento + vertimento de montante, desvio para o destino, bombeamento);
- conversão m³/s ↔ hm³ em todas as colunas;
- limites de volume e turbinamento;
- agregado `99;99` igual à soma das unidades;
- volume útil e percentual.

O revisor hidro acrescentou:

- **Períodos de duração variável** funcionam: período 1 de 1,5 h dá V1 = 786,536 hm³, calculado à mão e igual ao impresso.
- **O PiBalHidr é lido da linha certa**, com sinal coerente: positivo quando a água extra é vertida com penalidade, por exemplo CAMARGOS = 0,2 = `PenalVert`.

---

## 7. FPHA (os três modos) e EngMax

### 7.0 O mínimo de modelagem para ler esta seção

**FPHA, a Função de Produção Hidrelétrica Aproximada.** A potência de uma usina depende da vazão turbinada **e** da altura de queda, que por sua vez depende do volume (cota do reservatório) e da vazão total defluente (cota do canal de fuga). Essa função é não linear. O DESSEM a aproxima por um conjunto de **hiperplanos**, e a geração precisa ficar abaixo de todos eles:

```
G ≤ α · (γ0 + γV·V + γQ·Q + γS·S)     para cada plano k
```

| Símbolo | Significado |
|---|---|
| γ0, γV, γQ, γS | coeficientes do plano k |
| α | **fator de correção**: um número ligeiramente menor que 1, que corrige o viés sistemático dos planos (que ficam "por cima" da função real) |

Na forma do Libs/DESSEM, o α multiplica os coeficientes **brutos**, uma única vez.

**Os três modos:**

| Modo | De onde vêm os planos e o α |
|---|---|
| `precomputed` | Planos e α lidos de `OPER_FPHA.csv` (colunas de coeficientes + `Fcorrec`). |
| `computed` | O labdessem calcula os planos a partir da geometria (`CAD_GEOMETRIA_UHE.csv`, `CAD_TAILRACE.csv`) com o crate `labdessem-fpha` (qhull), calcula o α e os injeta no `System`. |
| `constant_productivity` | Não há planos: G = ρ·Q (seção 6). |

**EngMax, o engolimento máximo.** Quando o reservatório está baixo, a queda é menor e a turbina não consegue engolir a vazão nominal. O EngMax limita Σq da usina a um valor que depende do volume. No labdessem, esse limite é **recalculado entre um solve e o próximo** a partir da solução anterior (regra R1, ADR-017). Se ficar inviável, a execução aborta com erro explícito: o EngMax não tem folga, por desenho.

---

### FP-02 — FPHA "computed": o fator de correção α é aplicado duas vezes

> **Severidade: ALTA** · **Veredito: CONFIRMADO** (revisor FPHA, com reprodução própria) · **Configs: `FPHA="computed"`, qualquer usina com α < 1**

**Em uma frase.** No modo `computed`, os planos que saem do ajuste **já vêm multiplicados por α**. Mesmo assim, o labdessem os entrega ao modelo com `correction_factor = α`, e o modelo multiplica por α **de novo**. O envelope efetivo fica α²·FPHA em vez de α·FPHA.

**O caminho do α:**

1. [pipeline.rs:84-85](../../crates/labdessem-fpha/src/pipeline.rs#L84-L85) calcula `alpha` e faz `planes = scale_planes(&raw_planes, alpha)`: **primeira** multiplicação.
2. Esses planos escalados são devolvidos em `FphaPlantResult.planes`.
3. [fpha_integration.rs:313-320](../../crates/labdessem-cli/src/fpha_integration.rs#L313-L320) cria os segmentos com `correction_factor: result.alpha`.
4. [constraints/hydro.rs:227-252](../../crates/labdessem-model/src/constraints/hydro.rs#L227-L252) multiplica cada coeficiente e o lado direito por `correction_factor`: **segunda** multiplicação.

**Como o modo `precomputed` faz.** Ele lê os coeficientes brutos e o `Fcorrec` separadamente, e o modelo multiplica uma vez. Está certo e coerente com o Libs. O problema é só no `computed`, porque ele entrega planos já escalados **e** o α.

**Evidência (reprodução do revisor).** A métrica é a razão G / min_k(plano impresso), nas linhas `99;99` com Q > 0:

| Usina | α | Razão no `computed` | Razão no `precomputed` com os **mesmos** planos e `Fcorrec=1` |
|---|---|---|---|
| CAMARGOS | 0,969607 | **0,969608** | 1,000000 |
| FURNAS | 0,991295 | **0,991296** | 1,000000 |
| ITUTINGA, FUNIL | 1,000000 | 1,000000 | 1,000000 |

A razão é **exatamente α**: o modelo está usando α × (plano impresso) = α²·FPHA₀. O objetivo do `computed` sai **+3.141,8 R$** acima do `precomputed` com os mesmos planos.

**Agravante.** Um teste existente, [fpha_integration.rs:394-424](../../crates/labdessem-cli/src/fpha_integration.rs#L394-L424) (`convert_single_plane`), **exige** `correction_factor == alpha` e trava o comportamento errado (FP-R03). Nenhum teste cobre o produto efetivo `cf × coef`.

**Correção sugerida (não implementada).**

- (a) `correction_factor: 1.0` na conversão. É uma linha, mas o γS (que é ajustado sem α, FP-08) fica sem α, divergindo do Libs.
- (b) **Tirar o `scale_planes` do pipeline e manter `cf = α`.** Fica igual ao Libs e ao formato do `OPER_FPHA`, e o α passa a ser aplicado também ao γS. Exige ajustar o cálculo de desvio ([pipeline.rs:101](../../crates/labdessem-fpha/src/pipeline.rs#L101)) e o cabeçalho de `resultado_fpha_computed.csv`. **Recomendação do revisor.**

Nos dois casos, o teste `convert_single_plane` precisa ser trocado por um teste sobre o produto `cf × coef`.

**Decisão sua.** (a) ou (b), junto com o FP-08.

**Teste de prova: CONFIRMADO por execução** (não há teste Rust). Em cada usina, a razão máxima entre a geração e o plano impresso é **exatamente α**:

- CAMARGOS: 0,969606, com α = 0,969607;
- FURNAS: 0,991296, com α = 0,991295.

No `precomputed` com os mesmos planos, a razão é 1. O objetivo do `computed` fica +3.125,65 R$ acima. Seção 12.

---

### FP-05 — `CAD_FPHA_CONFIG.csv` é lido, mas seus campos são ignorados

> **Severidade: MÉDIA** · **Veredito: CONFIRMADO** · **Configs: `FPHA="computed"` com `CAD_FPHA_CONFIG.csv` presente**

**Em uma frase.** O arquivo é anunciado no README, no CHANGELOG e em `docs/docs.html`, e o log diz `Lendo arquivo: CAD_FPHA_CONFIG.csv`, mas número de planos, tolerância e janela de volume não mudam nada.

**Evidência no código.**

- [readers/fpha_geometry.rs:194-205](../../crates/labdessem-io/src/readers/fpha_geometry.rs#L194-L205) marca `num_planes`, `tolerance`, `min_volume_pct` e `max_volume_pct` com `#[allow(dead_code)]`: o próprio código admite que não os usa.
- Só `num_planes` é copiado, para `max_planes` ([fpha_integration.rs:255-269](../../crates/labdessem-cli/src/fpha_integration.rs#L255-L269)), e esse valor não é lido por nenhum código de produção.

**Reprodução.** Linha `3;2;5.0;0.5;0.6` (FURNAS: 2 planos, janela de 50 % a 60 % do volume útil). Resultado: FURNAS com **6 planos idênticos, byte a byte**, aos do caso sem o arquivo.

**Decisão sua.** Implementar os campos ou retirar o arquivo da documentação.

**Teste de prova: CONFIRMADO por execução.** Três versões do `CAD_FPHA_CONFIG` geram os mesmos 6 planos e o mesmo objetivo (4.502.332,7276). Seção 12.

---

### 7.x Achados de severidade BAIXA em FPHA/EngMax

| ID | Veredito | O que é | Observação |
|---|---|---|---|
| **FP-06** | PARCIAL | A FPHA usa o volume do **fim** do período (V[t+1]). | Segue o MM (eq. 9.33, em que V^t é o volume do fim) e diverge da forma do Libs (volume médio) e do ADR-006 do próprio EngMax, que usa o médio. Efeito ≤ 0,25 MW no exemplo. **Decisão sua:** registrar em ADR qual volume a FPHA usa. O revisor recomenda o médio. |
| **FP-07** | CONFIRMADO | No `precomputed`, uma usina **sem unidades** com linhas no `OPER_FPHA` gera restrição `−γV·V − γS·S ≤ γ0`. Com γ0 > 0 e γS < 0, o problema fica inviável. | Entrada atípica. |
| **FP-08** | PARCIAL | O γS é ajustado sobre a FPH **sem** α, depois que os outros coeficientes já foram escalados. | Se é erro depende de como o FP-02 for corrigido. Com a opção (b), o γS recebe α uma vez, como no Libs. |
| **FP-09** | PARCIAL | `GeracaoMaxPontoMW` ignora o EngMax. | Só saída. A coluna pode mostrar uma capacidade que o modelo não deixa alcançar. |
| **FP-10** | CONFIRMADO, latente | Contagens erradas em `ConstraintOffsetParams` (produtividade, acoplamento, canalização). | Faz parte do NU-10. |
| **FP-11** | CONFIRMADO | `docs/design/fpha-formulation.md` desatualizado: cita `FPHA = 0/1`, um arquivo `readers/fpha.rs` que não existe e nomes de colunas antigos. Não fala do `computed` nem do α. | Documentação. |
| **FP-12** | PARCIAL | A média de longo termo (MLT) é uma média simples, sem ponderar por dias do mês. `MissingHydro` é fatal até para usinas sem unidades. | Efeito < 0,5 % no exemplo. |
| **EM-04** | CONFIRMADO | NaN na geometria (por exemplo `a0 = NaN` no `CAD_TAILRACE`) vira EngMax = 0 sem erro. A usina fica impedida de turbinar, e só aparece um aviso enganoso de "non-positive net head". | Validação de entrada. |
| **FP-R01** | NOVO | Linha duplicada da mesma usina em `CAD_TAILRACE` ou `CAD_FPHA_CONFIG`: vale a última, sem aviso. | Validação. |
| **FP-R02** | NOVO | `computed` sem as colunas `Eficiencia` ou `Perda.m` usa η = 1 e perda 0 sem aviso. A FPH fica superestimada em cerca de 9 % em FURNAS. | Validação. Deveria ao menos avisar. |
| **FP-R03** | NOVO | O teste `convert_single_plane` trava o FP-02. | Nota de teste. |

### 7.y Achados de EngMax que **não** são defeito

| ID | Veredito | Alegação original | Por que não é defeito |
|---|---|---|---|
| **EM-01** | **REFUTADO** | "Com `FPHA=computed`, `UCT=1` e `EngMax=1` o exemplo aborta (32 execuções da matriz)." | O erro rígido é o **ADR-017**, aceito. A causa é **física**: FURNAS começa com **0,59 % do volume útil**. A conta à mão dá queda líquida 76,3 m contra queda de referência 89,8 m, logo EngMax = 1506 × (76,3/89,8)^0,5 = **1388 m³/s**, menor que os 1506 m³/s de que o despacho precisa. Sem variável de déficit (RD-05/MX-05), qualquer limite físico ativo vira inviabilidade. Com produtividade constante o caso roda porque ρ = 1,0 no CAD, contra cerca de 0,81 real da FURNAS. O que sobra é lacuna de **cobertura**: não há caso de exemplo que exercite EngMax com FPHA física. |
| **EM-02** | INCONCLUSIVO | "O limite do EngMax muda entre o MILP e o LP-CALC-CMO." | O mecanismo existe (regra R1). Mas todas as saídas, despacho e duais, vêm do LP-CALC-CMO, então não há "CMO calculado sobre outro despacho". Duas tentativas de provocar um problema não reproduziram nada. A matriz registra o efeito colateral como MX-12 (objetivos de etapas incomparáveis), BAIXA. |
| **EM-03** | **REFUTADO** | "Usina sem unidades sai com `EngMaxHM3 = 0`." | Documentado em [engmax.rs:268](../../crates/labdessem-simulation/src/engmax.rs#L268) e fisicamente correto: sem turbina, engolimento zero. |

**Verificado e correto em FPHA/EngMax** (matriz, 96 execuções `computed`):

- toda geração fica dentro do envelope impresso (folga máxima de 1,7e-4 MW);
- o acoplamento por unidade g_u ≤ ρmax·q_u vale;
- Σq ≤ EngMax vale em todos os períodos, e as colunas de EngMax ficam vazias com `EngMax=0`;
- o EngMax relata 0 excedências em todas as execuções OK.

O envelope impresso é o α·FPHA₀. Por causa do FP-02, o modelo aplica mais um α por cima.

---

## 8. Rede elétrica, CMB e CMO

### 8.0 O mínimo de modelagem para ler esta seção

Com `rede=1`, o labdessem representa a rede elétrica pelo **modelo DC**. O fluxo em cada linha é uma combinação linear das injeções líquidas nas barras:

```
fluxo_l = Σ_i PTDF(l,i) · (geração_i − carga_i)
```

A **PTDF** (Power Transfer Distribution Factor) de uma linha l em relação a uma barra i diz quanto do MW injetado em i passa por l. A barra **swing** (barra 1 no exemplo) é a referência: a coluna PTDF dela é zero, e ela fecha o balanço.

**Cortes de fluxo.** Para cada linha *l* e período *t* que precisam de limite, o labdessem acrescenta duas restrições, definidas em [helpers.rs:181-197](../../crates/labdessem-simulation/src/stages/helpers.rs#L181-L197):

```
flow_cut_upper:   Σ PTDF·g  ≤  +F_l + Σ PTDF·d        (fluxo ≤ +capacidade)
flow_cut_lower:   Σ PTDF·g  ≥  −F_l + Σ PTDF·d        (fluxo ≥ −capacidade)
```

**Os preços:**

- **λ_DS (PiDemanda)** é o dual do balanço de demanda do submercado, uma igualdade.
- **CMB (Custo Marginal de Barra)** responde quanto custa 1 MW a mais de carga na barra i:

  ```
  CMB_i = λ_DS(s_i) + Σ_l PTDF(l,i) · (μ⁺_l + μ⁻_l)
  ```

  Aqui μ⁺ e μ⁻ são os duais dos cortes superior e inferior, **na convenção ∂custo/∂lado direito**. Quando nenhuma linha está no limite, os μ são zero e o CMB de todas as barras é igual a λ_DS.

- **CMO por submercado** = média dos CMB das barras do submercado, ponderada pela carga ([duals/cmo.rs](../../crates/labdessem-network/src/duals/cmo.rs)).

---

### RD-01 — Com uma linha congestionada no limite inferior, o CMB e o CMO saem com a componente de congestionamento **invertida**

> **Severidade: CRÍTICA** · **Veredito: CONFIRMADO por três provas independentes** (também NU-02 e MX-01) · **Configs: `rede=1`, os dois modos, sempre que alguma linha fica presa em −capacidade no LP-CALC-CMO**

**Em uma frase.** A biblioteca `good_lp` 1.15.0 guarda toda restrição `≥` virada ao contrário (`−a ≤ −b`). O HiGHS devolve o dual da linha virada, e o labdessem o usa como se fosse o da linha original. O dual do corte `flow_cut_lower` chega **com o sinal trocado**, e o CMB soma esse termo com o sinal errado.

**Por que isso é o achado mais grave.**

- O CMB e o CMO são **o** produto de preço do modelo.
- O erro é **silencioso**: não há aviso, e o valor é plausível.
- Ele aparece na **configuração padrão** assim que a rede congestiona.
- Ele faz o CMO depender de **qual ponta da linha foi declarada como "De"**, o que é fisicamente arbitrário.

**A cadeia, passo a passo.**

1. [helpers.rs:186-195](../../crates/labdessem-simulation/src/stages/helpers.rs#L186-L195) cria o corte inferior como `Σ PTDF·g ≥ −F + shift`.
2. [backends/highs.rs:176-177](../../crates/labdessem-solver/src/backends/highs.rs#L176-L177) o envia com `constraint!(expr >= row_lb)`.
3. No good_lp 1.15.0, travado em [Cargo.lock:604-606](../../Cargo.lock#L604-L606), `geq(a, b)` é implementado como `leq(b, a)`. A linha chega ao HiGHS como `−expr ≤ −row_lb`, e o sentido original não é guardado.
4. O HiGHS minimiza e devolve y = ∂Z/∂(lado direito **que recebeu**) = ∂Z/∂(−rhs) = **−∂Z/∂rhs**.
5. O good_lp devolve esse número cru (`dual_rows()[i]`), sem desfazer a inversão.
6. [extraction.rs:37](../../crates/labdessem-network/src/duals/extraction.rs#L37) soma `dual[upper] + dual[lower]` como se os dois estivessem na mesma convenção, e [cmb.rs](../../crates/labdessem-network/src/duals/cmb.rs) aplica `CMB = λ_DS + Σ PTDF·μ`.

Em vez de μ⁺ + μ⁻, o código calcula μ⁺ − μ⁻. Com a linha no limite **superior**, μ⁻ = 0 e o resultado está certo. Com a linha no limite **inferior**, μ⁺ = 0 e o termo entra **com o sinal trocado**.

**Prova 1: sonda mínima.** O revisor montou LPs de uma variável com o próprio `HighsSolver` do projeto:

| LP | ∂Z/∂rhs correto | Dual devolvido |
|---|---|---|
| min x, sujeito a x ≥ 3 | +1 | **−1** |
| min −x, sujeito a x ≤ 3 | −1 | −1 |
| min x, sujeito a x = 3 | +1 | +1 |

Só a linha `≥` volta com o sinal trocado.

**Prova 2: diferenças finitas.** O preço de uma barra é a variação do custo quando se acrescenta carga nela. O revisor acrescentou ±0,5 MW na barra 20 (e no `OPER_SBM`) e mediu Δcusto diretamente, sem usar dual nenhum. O caso tem as linhas 23 e 24 com 120 MW (`k0`). O caso `k0rev` é o mesmo com De/Para dessas linhas trocados:

| Barra | CMB impresso em `k0` | CMB impresso em `k0rev` | **Diferença finita** (verdade) |
|---|---|---|---|
| 20 (Fic2) | 277,4007 | 94,0517 | **94,0517** |
| 11 (Fic1) | 116,7040 | 254,8484 | **254,8485** |
| 14 (Fic1) | 18,4355 | 353,1170 | **353,1169** |

Em `k0rev` a linha fica no limite **superior**, e o CMB bate com a diferença finita. Em `k0`, o mesmo problema físico, com a linha declarada ao contrário, fica no limite **inferior**, e o CMB sai espelhado em torno de λ_DS.

**Prova 3: o CMO depende da orientação das linhas.** O custo total é **idêntico** nos dois casos (8.796.911,5340), porque o despacho é o mesmo. O CMO não é:

| Caso | CMO Fic2, período 11 |
|---|---|
| `k0` (linhas 23 e 24 no limite inferior) | **674,17** |
| `k0rev` (as mesmas linhas, declaradas ao contrário) | 68,74 ← correto |

**Na configuração padrão do exemplo** (opção 2, UCT=1, EngMax=1, FCF=1), alterando só as capacidades das linhas 23 e 24 para 120 MW, o CMO do período 20 fica assim:

| Submercado | Linhas declaradas como no exemplo | Linhas declaradas ao contrário |
|---|---|---|
| Fic1 | 280,85 | 319,15 |
| Fic2 | 388,14 | 211,66 |

O mesmo acontece na opção 1.

**A matriz confirmou o mesmo defeito por outro caminho.** Nos experimentos com todas as linhas a 250 e 175 MW, a condição de otimalidade "térmica marginal fora da swing ⇒ CMB da barra = CVU" falha com desvio de até **577 R$/MWh**. Invertendo o sinal **só** das linhas no limite inferior, o desvio cai para **0,000**. Exemplo (cong175, t=45, Fic2): CMO impresso **1533,69**, CMO corrigido **156,37**.

**Por que o exemplo versionado não mostra o problema.** As linhas de 600 MW nunca congestionam. Nenhuma das 112 execuções da matriz com `rede=1` tem linha no limite. Nenhum teste do repositório fixa o sinal do CMB.

**O que não é afetado.**

- O **despacho** e o **custo total**: a inversão de uma linha não muda o problema, só a leitura do dual.
- O **CMO sem rede** (dual de uma igualdade).
- O **valor da água** (dual de uma igualdade).
- Os outros duais de linhas `≥` (FCF, limite operativo inferior, UC), que hoje ninguém lê.

**Cuidado ao corrigir (RD-R01).** O `Cargo.toml` pede `good_lp = "1.15"`, com caret. A versão 1.15.3, que já está no cache do cargo, **corrige** a inversão. Um `cargo update` muda o sinal de **todos** os duais `≥` de uma vez. Se alguém negar o dual em `highs.rs` e depois atualizar o good_lp, o erro volta (dupla negação). A correção precisa ser feita **por uma única via**:

- atualizar para `>= 1.15.3` e travar a versão;
- **ou** negar o dual e travar `= 1.15.0`.

Nos dois casos, com o teste unitário da sonda protegendo o sinal.

**Decisão sua.** Qual das duas vias.

**Teste de prova: CONFIRMADO, nos dois níveis.**

- **No solver isolado,** o dual de `x ≥ 3` sai **−1**, e a diferença finita dá +1. O controle mostra que esse é exatamente o dual de `−x ≤ −3`.
- **Ponta a ponta,** inverter só a declaração De/Para de uma linha muda a CMO em **96 de 96** pares (período, submercado), enquanto objetivo e fluxos não mudam (controle).
- **CMB com a linha no limite inferior:** a barra 20 sai com 277,40 impresso contra 94,05 da diferença finita. Com a linha declarada ao contrário, impresso e diferença finita coincidem nas três barras testadas.

O teste `audit_rd_r01_good_lp_requirement_is_pinned` registra o risco do `cargo update`. Seção 12.

---

### RD-02 — O CMB não é dividido pela duração do período: no exemplo, sai pela **metade**

> **Severidade: ALTA** · **Veredito: CONFIRMADO** (também parte de MX-02 e NU-05) · **Configs: todas com `rede=1` (112 de 112 execuções)**

**Em uma frase.** O custo térmico na FO é CVU × g × Δt. O dual bruto, portanto, está em R$/MW **por período**. O código divide o CMO por Δt para chegar em R$/MWh, mas **esquece de dividir o CMB**.

**Código.** [helpers.rs:483-489](../../crates/labdessem-simulation/src/stages/helpers.rs#L483-L489): `compute_cmb_per_bus` usa os duais crus, e só o CMO é dividido por `period_duration_hours`. [conversions.rs:145](../../crates/labdessem-cli/src/conversions.rs#L145) grava o CMB como veio.

**Evidência.** Exemplo versionado, sem congestionamento (o CMB deveria ser igual ao CMO):

| Grandeza | Valor |
|---|---|
| CMO Fic1, período 1 | 0,10288 |
| CMB de qualquer barra Fic1, "período 0" | **0,0514** (= 0,10288 × 0,5) |

Com térmica marginal, o CMB impresso é 203,67 contra CVU 407,34. Dividindo por 0,5, os checks M03, M04 e M05 da matriz fecham com diferença ≤ 1e-4.

**Documento.** `docs/design/cmb-cmo-formulation.md` promete "CMB in R$/MWh".

**Severidade.** Mantida ALTA: é sistemático em toda execução com rede, num produto de preço. Seria MÉDIA se o CMB for considerado apenas diagnóstico.

**Teste de prova: CONFIRMADO.** Sem congestionamento, **1104 de 1104** linhas do cmobar têm CMB = CMO/2 (150 contra 300). O controle confirma que CMB ÷ Δt = CMO = PiDemanda. Seção 12.

---

### RD-04 — Duas bases de demanda diferentes: o balanço usa `OPER_SBM`, a rede usa `OPER_CARGA_BARRA`, e a diferença (≈ 20 %) vai para a barra swing

> **Severidade: ALTA** · **Veredito: CONFIRMADO** (também MX-04) · **Configs: todas com `rede=1`. O dado inconsistente está no exemplo versionado.**

**Em uma frase.** No exemplo, a demanda do submercado (`OPER_SBM`) é cerca de 270 MW maior que a soma das cargas das barras (`OPER_CARGA_BARRA`). O balanço de energia atende `OPER_SBM`, mas a rede só "vê" as cargas de barra. A geração excedente é retirada implicitamente na barra swing, como se houvesse uma carga fantasma de 270 MW na barra 1.

**Como deveria ser.** No DESSEM, com rede, a demanda do submercado **é** a soma das cargas das barras: D_k = Σ_{i∈k} d_i (MM eq. 7.3). Não existem duas bases de demanda.

**Código.**

- O balanço usa `submarket.demand_mw` ([demand.rs:30](../../crates/labdessem-model/src/constraints/demand.rs#L30)).
- Os fluxos e os cortes usam `bus.demand_mw` ([readers/network.rs:72-75](../../crates/labdessem-io/src/readers/network.rs#L72-L75), `flow/constant_shift.rs`).
- Ninguém confere que as duas somas batem ([system.rs:135-145](../../crates/labdessem-core/src/system/system.rs#L135-L145)).

**Evidência.**

| Período | OPER_SBM − Σ barras (MW) | Retirada implícita na swing, reconstruída da saída (MW) |
|---|---|---|
| 1 | 271,68 | 271,678 |
| 24 | 300,88 | 300,879 |
| 48 | 299,61 | 299,613 |

A retirada fantasma na barra 1 bate com a diferença até a 3ª casa decimal.

- Escalando as cargas de barra para fechar com `OPER_SBM`, os fluxos mudam até **170 MW**, em linhas de 600 MW.
- Com rede apertada (capacidade de 250 MW), de 37 a 65 pares linha × período excederiam a capacidade.
- Parte da demanda de Fic2 (≈ 60 MW) é retirada numa barra de Fic1, então o intercâmbio entre submercados deixa de corresponder ao fluxo físico na fronteira.

**Pode ser intencional (perdas)?** O revisor tentou essa explicação e a descartou:

- não há contrato documentado;
- 20 % é muito mais que perdas típicas;
- o DESSEM não tem duas bases.

**Correção sugerida (não implementada).**

- Validar Σ d_i = D_s na leitura, com tolerância.
- **Ou** derivar D_s das barras quando `rede=1`.

O dado do exemplo precisa ser corrigido de qualquer forma.

**Decisão sua.** Qual é a base de demanda oficial com rede: as barras (como no DESSEM) ou o submercado?

**Teste de prova: CONFIRMADO.** Os 96 pares (submercado, período) do exemplo são inconsistentes. A retirada implícita na barra swing é **exatamente** a diferença `OPER_SBM − Σ OPER_CARGA_BARRA`: 271,68 MW em t=1 e até 391,5 MW. No controle com um deck consistente, a retirada é 0,0001 MW. Seção 12.

---

### RD-05 (= MX-05) — Não existe variável de déficit: qualquer aperto vira "LP is infeasible", sem resultado

> **Severidade: MÉDIA** (revisor de rede) / ALTA (matriz) · **Veredito: PARCIAL** · **Configs: todas. É o modo de falha das 32 execuções `computed + UCT=1 + EngMax=1`.**

**O fato.** Não há variável de déficit (corte de carga) nem usina fictícia de déficit. O balanço de demanda é uma igualdade estrita ([demand.rs:22-31](../../crates/labdessem-model/src/constraints/demand.rs#L22-L31)). Se não houver geração suficiente, a execução aborta com `Error: LP is infeasible`, sem nenhum arquivo em `RESULTADOS/` e sem diagnóstico de submercado ou período.

**Por que só PARCIAL.**

- No DESSEM **com rede** também não há corte de carga (MM §18.2), e os decks oficiais do ONS rodam com déficit desabilitado.
- A falha é **ruidosa** (aborta). Ela não corrompe resultado nenhum.

**Por que a matriz chamou de ALTA.** As 32 combinações `FPHA=computed + UCT=1 + EngMax=1` abortam por isso. A conta à mão no período 1 mostra que faltam **67 MW**: Fic2 precisa de 1303,3 MW hidráulicos e o máximo com FPHA física e EngMax é 1235,9 MW. Sem déficit, não há como o caso rodar.

**Decisão sua.**

- (a) Manter sem déficit, documentar e melhorar o diagnóstico de inviabilidade.
- (b) Acrescentar déficit só com `rede=0`.
- (c) Acrescentar déficit sempre, com custo alto.

---

### RD-07 — Par de submercados sem limite declarado vira intercâmbio **ilimitado e gratuito**

> **Severidade: MÉDIA** · **Veredito: PARCIAL** · **Configs: `rede=0` com `OPER_LIMITE_INTERCAMBIO` incompleto**

**O fato.** O modelo cria uma variável de intercâmbio para todo par ordenado de submercados. Se o par não está em `OPER_LIMITE_INTERCAMBIO.csv`, a variável fica **sem limite superior e sem custo**:

- [variables.rs:239-246](../../crates/labdessem-model/src/variables.rs#L239-L246);
- [objective.rs:71-80](../../crates/labdessem-model/src/objective.rs#L71-L80);
- [demand.rs:46-51](../../crates/labdessem-model/src/constraints/demand.rs#L46-L51).

**Evidência.**

| Caso | Limites declarados | FO | CMO período 24 (Fic1 / Fic2) |
|---|---|---|---|
| `ich0` | 1→2 = 10 **e** 2→1 = 10 | 11.167.805,93 | 931,56 / 0,00288 |
| `ich` | **só** 1→2 = 10 | **1.343.528,33** | 300 / 300 |

Omitir uma linha do arquivo derruba o custo em **88 %**.

**Atenuantes.** Com `rede=1`, a rede física limita indiretamente o intercâmbio. No DESSEM, capacidade **em branco** tem default ∞, mas isso trata de campo vazio, não de registro ausente.

**Decisão sua.** Par não declarado deve ser ilimitado (documentar), proibido (limite 0) ou erro de leitura?

---

### RD-03, RD-06, RD-08, RD-15 — Problemas médios de rotulagem e agregação

| ID | O que é | Evidência | Onde |
|---|---|---|---|
| **RD-03** | `resultado_cmobar.csv` numera os períodos de **0** a 47, enquanto `cmosist` e `rede` usam 1 a 48. Com durações variáveis, o rótulo é o período **interno**, sem agregação: com o período 1 de 2 h, o cmobar vai de 0 a 50, `rede` de 1 a 51 e `cmosist` de 1 a 48. | Junções CMB × CMO por período saem deslocadas em 1. | [helpers.rs:437](../../crates/labdessem-simulation/src/stages/helpers.rs#L437), [conversions.rs:141](../../crates/labdessem-cli/src/conversions.rs#L141) |
| **RD-06** | A barra swing não aparece no `cmobar` (23 × 48 linhas em vez de 24 × 48) e fica fora da média ponderada do CMO. O documento (eq. 2.44) manda somar todas as barras. | Viés pequeno: Fic1 372,3599 sem a swing, 372,3095 com ela. | [cmb.rs:41](../../crates/labdessem-network/src/duals/cmb.rs#L41), [cmo.rs:23-51](../../crates/labdessem-network/src/duals/cmo.rs#L23-L51) |
| **RD-08** | `resultado_rede.csv` e os arquivos de inviabilidade dizem "periodo original" no cabeçalho, mas trazem o período **interno**. | Restrição no período original 3 aparece como 6 quando o período 1 tem 2 h. Sem efeito com durações iguais. | [computation.rs:27](../../crates/labdessem-network/src/flow/computation.rs#L27), [helpers.rs:272](../../crates/labdessem-simulation/src/stages/helpers.rs#L272) |
| **RD-15** | A barra (e, nas térmicas, o submercado) da usina vem só da 1ª unidade. `CAD_UNID_UTE` permite barra por unidade, e no DESSEM unidades de uma usina em barras diferentes existem. Revisor elevou de BAIXA. | A geração da unidade 2 é injetada na barra da unidade 1, e os fluxos saem errados em silêncio. Mesmo mecanismo de TH-07 e HI-06. | [readers/thermal.rs:176-186](../../crates/labdessem-io/src/readers/thermal.rs#L176-L186), [readers/hydro.rs:323-328](../../crates/labdessem-io/src/readers/hydro.rs#L323-L328) |

**Testes de prova: todos CONFIRMADOS.**

- **RD-03:** o cmobar rotula os períodos de 0 a 47; com um período de 2 h, de 0 a 50.
- **RD-06:** 23 registros de CMB para 24 barras; a CMO difere em até 4,87 R$/MWh num caso congestionado.
- **RD-08:** período 6 reportado para uma restrição do período original 3.

Os controles passam. Seção 12.

### 8.x Achados de severidade BAIXA em rede

| ID | O que é | Onde |
|---|---|---|
| **RD-09** | Com `rede=0`, é obrigatório existir um submercado de código 1, que vira a referência angular. Sem ele, a execução aborta com uma mensagem obscura. | [readers/network.rs:34](../../crates/labdessem-io/src/readers/network.rs#L34) |
| **RD-10** (= MX-13) | A coluna `Violacao` não tem tolerância: um fluxo de −250,0000 com limite 250 aparece com `Violacao=1`, enquanto o resumo diz "Violations: 0". | [conversions.rs:83](../../crates/labdessem-cli/src/conversions.rs#L83) |
| **RD-11** | O intercâmbio entre submercados não é gravado em nenhum arquivo. | [orchestrator.rs:46-75](../../crates/labdessem-io/src/output/orchestrator.rs#L46-L75) |
| **RD-12** | Penalidade única de 1e6 para toda folga (rede e limites), sem Δt e sem distinguir unidades. Só atua em caso já inviável. Com folga ativa, o CMO chega a 2,19e6 R$/MWh sem marcação no `cmosist`. | [common.rs:6](../../crates/labdessem-simulation/src/stages/common.rs#L6) |
| **RD-13** | Offsets de linha divergem do modelo depois de `hydro_balance` (latente, parte do NU-10). | [indexing.rs:278-280](../../crates/labdessem-model/src/indexing.rs#L278-L280) |
| **RD-14** (= NU-08) | O backend envia linha de faixa (lb e ub finitos) só com o limite superior, e **pula** linha livre, o que desalinharia os duais dos cortes. Latente: a conversão nunca gera esses casos hoje. | [highs.rs:169-181](../../crates/labdessem-solver/src/backends/highs.rs#L169-L181) |
| **RD-16** | Submercado ou renovável sem dados recebe 0 em silêncio. Linha duplicada sobrescreve. | [readers/submarket.rs:31-48](../../crates/labdessem-io/src/readers/submarket.rs#L31-L48) |
| **RD-17** | Ilha elétrica ou barra isolada gera um `SingularMatrix` genérico, sem dizer qual barra. | [linalg/solve.rs:41-42](../../crates/labdessem-network/src/linalg/solve.rs#L41-L42) |
| **RD-R01** | O sinal dos duais `≥` depende da versão do good_lp que o cargo resolve (ver RD-01). | [labdessem-solver/Cargo.toml:11](../../crates/labdessem-solver/Cargo.toml#L11) |
| **RD-R02** | `model_to_lp_template` descarta **em silêncio** termos cujo nome de variável não existe. Um renomeio futuro de variável faria um corte de fluxo ou um termo da FO sumir sem erro. | [conversion.rs:49-53](../../crates/labdessem-solver/src/conversion.rs#L49-L53), [:73-77](../../crates/labdessem-solver/src/conversion.rs#L73-L77) |
| **RD-R03** | Acessores `network_flow_slack` e `operational_limit_slack` com o mesmo offset e sem uso. Código morto. | [accessor.rs:214-226](../../crates/labdessem-solver/src/accessor.rs#L214-L226) |

**Verificado e correto em rede.** O verificador da matriz passou nas 112 execuções com `rede=1`:

- **Fluxos.** Recalculados com uma PTDF própria e independente, batem com os impressos (erro máximo de 5e-5 MW).
- **Limites.** Nenhum fluxo acima da capacidade.
- **Mapeamento corte → linha da matriz** correto: os cortes são as últimas linhas, e EngMax e marcadores entram antes.
- **Domínio das variáveis no LP-CALC-CMO.** As variáveis de UC são contínuas e fixadas, e não binárias, então o backend não cai no caminho MILP, que zeraria os duais.
- **λ_DS** é o dual de uma igualdade e tem o sinal certo. O CMO sem rede está certo.

---

## 9. Função de custo futuro (FCF)

### 9.0 O mínimo de modelagem para ler esta seção

A FCF diz quanto custa, **depois** do horizonte, terminar o dia com cada combinação de volumes nos reservatórios. Ela vem de um modelo de prazo mais longo (no DESSEM real, o DECOMP; aqui, um arquivo `OPER_FCF.bin` no formato FlatBuffers do projeto **cobre**). A FCF é um conjunto de **cortes de Benders**, e uma variável α (custo futuro) entra na FO com coeficiente 1:

```
α ≥ π0_k + Σ_i π_{k,i} · V_{i,T}      para cada corte k
```

| Símbolo | Significado |
|---|---|
| π0_k | intercepto do corte k |
| π_{k,i} | "valor da água" da usina i no corte k |
| V_{i,T} | volume final da usina i |

Na convenção do DESSEM (MM eq. 19.2) e do cobre, π é **tipicamente negativo**: mais água guardada, menor custo futuro.

### 9.1 O que a revisão concluiu sobre a FCF

O revisor do núcleo decodificou o `OPER_FCF.bin` do exemplo com uma ferramenta própria e rodou experimentos com cortes alterados:

| Aspecto | Veredito |
|---|---|
| **Sinal** | **Correto.** É o mesmo do cobre e do DESSEM. Com π < 0 o reservatório enche, com π > 0 esvazia, e o PiBalHidr no fim do horizonte reproduz π. Os coeficientes **positivos** do exemplo são uma propriedade do **dado**, não do código. Isso derruba a parte de "sinal invertido" do MX-14. |
| **Referencial do volume** | **Divergente** → NU-R02 (abaixo) |
| **Unidade e escala** | **Não especificadas**; o exemplo está provavelmente 1000× pequeno → NU-R01 (abaixo) |
| **Associação corte ↔ usina** | Por **posição** no `CAD_UHE`, sem conferir códigos → NU-R03 |
| **Efeito no exemplo** | **Nenhum sobre as decisões.** Os volumes finais são idênticos com e sem FCF, e o objetivo difere exatamente por α = 29.825,81. |

O arquivo do exemplo traz 20 cortes **idênticos**: intercepto 29.714,79 e π = [1/6, 1/9, 1/18, 0] para CAMARGOS, ITUTINGA, FUNIL e FURNAS.

**As linhas da FCF são `≥`, mas não são afetadas pelo RD-01**, porque nenhum código lê o dual delas.

---

### NU-R02 — Os cortes são aplicados sobre (V − Vmin), mas o formato e a documentação usam V absoluto

> **Severidade: ALTA** · **Veredito: NOVO, encontrado pelo revisor do núcleo e confirmado numericamente** · **Configs: `FCF=1`. Inócuo com cortes idênticos (como no exemplo); muda decisões com uma FCF real.**

**Em uma frase.** O código subtrai Σπ·Vmin do lado direito de cada corte, o que equivale a usar o volume **útil** (V − Vmin). O cobre, que produz o arquivo, e a documentação do próprio projeto usam o volume **absoluto**.

**Código.** [limits.rs:42](../../crates/labdessem-model/src/constraints/limits.rs#L42) faz `rhs -= coefficient * min_volume_hm3`. Resultado: α ≥ π0 + Σπ·(V − Vmin).

**Referências que dizem V absoluto:**

- `docs/design/future-cost-function.md:16` e `:75-77`: `future_cost ≥ intercept + Σ coef·final_volume`;
- o cobre: o estado x é o armazenamento absoluto, com lb = `min_storage_hm3`;
- DESSEM MM eq. 19.2: V_{i,T} absoluto.

**Prova numérica.** O α do caso base, obtido por diferença de objetivo com decisões idênticas, é 29.825,8063:

| Hipótese | α calculado |
|---|---|
| V − Vmin: 29.714,7912 + Σπ(V − Vmin) | **29.825,8063** ← bate exatamente |
| V absoluto | 29.863,9174 (Δ = Σπ·Vmin = 38,11) |

**Por que importa, embora o exemplo não mostre.** Com cortes **idênticos**, todos se deslocam igualmente e só o valor de α muda (38 R$). Numa FCF real, os cortes têm π diferentes, então cada corte se desloca por um valor diferente (−Σ_i π_{k,i}·Vmin_i). Isso muda **qual corte fica ativo**, e com ele o valor da água e as decisões. Ordem de grandeza: uma diferença de 100 R$/hm³ no π da FURNAS entre dois cortes, vezes Vmin = 5733 hm³, desloca um corte em relação ao outro em cerca de 573 mil R$.

**Contra-argumento considerado.** Se o contrato fosse "volume útil", no estilo DECOMP, a subtração estaria certa. Mas o formato é o do cobre, e a própria documentação do projeto diz V absoluto. O port em Julia (`labdessem_julia`) replica o código Rust, o que sugere herança, não decisão.

**Decisão sua.** Qual é o contrato: volume absoluto (cobre, documentação) ou volume útil? Depois de decidido, um dos dois (código ou documentação) precisa mudar.

**Teste de prova: CONFIRMADO.** Com um corte α ≥ 1·V, Vmin = 100 e V_T = 150, o modelo implica α = **50**; o certo seria 150. Com dois cortes de inclinações diferentes, o deslocamento **troca o corte ativo**. Com Vmin = 0 (controle), as duas convenções coincidem.

Na execução do exemplo, FCF=1 menos FCF=0 dá 29.825,81, que é o α calculado com (V − Vmin). Seção 12.

---

### NU-R01 — A unidade dos cortes não está definida, e o exemplo está provavelmente 1000× pequeno

> **Severidade: ALTA, condicional ao contrato** · **Veredito: NOVO** · **Configs: `FCF=1`**

**Em uma frase.** Nem a documentação nem o leitor dizem em que unidade estão o intercepto e os π. O arquivo do exemplo foi gerado pelo cobre numa época em que ele gravava os cortes em **milhares** de R$ (`COST_SCALE_FACTOR = 1000`). Lido como R$, o custo futuro fica irrisório e a FCF não muda nada.

**Evidência.**

- O arquivo foi commitado em 2026-05-05. Nessa data, o cobre gravava os cortes no espaço escalado (`COST_SCALE_FACTOR = 1000`, no commit `e8081a68` do cobre).
- **Plausibilidade:**
  - intercepto × 1000 = 29,7 milhões de R$, cerca de 6,65 dias do custo presente diário (4,47 milhões de R$);
  - sem o fator, o custo futuro seria 0,7 % de **um** dia.
- O DECOMP também usa milhares de R$ nos cortes.
- **Com o fator 1000 aplicado**, a FCF passa a mandar: CAMARGOS vai de 786 a 120 hm³ (esvazia, porque π > 0) e FURNAS de 5761 a 6438 hm³.

**Decisão sua.**

- Definir a unidade do arquivo e documentá-la.
- Confirmar com o produtor (cobre) o fator dos coeficientes, porque o cobre daquela época também escalava colunas.
- Considerar regenerar o `OPER_FCF.bin` do exemplo com cortes não idênticos e π negativos, para que a FCF seja exercitada.

---

### 9.x Outros achados da FCF (BAIXA)

| ID | O que é |
|---|---|
| **NU-R03** | Coeficientes associados à usina pela **posição** no `CAD_UHE`. O cobre ordena por id, e no exemplo as ordens coincidem. `stage_id`, `state_dimension` e versão do arquivo não são conferidos: um arquivo do cobre ≥ 0.14 (layout `AffinePiece`) seria lido com os campos errados. A leitura de campos FlatBuffers não confere o tamanho da vtable. **Se todos os cortes estiverem inativos, α = 0 sem aviso.** Vira MÉDIA se a ordem do CAD diferir da ordem de estados da política. |
| **NU-R04** | `future-cost-function.md:98` diz que o formato é "compatível com DECOMP/DESSEM", o que é falso: é FlatBuffers do cobre. Não há fator de desconto, e α ≥ 0 é fixo. |
| **NU-12** | O valor de α não é escrito em nenhuma saída. `resultado_fcf_eco.csv` só ecoa os cortes de entrada. Hoje α só se obtém por diferença de objetivo. |
| **MX-14** | A parte "coeficientes com sinal economicamente invertido" é propriedade do **dado** do exemplo; o código está certo (ver 9.1). A parte "doc diverge do código" é o NU-R02. |

---

## 10. Núcleo: conversão, solver, configuração e orquestração

### NU-11 (= MX-11) — Erro de digitação no `study_config.json` muda o modelo sem aviso

> **Severidade: MÉDIA** (revisor elevou de BAIXA) · **Veredito: CONFIRMADO** · **Configs: qualquer**

**Em uma frase.** Chaves desconhecidas são ignoradas, FCF e EngMax ausentes valem 0, e flags com valor 2 ou 7 valem como 1. `"fcf": 1`, com f minúsculo, **desliga** a FCF em silêncio.

**Matriz de configurações de borda (seção 3 da matriz):**

| Valor testado | Resultado | Rejeitado? |
|---|---|---|
| `rede: 2`, `UCT: 2`, `UCH: 2`, `TON_Residual: 2`, `FCF: 2`, `EngMax: 2` | igual a 1 | **não** |
| `rede: 7` | igual a 1 | **não** |
| sem a chave `FCF` | FCF desligada (Δ objetivo 29.825,81) | **não** |
| sem a chave `EngMax` | EngMax desligado | **não** |
| sem a chave `TON_Residual` ou `UCH` | erro `missing field` | sim (assimetria: estas são obrigatórias, FCF e EngMax não) |
| `"fcf": 1` (minúsculo) | FCF desligada | **não** |
| chave desconhecida qualquer | ignorada | **não** |
| `FPHA: 0` ou `FPHA: 1` | erro obscuro `expected value at line 7 column 11` | sim, mas o comentário de `study.rs:99` ainda documenta esses valores |
| `opcao_execucao: 0` ou `3` | erro claro | sim |

**Código.**

- [study.rs:85-111](../../crates/labdessem-io/src/study.rs#L85-L111): campos `u8` sem faixa, `#[serde(default)]` em FCF e EngMax, sem `deny_unknown_fields`.
- [study.rs:141-147](../../crates/labdessem-io/src/study.rs#L141-L147) converte com `!= 0`.
- O JSON schema (`schemas/study_config.schema.json`) também não pega: não tem `additionalProperties: false` e as flags inteiras só têm `minimum: 0`.
- O banner "Flags:" impresso na execução omite a FCF, então nem o log mostra que ela está desligada.

**Detalhe.** A chave `$schema` do exemplo depende de chaves desconhecidas serem aceitas. A correção precisa permitir essa chave explicitamente.

**Teste de prova: CONFIRMADO.** `"rede": 2` é aceito. A chave `"fcf"`, em minúsculas, é ignorada: a FCF fica desligada e a execução termina "ok", com objetivo 1.949.500,79 em vez de 1.979.326,35. Seção 12.

---

### NU-07 (= TH-R01 = HI-08) — Nomes duplicados fundem variáveis em silêncio

> **Severidade: MÉDIA** · **Veredito: CONFIRMADO** · **Configs: qualquer, com nome de usina ou unidade repetido no CAD**

**Em uma frase.** A conversão monta um mapa `nome → coluna` ([conversion.rs:22-26](../../crates/labdessem-solver/src/conversion.rs#L22-L26)). Se dois objetos geram o mesmo nome, o mapa fica com a última coluna. As restrições dos dois passam a apontar para a mesma variável, e a primeira coluna fica órfã.

**Evidência.** Três reproduções independentes, todas com exit 0:

| Caso | Objetivo | Observação |
|---|---|---|
| Termica-3 renomeada para Termica-2 | 5.761.318 (contra 4.467.814) | duas unidades com 90 MW e CVUs diferentes |
| Linha da Termica-3 duplicada no CAD | **137.835.458** (contra 4.497.640) | violação de rede espúria de 133,76 MW |
| ITUTINGA renomeada para CAMARGOS | 4.497.511 (contra 4.497.640) | o reservatório real da CAMARGOS some |

**Mesma família:**

- **RD-R02**: termo com nome inexistente é descartado em silêncio.
- **NU-R05**: folgas de limite operativo com nomes iguais compartilham uma coluna.
- **TH-01**: atribuição em vez de soma na FO.

**Correção sugerida (não implementada).** Validar unicidade de nomes no `System`, e na conversão detectar nome duplicado e nome inexistente (erro ou `debug_assert`).

---

### NU-09 — O status do solver nunca é conferido e as opções do HiGHS não são configuráveis

> **Severidade: BAIXA** · **Veredito: CONFIRMADO**

**O que acontece.**

- O good_lp 1.15.0 devolve `Ok` também quando o HiGHS para por limite de tempo, de iterações ou de gap.
- [highs.rs:193-231](../../crates/labdessem-solver/src/backends/highs.rs#L193-L231) aceita qualquer `Ok`, e o status gravado é sempre "OPTIMAL" ou "OPTIMAL_WITH_SLACK".
- `apply_profile` ([highs.rs:255](../../crates/labdessem-solver/src/backends/highs.rs#L255)) não faz nada.
- O gap MIP fica no padrão implícito de 1e-4. As diferenças de até 431 R$ entre configurações na matriz são desse gap.

**Por que BAIXA.** Sem limite de tempo configurado, o HiGHS não para por tempo. O risco é a falta de controle, não um resultado errado hoje.

---

### NU-10 — Os offsets de linhas da indexação divergem do modelo real (latente)

> **Severidade: BAIXA, latente** · **Veredito: CONFIRMADO por cinco agentes diferentes** (TH-10, HI-09, FP-10, RD-13 e MX-15 são partes deste)

**Em uma frase.** A indexação ([indexing.rs:265-367](../../crates/labdessem-model/src/indexing.rs#L265-L367)) **calcula** em que linha começa cada bloco de restrições. Depois do bloco `hydro_balance`, esses cálculos não batem com as linhas que o modelo **realmente** cria.

**Divergências encontradas:**

| Bloco | Indexação conta | Modelo cria |
|---|---|---|
| produtividade | 1 por unidade e período | 1 por **usina** e período |
| canalização UCH | 3 por unidade e período | 2 |
| acoplamento FPHA | todas as unidades | pula unidades com ρmax ≤ 0 |
| limite de intercâmbio | todos os pares | só pares declarados |
| limite operativo | 1 por limite | 1 por limite **e período** |
| turbinamento | bloco de mínimos + bloco de máximos | linhas intercaladas |
| UC térmico | colunas reservadas no LP | não existem no LP; `thermal_min_up` deslocado pelas linhas de condição inicial |

**Por que é latente.** O código de produção só lê dois offsets, `demand_balance` (para o CMO) e `hydro_balance` (para o valor da água). Esses são os **dois primeiros** blocos, e os dois estão corretos, como confirmaram três revisores e os checks M01 e H17 da matriz. Os cortes de fluxo são localizados pelo fim da matriz (`dual.len() − 2·cortes`), não por offset.

**Quando vira bug.** No primeiro dia em que alguém ler o dual da FPHA, do EngMax ou da FCF pelo offset.

**Correção sugerida (não implementada).** Derivar os offsets do modelo construído, contando por nome ou registrando o início de cada bloco na construção, em vez de recalculá-los à parte.

---

### MX-10 — Modo 1 com rede congestionada: o MILP decide o comprometimento sem ver a rede e nunca é corrigido

> **Severidade: MÉDIA** (limitação metodológica) · **Veredito: números CONFIRMADOS; a causa apontada pela matriz foi CORRIGIDA pelo revisor do núcleo** · **Configs: `opcao=1`, `rede=1`, `UCT=1`, rede apertada**

**O que a matriz observou.** Com todas as linhas a 250 MW:

| Modo | Folga de rede no resultado final | Objetivo |
|---|---|---|
| opção 1 | **5597,5 MW** | 5,60e9 |
| opção 2 | 1935,7 MW | 1,94e9 |

São **2,9× mais violação** no modo 1 para o mesmo problema. Com 175 MW: 10.184,5 MW contra 6527,2 MW.

**A causa.** A matriz atribuiu o problema a "pular o laço LP-FIXED" ([orchestration.rs:121](../../crates/labdessem-simulation/src/stages/orchestration.rs#L121)). O revisor do núcleo mostrou que isso **não** é a causa:

- O LP-CALC-CMO final é **a mesma chamada** nos dois ramos (`all_cuts`, comprometimento do MILP). Com EngMax=0, rodar o laço produziria exatamente o mesmo LP final.
- A causa real é que, no modo 1, o MILP só vê os cortes descobertos pelo laço LP inicial. Se o comprometimento que ele escolhe viola outras linhas, **o MILP nunca é resolvido de novo** com esses cortes. O laço LP-FIXED só ajusta o despacho com o comprometimento já fixo. Isso acontece também no ramo sem folga.
- "Nunca voltar ao MILP" é **escolha de projeto documentada** (`iterative-workflow.md`, "Post-MILP Bifurcation") e tem terminação garantida. O laço tem no máximo |linhas| × T voltas.

**Consequência.** Em rede congestionada, o modo 1 pode entregar resultado muito pior que o modo 2.

**Decisão sua.** Aceitar como limitação documentada (e talvez recomendar o modo 2 com rede apertada), ou voltar ao MILP com os cortes violados até a rede ficar viável.

---

### 10.x Outros achados do núcleo e da orquestração (BAIXA, salvo indicação)

| ID | O que é |
|---|---|
| **NU-08** (= RD-14) | Backend: uma linha sem limites é pulada com `continue`, deslocando os duais seguintes; uma linha de faixa perde o limite inferior; uma variável binária ignora lb/ub. **Latente:** a conversão nunca gera esses casos hoje. |
| **NU-13** | A relaxação com folgas também é disparada quando o problema é `Unbounded`, onde folga não resolve. |
| **NU-14** | A documentação diz que `unit_interval` é contínua, mas o domínio é `Binary`. Só a documentação está errada; partida e parada binárias são a escolha segura. |
| **NU-R05** | Folgas de limite operativo com o mesmo nome (mesma usina, variável e período em dois limites) se fundem numa coluna. A violação fica subpenalizada. |
| **NU-R06** (= parte do MX-10) | No modo 1, o `Violations` do resumo é o do MILP (33), não o do LP-CALC-CMO que gera as saídas (38). |
| **MX-06** (MÉDIA) | `FPHA="precomputed"` é inutilizável com o exemplo, porque `OPER_FPHA.csv` só tem cabeçalho. Um terço da matriz (128 execuções) aborta em `missing FPHA data for hydro plant 0`, e a mensagem não cita o arquivo. **Lacuna de cobertura:** o modo `precomputed` nunca foi exercitado de ponta a ponta com dados. |
| **MX-12** | Com `FPHA=computed` e `EngMax=1`, o EngMax é recalculado entre etapas. O LP do modo 1 fica 718,5 R$ **acima** do MILP, e o resultado final difere 6 R$ entre os modos 1 e 2. É intencional (ADR-017, regra R1), mas torna os objetivos das etapas incomparáveis e deve ser documentado. O revisor do núcleo confirmou: "objetivo do MILP = objetivo do LP-CALC-CMO" só vale com `EngMax=0`. |
| **MX-16** | `TON_Residual` não tem efeito no exemplo: o CMO residual (300) é igual ao CVU da única térmica ligada no fim. Com CMO residual = 0, a diferença é exatamente +12.000 R$, ou seja, a lógica funciona e o exemplo é que não a exercita. Lacuna de cobertura. |
| **MX-17** | O log imprime `MILP #01 inviavel com restricoes de rede/limite. Adicionando folgas...` antes do erro de EngMax, sugerindo que a rede era o problema, quando não era. |

---

## 11. A matriz de configurações (384 execuções)

### 11.1 O que foi feito

Cada uma das 384 combinações de flags foi executada com o binário de release, sobre uma **cópia** do caso `examples/24Barras`. O caso tem:

- 48 períodos de 0,5 h;
- 2 submercados;
- 3 térmicas;
- 4 hidrelétricas em cascata com 17 unidades;
- 24 barras e 34 linhas.

Um verificador independente, escrito em Python sem usar código do projeto, conferiu cada saída contra **40 identidades** que precisam valer se o modelo e a escrita estiverem certos. Exemplos:

- o balanço de energia fecha;
- o fluxo impresso é o que a PTDF calcula com a geração impressa;
- o objetivo impresso é a soma dos custos recompostos a partir do despacho impresso;
- térmica marginal fora da swing ⇒ CMB da barra = CVU.

### 11.2 Resultado geral

| | Execuções |
|---|---|
| Total | 384 |
| Terminaram com sucesso | **224** |
| Abortaram | **160** |

**As 160 que abortaram:**

| Grupo | Quantas | Configuração | Motivo | Achado |
|---|---|---|---|---|
| A | 128 | **todas** com `FPHA="precomputed"` | `missing FPHA data for hydro plant 0`: o `OPER_FPHA.csv` do exemplo só tem cabeçalho | MX-06 |
| B | 32 | todas com `FPHA="computed"` + `UCT=1` + `EngMax=1` | MILP inviável: faltam 67 MW no período 1, e o modelo não tem déficit | EM-01 (refutado como bug do EngMax), RD-05/MX-05 |

### 11.3 O que **fecha** nas 224 execuções que terminaram

| Identidade verificada | Resultado |
|---|---|
| Balanço de energia por período (B01) | fecha, erro ≤ 5e-7 MW |
| Intercâmbio implícito dentro dos limites (B02) | fecha |
| Balanço hídrico (H01) | fecha, erro ≤ 1e-6 hm³ |
| Água de montante, conversão m³/s ↔ hm³, limites de volume, turbinamento e geração (H02–H08, H14–H16) | fecham |
| Envelope FPHA e acoplamento por unidade com FPHA (H10, H10b, H10c) | fecham |
| Σq ≤ EngMax (H11) | fecha |
| Trajetórias térmicas, Pmin/Pmax, tempos mínimos e condição inicial (T02) | fecham |
| α da FCF recalculado com (V − Vmin) (F01) | fecha |
| **Objetivo recomposto a partir do despacho impresso** (O01) | **fecha, erro ≤ 2,2e-4 R$**, inclusive com TON e com folga de rede |
| **Fluxos recalculados com PTDF própria** (N01) | **fecham, erro ≤ 5e-5 MW** |
| CMO = PiDemanda sem rede (M01) | fecha |
| Monotonicidade (seção 11.5) | nenhuma violação |

**Leitura.** O despacho impresso é coerente com o objetivo impresso, e os fluxos impressos são coerentes com o despacho. **A cadeia "modelo resolvido → CSV" está íntegra para as grandezas primais** (geração, volumes, fluxos, custo). Os problemas estão:

- na **entrada**: HI-01, RD-04, NU-11;
- em **detalhes da formulação**: TH-01, TH-03, FP-02, HI-03, NU-R02;
- nos **duais e preços**: RD-01, RD-02;
- em **colunas secundárias de saída**: StatusOn, rótulos de período.

### 11.4 O que **não fecha**, por padrão de flags

| Check | Falha em | Achado |
|---|---|---|
| R01, M04, M05, M06 (cmobar: contagem, escala, período) | 112/112 com `rede=1` | RD-02, RD-03, RD-06 |
| M03 (CMB da térmica marginal = CVU) | 16 execuções `rede=1 & computed & EngMax=1` (sem congestionamento: só a escala); e **todos** os períodos congestionados dos experimentos extras | RD-02; RD-01 |
| I01, B03, N04 (demanda SBM × cargas de barra) | todas | RD-04 |
| H04 (afluência impressa = `OPER_VAZAO` casado pelo **nome**) | 224/224 | HI-01 |
| H09b (g = ρ·q por unidade) | 128/128 com produtividade constante | HI-04 |
| H12 (StatusOn hidráulico coerente) | 112/112 com `UCH=0` | HI-02 |
| T05 (StatusOn térmico = comprometimento) | 96/96 com `UCT=1` | TH-02 |
| O02 (LP ≤ MILP; LP-CALC-CMO = MILP) | 32 com `computed & EngMax=1` | MX-12 (efeito documentado do EngMax dinâmico) |

### 11.5 Monotonicidade: o modelo reage na direção certa a cada flag

Foram comparados pares de execuções que diferem em **uma** flag, com tolerância igual ao gap MIP (1e-4):

| Comparação | Pares | Violações | Observação |
|---|---|---|---|
| rede=0 ≤ rede=1 | 112 | 0 | |
| UCT=0 ≤ UCT=1 | 96 | 0 | +2,5 milhões: UCT=0 relaxa Pmin e trajetórias |
| UCH=0 ≤ UCH=1 | 112 | 0 | |
| TON=0 ≤ TON=1 (UCT=1) | 48 | 0 | **sem efeito no exemplo** (MX-16) |
| TON=0 = TON=1 (UCT=0) | 64 | 0 | idênticos, como esperado (sem UC não há TON) |
| FCF=0 ≤ FCF=1 | 112 | 0 | diferença = α = 29.825,56 |
| EngMax=0 ≤ EngMax=1 | 96 | 0 | só morde com FPHA computed (+547 mil) |
| opção 1 = opção 2 | 112 | 0 | diferenças ≤ gap; 6 R$ com EngMax (MX-12) |

### 11.6 Experimentos extras (caminhos que o exemplo não exercita)

| Experimento | Modificação | O que revelou |
|---|---|---|
| `cong250`, `cong175` | todas as linhas com 250 ou 175 MW | **RD-01** (CMB invertido, até 577 R$/MWh de desvio); RD-10 (Violacao sem tolerância); MX-10 (modo 1 com 2,9× mais folga) |
| `tonres_cmo0` | CMO residual = 0 | a lógica do TON residual funciona (+12.000 R$ exatos) |
| `vazao_fix` | códigos do `OPER_VAZAO` corrigidos | quantifica o HI-01; a inviabilidade de `computed + UCT + EngMax` persiste |

### 11.7 Limitações da matriz

- **`precomputed` não foi exercitado** (sem dados no exemplo).
- **Todos os períodos têm 0,5 h**, então os achados que só aparecem com durações variáveis (RD-03, RD-08, TH-12) foram vistos pelos revisores em casos próprios, não pela matriz.
- **Não há renováveis nem bombeamento** no exemplo: `CAD_REN`, `CAD_USIE` e `OPER_REST_LIM` têm só o cabeçalho. As restrições de renováveis e de elevatórias foram auditadas **só por leitura de código**, sem nenhuma execução. Os limites operativos só foram exercitados nos casos montados pelos revisores e testadores.
- **O comprometimento térmico não é impresso** (TH-02), então o verificador reconstruiu o status a partir das trajetórias.

---

## 12. Testes de prova (fase 4)

### 12.1 O que é um "teste de prova" e como ler o resultado

Ler o código e rodar a matriz mostram que um defeito **provavelmente** existe. O teste de prova serve para **demonstrar** isso com um programa que qualquer pessoa pode rodar. Cada achado relevante ganhou dois tipos de teste:

| Tipo | Nome | O que afirma | Resultado esperado **hoje** | Resultado esperado **depois da correção** |
|---|---|---|---|---|
| Teste do defeito | `audit_<id>_...` | o comportamento **correto** | **falha** (o defeito existe) | passa |
| Controle | `audit_<id>_control_...` | a mesma verificação, numa situação em que o defeito **não pode** aparecer | **passa** | passa |
| Informativo | `audit_<id>_info_...` ou "FATO" | documenta um fato do código atual | passa | pode mudar |
| Sonda | `probe_...` (`#[ignore]`) | não afirma nada, só **imprime números** para este relatório | passa | passa |

**Por que o controle importa.** Imagine que o teste do TH-01 falhasse porque o caso de exemplo não carrega, e não por causa do bug. O controle usa o mesmo caso, a mesma montagem e a mesma leitura, e só retira a condição do bug. Se o controle passa e o teste do defeito falha, a falha vem do defeito e não de um erro do próprio teste.

**Como os testes foram rodados.**

- Os testes estão em clones isolados do commit `3d72ec6`, em `~/.cache/labdessem-audit/`.
- Nenhum arquivo de produção foi alterado. A única linha fora de arquivos de teste é um `mod audit_tests;` em `stages/mod.rs`, para um teste que precisa de funções privadas do crate.
- Os testes de ponta a ponta (E2E) rodam o **binário de release do commit auditado** (`target/release/labdessem`) sobre cópias do `examples/24Barras` com uma modificação cada.
- Rodada final: 05/11/2026, das 19h48 às 19h51. As saídas completas estão em `~/.cache/labdessem-audit/test_logs/`.

**Um registro honesto do processo.** Os quatro agentes testadores escreveram os testes, mas a máquina reiniciou antes de eles registrarem os resultados. Rodei novamente todas as suítes, **sem alterar o conteúdo de nenhum teste**. Só foram necessários dois ajustes mecânicos:

1. um arquivo de teste do núcleo tinha ficado com nome temporário, porque a gravação foi interrompida, e foi renomeado;
2. um binário de teste corrompido pelo reinício (`Exec format error`) foi recompilado.

### 12.2 Placar geral

| Suíte | Arquivos | Testes | Defeito: **falham** como esperado | Controles: **passam** | Informativos e sondas | Fora do esperado |
|---|---|---|---|---|---|---|
| Térmicas | `labdessem-io`, `labdessem-solver`, `labdessem-uc` → `tests/audit_termicas.rs` | 48 | 26 | 20 | 2 | — |
| Hidro e FPHA | `labdessem-io/tests/audit_hidro.rs`, `labdessem-cli/tests/audit_hidro_model.rs` | 38 | 21 | 16 | 1 | — |
| Rede | `labdessem-solver/tests/audit_rede.rs`, `labdessem-cli/tests/audit_rede.rs` | 70 | 32 | 30 | 6 sondas | 2 (NU-13, ver 12.5) |
| Núcleo e FCF | `labdessem-io` e `labdessem-model` → `tests/audit_nucleo.rs`, `labdessem-simulation/src/stages/audit_tests.rs` | 17 | 8 | 9 | — | 1 (NU-R03 vtable, ver 12.5) |
| **Total** | 10 arquivos | **173** | **87** | **75** | **9** | **3** |

**Leitura.**

- **Nenhum controle falhou.** Os 87 testes de defeito falham porque o defeito existe, não porque o teste está quebrado.
- **Um** dos 87 falha pelo motivo errado: o NU-R03 vtable, explicado em 12.5. Sobram **86 demonstrações válidas**.
- **Dois** testes de defeito **passam**, os do NU-13. Eles só verificam um pré-requisito do defeito, e o defeito em si continua sem demonstração (12.5).

### 12.3 Resultado por achado

Em cada célula, entre aspas, está o **trecho essencial da mensagem de falha** que o teste imprime hoje.

#### Térmicas

| Achado | O que o teste do defeito mostra hoje | Controle (passa) | Veredito |
|---|---|---|---|
| **TH-01** (ALTA) | `audit_th01_startup_cost_survives_ton_residual`: o coeficiente da coluna `thermal_startup[Termica-3,t=1]` sai **"−0" em vez de 1.000.000**, e as **48 de 48** colunas de partida perdem o custo. `audit_th01_objective_sums_repeated_terms`: uma variável com termos 3 e 4 na FO recebe **4, não 7**. `audit_th01_milp_optimum_with_ton_residual_is_not_below_without`: o ótimo com TON=1 (4.497.639,84) fica **abaixo** do ótimo com TON=0 (5.497.639,84), o que é impossível, porque o TON só **acrescenta** custos não negativos. | termo único preservado; partida sem TON preservada; termos repetidos em **restrições** são somados (o bug é só na FO) | **CONFIRMADO** |
| **TH-R01** | `audit_thr01_duplicate_unit_must_not_merge_columns`: com uma unidade duplicada, **192 colunas** compartilham nome. A primeira de cada par vira órfã: custo 0 e nenhuma linha. Na outra, a geração entra no balanço com **coeficiente 2**. | unidades distintas têm colunas distintas | **CONFIRMADO** |
| **TH-03** (ALTA) | `audit_th03_initial_rampdown_completion_must_be_feasible` (crate `uc`): a trajetória **fisicamente correta**, que termina a rampa e fica em 0, é **rejeitada** pela linha `thermal_ramp_upper[t=1]`, que vira "0 ≤ −672". O modelo **proíbe** desligar. `audit_th03_solve_initial_rampdown_unit_is_off_after_remaining_step`: resolvendo, o período 2 sai com **g = 255 MW**. `audit_th03_subcase_finished_rampdown_cannot_jump_to_pmin`: no subcaso TRampaInic = duração total da rampa, a unidade salta de 0 para **523 MW** no período 1 sem partida. | unidade inicialmente desligada fica desligada; desligamento dentro do horizonte é viável; a partida respeita a trajetória | **CONFIRMADO**, inclusive o subcaso |
| **TH-02** (MÉDIA) | O teste do lado do `output_prep` **não chegou a ser escrito** (o testador foi interrompido). `audit_th02_control_writer_honors_commitment_when_provided` **passa**: se `thermal_on` fosse entregue ao escritor, o `StatusOn` sairia certo. Isso confirma que a correção é no `output_prep.rs`, não no escritor. | — | **CONFIRMADO por execução**: caso base (`Termica-3;1;6`) e check T05 da matriz, falha em 96 de 96 execuções |
| TH-06 | "a unidade cai de **600 MW para 0** em meia hora, pulando D[0] = 255 MW" | unidade barata continua ligada | comportamento confirmado; segue BAIXA (discretização literal da eq. 10.10) |
| TH-07 | unidade 2 na barra 3/Fic2, numa usina cuja unidade 1 está na barra 7/Fic1: **aceita**, e a barra da unidade 2 é descartada | mesma barra e submercado aceitos | CONFIRMADO |
| TH-08 | `Ipasso` duplicado vira a trajetória **[255, 255, 0]**; `CodigoUTE=999` é aceito; linha de unidade inexistente é ignorada; **no exemplo**, as rampas da Termica-3 trazem `CodigoUTE=1`, que é o código da Termica-2 | exemplo limpo lê as trajetórias esperadas | CONFIRMADO |
| TH-09 | Ton = 82 h com Tinic = 81,75 h: sobram 0,25 h, mas **0 períodos** são fixados (deveria ser 1). O mesmo acontece com o Toff. | Tinic múltiplo de 0,5 h | CONFIRMADO |
| TH-10 | offsets de linha deslocados em 4; no modo LP, 432 colunas de UC reservadas e inexistentes | variantes sem as linhas de fixação inicial | CONFIRMADO, **latente** (ninguém consome; as colunas de UC são os últimos blocos) |
| TH-12 | com período original de 1 h, `TempoPermanenciaOn` sai **4 em vez de 2**; o agregado usa o tempo da unidade **desligada** (201 em vez de 3) | períodos de 0,5 h; usina de uma unidade | CONFIRMADO |
| TH-R02 | unidade numerada 99 colide com o agregado: **48 chaves duplicadas** | códigos regulares | CONFIRMADO |

#### Hidráulica, FPHA e EngMax

| Achado | O que o teste do defeito mostra hoje | Controle (passa) | Veredito |
|---|---|---|---|
| **HI-01** (CRÍTICA) | `audit_hi01_shipped_example_inflow_must_match_oper_vazao_by_plant_name`: no exemplo, CAMARGOS usa **0** (o arquivo diz 92), ITUTINGA usa **92** (o arquivo diz 0) e FUNIL usa **0** (o arquivo diz 175). Também são aceitos **em silêncio**: código 99 inexistente, usina sem nenhuma linha de afluência (fica com 0) e período faltando (fica com 0). | códigos consistentes são lidos certo; código desconhecido em `Jusante` **é** rejeitado (a validação existe em outro leitor) | **CONFIRMADO** |
| **HI-03** (ALTA) | `audit_hi03_model_must_link_generation_to_water_for_every_plant_with_units`: "FUNIL-GRANDE tem 3 unidades e **0 linhas** que ligam geração a água: geração livre até Pmax = 180 MW". O teste informativo mostra que a regra `HydroPlant::validate` **existe** e rejeitaria o caso, mas não é rodada depois da injeção da FPHA. | usina com planos tem as linhas de ligação | **CONFIRMADO** |
| **HI-02** (MÉDIA) | `audit_hi02_..._when_uch_disabled`: com UCH=0, `hydro_commitment(0,0)` devolve o valor de **outra coluna, 3784**, que é a primeira folga dinâmica | com UCH=1 lê a variável de status certa | **CONFIRMADO** |
| **HI-04** (MÉDIA) | "**17 unidades** sem relação g_u ↔ q_u própria" com produtividade constante | o modo FPHA tem acoplamento por unidade | **CONFIRMADO** |
| **HI-05** (MÉDIA, disputado) | "**1632 linhas** de canalização hidráulica (2 por unidade-período) e **nenhuma** tem termo de turbinamento" | a canalização da **geração** liga g ao status | **CONFIRMADO** o fato; a severidade depende da decisão D3 |
| **HI-07** (MÉDIA) | "limite superior do desvio = **None**" | usina sem desvio tem limite finito | **CONFIRMADO** |
| HI-06 | FURNAS com conjunto 1 na barra 16 e conjunto 2 na barra 3: aceita, e vale a barra 16 | mesma barra em todos os conjuntos | CONFIRMADO |
| HI-08 | duas usinas chamadas CAMARGOS são aceitas | nomes distintos | CONFIRMADO |
| HI-09 | o offset de `hydro_turbining_min` aponta para uma linha de `hydro_turbining_upper` | offsets consumidos (`demand_balance`, `hydro_balance`) estão certos | CONFIRMADO, latente |
| HI-R01 | com `rede=1`, a usina só com reservatório é rejeitada ("missing Barra") | aceita com `rede=0` | CONFIRMADO |
| HI-R02 | ciclo de cascata FUNIL ↔ FURNAS é aceito | cascata acíclica | CONFIRMADO |
| HI-R03 | código 9 no `CAD_CONJ_UHE` é descartado; FUNIL-GRANDE fica com **0 unidades** sem aviso | código válido mantém as unidades | CONFIRMADO |
| **FP-02** (ALTA) | sem teste Rust; prova por execução (12.4) | — | **CONFIRMADO por execução** |
| **FP-05** (MÉDIA) | sem teste Rust; prova por execução (12.4) | — | **CONFIRMADO por execução** |
| FP-07 | "**48 linhas** `hydro_fpha` para a usina sem unidades" | usinas com unidades recebem as linhas | CONFIRMADO |
| FP-10 | offset do 1º corte da FCF calculado em 3024; a posição real é 2400 (diferença = (17 unidades − 4 usinas) × 48) | — | CONFIRMADO, latente |
| FP-R01 | linha duplicada no `CAD_TAILRACE` (vale a última: a0 = 0) e no `CAD_FPHA_CONFIG` é aceita | sem duplicata | CONFIRMADO |
| EM-04 | `a0 = NaN` no `CAD_TAILRACE` e cota NaN no `CAD_GEOMETRIA_UHE` são aceitos | valores finitos | CONFIRMADO |

#### Rede, preços e solver

| Achado | O que o teste do defeito mostra hoje | Controle (passa) | Veredito |
|---|---|---|---|
| **RD-01** (CRÍTICA) | **No solver, isolado:** em `min x s.t. x ≥ 3`, o dual sai **−1**, mas a derivada do objetivo em relação ao lado direito é **+1** (diferença finita: 0,9999999999998899). **Ponta a ponta:** o mesmo sistema físico com uma linha declarada ao contrário muda a CMO em **96 de 96** pares (período, submercado), por exemplo período 10 Fic2: **674,17 contra 68,74**. Com a linha no limite inferior, o CMB impresso não bate com a diferença finita: barra 20, **277,40 impresso contra 94,05 real**. | dual de `≤` e de `=` corretos; o dual de `≥` é exatamente o de `−x ≤ −3` (o mecanismo); objetivo e fluxos **não** mudam com a orientação; com a linha declarada no outro sentido, CMB impresso = diferença finita **nas três barras** | **CONFIRMADO** |
| RD-R01 | "o requisito do `good_lp` é `1.15` (circunflexo): `cargo update` pode ir a 1.15.3 e **inverter o sinal** de todos os duais `≥`" | o `Cargo.lock` fixa 1.15.0 hoje; `^1.15` admite 1.15.3 | CONFIRMADO |
| **RD-02** (ALTA) | sem congestionamento, **1104 de 1104** linhas do cmobar têm **CMB = CMO/2** (150 contra 300) | CMB ÷ Δt = CMO, e CMO = PiDemanda | **CONFIRMADO** |
| **RD-04** (ALTA) | **96 de 96** pares (submercado, período) inconsistentes; t=1 Fic1: `OPER_SBM` = 1164,01 MW contra Σ`OPER_CARGA_BARRA` = 955,17 MW. A barra swing retira, **sem carga declarada**, exatamente essa diferença: 271,68 MW em t=1 e até **391,5 MW**. | retirada medida = diferença de entrada (266,04 = 266,04 MW); deck consistente → retirada de 0,0001 MW | **CONFIRMADO** |
| RD-03 | o cmobar rotula os períodos de **0 a 47**, contra 1 a 48 no cmosist; com um período de 2 h, de 0 a 50 (51 rótulos internos) | cmosist e térmicas usam os períodos originais | CONFIRMADO |
| RD-05 | demanda +50.000 MW: "LP is infeasible", **nenhum arquivo** de saída e nenhum diagnóstico. A sonda mostra o "precipício": +1.500 MW roda; +3.000 MW aborta. | demanda normal roda | CONFIRMADO |
| RD-06 | 23 registros de CMB para 24 barras (a swing some); cmobar com 1104 linhas, não 1152; a CMO impressa difere em até **4,87 R$/MWh** da média com a swing num caso congestionado | a barra que falta é exatamente a swing; CMO impressa = média sem a swing | CONFIRMADO |
| RD-07 | par de submercados omitido: objetivo **1,34 milhão**; declarado com 0 MW: **11,33 milhões**. Omitir **não** significa "sem interligação". | omitido = par com limite enorme; limite de 10 MW é respeitado | CONFIRMADO (a parte "ilimitado") |
| RD-08 | com um período de 2 h, o `resultado_rede` vai até 51 (períodos internos) e o `inviabilidade_lim` diz período **6** para uma restrição do período original **3** | `rest_lim` usa o período original; durações uniformes estão certas | CONFIRMADO |
| RD-09 | `rede=0` com submercados de código {2, 3}: aborta com "exactly one angle reference bus, found 0" | funciona se existir o código 1 | CONFIRMADO |
| RD-10 | **16 linhas** marcadas `Violacao=1` com \|fluxo\| = limite (−120,0000 contra 120) | o resumo da execução diz 0 violações | CONFIRMADO |
| RD-11 | o intercâmbio não está em nenhum dos 13 arquivos de saída | os outros arquivos existem | CONFIRMADO |
| RD-14 (= NU-08) | linha de faixa [2, 10] perde o limite inferior (x = 0); linha livre pulada **desalinha os duais** ([−1, 0] em vez de [0, −1]); coluna binária com limites [1, 1] resolve em 0 | linha só `≥` respeitada; sem linha livre os duais ficam alinhados; coluna contínua respeita os limites | CONFIRMADO, latente |
| RD-17 | barra 22 isolada → "reduced DC susceptance matrix is singular", sem dizer **qual** barra | rede conexa roda | CONFIRMADO |
| RD-R02 | termo com variável inexistente é **descartado em silêncio**, na restrição (2 termos declarados, 1 mantido) e na FO (custo 1e6 sem efeito) | termo com variável conhecida é mantido | CONFIRMADO |
| NU-09 | limite de iterações = 1 e limite de tempo de 1e-9 s pedidos via `HighsProfile` são **ignorados**: o `solve()` devolve `Ok` | o `good_lp` devolve `Ok` mesmo com status não ótimo (o mecanismo) | CONFIRMADO |
| **NU-11** (MÉDIA) | `"rede": 2` é aceito; a chave digitada errado `"fcf"` é **ignorada em silêncio** e a FCF fica desligada. Objetivo 1.979.326,35 com `"FCF"` e 1.949.500,79 com `"fcf"`, execução "ok". | config válida roda; tipo errado é rejeitado | **CONFIRMADO** |
| **MX-10** (MÉDIA) | com linhas de 250 MW e UCT=1, a folga de rede final é **5597,5 MW no modo 1** contra **1935,7 MW no modo 2** (2,89×). Com 175 MW: 10.184,5 contra 6.527,2 MW. | sem UC os dois modos coincidem | **CONFIRMADO** |
| NU-13 | **não demonstrado**, ver 12.5 | — | segue como achado de leitura |

#### Núcleo e FCF

| Achado | O que o teste do defeito mostra hoje | Controle (passa) | Veredito |
|---|---|---|---|
| **NU-R02** (ALTA) | `audit_nur02_fcf_cut_uses_absolute_volume`, com um corte α ≥ π·V, π = 1, Vmin = 100 e V_T = 150: o modelo gera lado direito −100 e α implícito **50**; o certo é α = **150**. `audit_nur02_heterogeneous_cuts_active_cut_and_envelope`: com dois cortes de inclinações diferentes, o deslocamento troca **qual corte fica ativo** (corte 2 em vez do corte 1) e a envoltória vai de 150 para **120**. | com Vmin = 0 as duas convenções coincidem; inclinação zero não é deslocada; cortes heterogêneos com Vmin = 0 | **CONFIRMADO** |
| NU-R03 | o coeficiente do estado 0 (usina id 1) vai para a **primeira usina listada** (id 2); FCF ligada com **todos os cortes inativos** passa no `validate()` (α = 0 em silêncio); cabeçalho com `state_dimension = 7` e 4 coeficientes por corte é aceito (24 cortes lidos) | usinas em ordem de id; um corte ativo; arquivo do exemplo lê de forma consistente; dimensão coerente aceita | CONFIRMADO (exceto a parte da vtable, 12.5) |
| NU-R05 | dois limites operativos na mesma usina, variável e período **dividem uma folga**: 1 nome de folga, colunas com [0, 2] não nulos. A violação reportada é **90 MW em vez de 170 MW**. | limites em períodos diferentes têm folgas próprias | CONFIRMADO |
| NU-R01 | sem teste: é uma questão de contrato e unidade. A execução confirma que a FCF do exemplo é **inerte** (12.4). | — | segue CONFIRMADO pela leitura |

### 12.4 Números das execuções de ponta a ponta

Todas as execuções usam o binário de release do commit auditado, sobre o `examples/24Barras` com **uma** modificação. A configuração do exemplo é opção 2, `rede=1`, `UCT=1` e produtividade constante, salvo indicação. O objetivo do caso base é **4.497.639,84 R$**.

**TH-01: a partida de 1 milhão some.** `CustoPartida` da Termica-3 = 1.000.000 R$. A Termica-3 parte no período 1 nas duas execuções (90 MW no p1, 180 MW no p2).

| Execução | Objetivo (R$) | Leitura |
|---|---|---|
| `TON_Residual=0` | 5.497.639,84 | base + 1.000.000: a partida foi paga |
| `TON_Residual=1` | **4.497.639,84** | igual à base: a partida saiu **de graça** |

**TH-03: geração forçada depois da rampa de desligamento.** Termica-1 com GerInic = 255 MW, StatusInic = 1, RdownInic = 1 e TRampaInic = 0,5 h (falta um passo da rampa D = [255, 0]).

| Execução | Termica-1 nos períodos 1, 2 e 3 | Objetivo (R$) |
|---|---|---|
| CVU normal (407,34) | 0, **255**, 0 MW | 4.549.562,57 |
| CVU = **100.000 R$/MWh** | 0, **255**, 0 MW | **17.247.626,72** (+12,75 milhões ≈ 255 MW × 0,5 h × 100.000) |
| Controle: CVU = 100.000, **sem** rampa inicial | 0, 0, 0 MW | 4.497.639,84 |
| Subcaso: TRampaInic = 1,0 h (rampa já terminada), CVU = 1 | **523** MW desde o p1, sem partida | 44.249,90 |

Mesmo a 100 mil R$/MWh, o otimizador gera 255 MW no período 2. Não é escolha: é a restrição de rampa repetida que obriga.

**HI-01: afluências trocadas no exemplo.** Afluência usada no período 1 (m³/s), lida do caso original e do caso com os códigos do `OPER_VAZAO` corrigidos:

| Usina | Arquivo (casado por nome) | Usada hoje | Usada com códigos corrigidos | Geração média hoje → corrigida (MW) |
|---|---|---|---|---|
| CAMARGOS | 92 | **0** | 92 | 0,0 → 23,0 |
| ITUTINGA | 0 | **92** | 0 | 52,0 → 23,0 |
| FUNIL-GRANDE | 175 | **0** | 175 | 91,4 → 180,0 |
| FURNAS | 523 | **0** | 523 | 937,9 → 855,3 |

Só entram 92 dos 790 m³/s (**88 % perdidos**). Neste exemplo, o objetivo quase não muda (4.497.639,84 → 4.497.640,39), mas o **despacho hidráulico por usina muda completamente**: FUNIL dobra, CAMARGOS passa a gerar e FURNAS termina o dia com 60 hm³ a mais. O pequeno efeito no custo é uma característica deste exemplo; não serve de garantia para outros decks. Removendo as linhas da FURNAS e incluindo um código inexistente, a execução também termina com sucesso, sem aviso.

**HI-03: geração sem água.** FPHA `computed`, removendo a geometria de uma usina:

| Execução | Resultado |
|---|---|
| geometria completa (controle) | nenhum período com G > 1 MW e Q = 0 |
| sem geometria da FUNIL-GRANDE | FUNIL gera **180 MW com Q = 0** em 48 de 48 períodos; a execução termina com sucesso |
| sem geometria da FURNAS | FURNAS gera **1216 MW com Q = 0** em 48 de 48 períodos |
| sem nenhuma geometria | as 4 usinas geram com Q = 0; só aparece um *warning* |

**HI-02: StatusOn hidráulico.** Com UCH=0, as 608 unidades-período que geram saem **todas** com `StatusOn = 0`. Com uma folga ativa, aparecem `StatusOn = 3784` e `StatusOff = −3783` (48 linhas fora de [0, 1]). Com UCH=1 (controle), nenhum caso.

**HI-04: unidade gera mais que sua turbina permite.** ITUTINGA com unidades heterogêneas, na qual a unidade 2-1 tem MaxTurb = 5 m³/s e ρ = 1, ou seja, no máximo 5 MW. Com produtividade constante, ela gera até **14 MW**, com Q = 0 nesses períodos, e a usina passa do máximo físico (43 MW) em 18 de 48 períodos. Em 257 de 816 unidades-período, G_u > ρ·Q_u. Com FPHA `computed` (controle), nenhum caso.

**HI-05: turbinamento em unidade desligada (UCH=1).** Com produtividade constante, há 96 linhas de unidade **desligada com Q > 0** e 85 usina-períodos em que a geração só fecha contando a água das unidades desligadas. Exemplo: CAMARGOS, t=2, G = 14,8 MW, com toda a água na unidade desligada. Com FPHA `precomputed` há 107 e 69 casos, respectivamente.

**HI-07: desvio sem limite.** Com um desvio declarado em CAMARGOS, o otimizador desvia **373.424,7 m³/s** em um período (o engolimento total é 214 m³/s) e o volume cai de **792 para 120 hm³** em meia hora, no último período. O incentivo para isso vem dos cortes do exemplo, que têm π > 0: mais volume no fim significa mais custo futuro, como explicado na seção 9. Mas a **possibilidade** de jogar fora 672 hm³ numa meia hora vem da falta de limite no desvio. Sem FCF, o desvio máximo é 738 m³/s, ainda acima do engolimento.

**FP-02: α aplicado duas vezes.** Em cada usina, a maior razão entre a geração e o plano FPHA impresso:

| Usina | α | max(G ÷ plano impresso), `computed` | Mesma razão, `precomputed` com os mesmos planos e Fcorrec = 1 |
|---|---|---|---|
| CAMARGOS | 0,969607 | **0,969606** | 1,000000 |
| FURNAS | 0,991295 | **0,991296** | 1,000000 |
| ITUTINGA, FUNIL | 1,000000 | 1,00000 (± 5e-6) | 1,000000 |

O plano impresso já contém α. A geração encosta em **α × plano impresso = α² × FPHA**. Quando α = 1 a razão é 1, como esperado. O objetivo do `computed` fica **+3.125,65 R$** acima do `precomputed`. O revisor tinha medido +3.141,8 R$; a diferença de 16 R$ não foi investigada e não muda a conclusão.

**FP-05: configuração por usina ignorada.** Três versões do `CAD_FPHA_CONFIG` (sem a usina, 2 planos com janela, 1 plano) produzem **os mesmos 6 planos** em FURNAS e CAMARGOS e o **mesmo objetivo**, 4.502.332,7276. Os campos são lidos e não têm efeito.

**MX-06.** `FPHA="precomputed"` no exemplo aborta com "missing FPHA data for hydro plant 0".

**EM-04.** Com `a0 = NaN` no `CAD_TAILRACE` de ITUTINGA, o EngMax vira 0 nos 48 períodos e a usina não turbina (verte 23 m³/s). O único sinal é um aviso de "non-positive net head", que não aponta para o NaN.

**FCF (NU-R01, NU-R02).** Objetivo com FCF=0: 4.467.814,03. Com FCF=1: 4.497.639,84. A diferença, **29.825,81**, é exatamente o α calculado com (V − Vmin). Com V absoluto seria 29.863,92. No exemplo os cortes são idênticos, então o α é uma **constante**: soma ao objetivo e não muda nenhuma decisão.

**RD-01, RD-04, RD-07, MX-10, NU-11.** Os números estão na tabela 12.3. Todos vêm de execuções completas do binário.

### 12.5 O que **não** foi demonstrado e por quê

| Item | Situação | Consequência para o relatório |
|---|---|---|
| **NU-13** (relaxação com folgas disparada em `Unbounded`) | Os dois testes só verificam o **pré-requisito**, que o backend distingue `Unbounded` de `Infeasible`, e **passam**. O defeito, que [solve.rs:237-238](../../crates/labdessem-simulation/src/stages/solve.rs#L237-L238) trate os dois igual, não foi exercitado de ponta a ponta. | segue como achado de **leitura**, BAIXA |
| **NU-R03, parte da vtable** (o leitor não confere o tamanho da vtable FlatBuffers) | O teste falha na **pré-condição**: esperava vtables de 22 bytes, e o arquivo do exemplo tem 18. É um erro de construção do teste, provavelmente o ajuste que o testador fazia quando foi interrompido. **Não prova nada.** | essa parte segue como achado de **leitura**; o resto do NU-R03 está demonstrado |
| **TH-02** | O teste Rust do `output_prep` não foi escrito. | a prova é a execução (caso base e matriz) |
| **FP-02, FP-05** | Não há teste Rust. | a prova é a execução (12.4) |
| Achados só de documentação ou de escolha deliberada (TH-04, TH-05, HI-R04, NU-R04, NU-14, parte doc do MX-14) | não são testáveis por programa | a evidência é a leitura |
| Demais achados BAIXA sem teste dedicado (por exemplo NU-12, RD-12, RD-16, FP-06, FP-08) | não foram priorizados na fase de teste | a evidência é a leitura, a revisão adversarial e, quando se aplica, a matriz |

### 12.6 Como rodar os testes de prova você mesmo

Os testes estão em quatro patches que só **acrescentam** arquivos de teste, mais uma linha `mod audit_tests;` em `stages/mod.rs`. Eles não tocam os mesmos arquivos e aplicam limpo sobre o `3d72ec6`. Isso foi conferido com `git apply --check`. O passo a passo está no **Apêndice B**.

Para rodar só uma suíte:

```bash
cargo test -p labdessem-solver --test audit_termicas       # térmicas (FO, colunas, TH-03 no solver)
cargo test -p labdessem-io     --test audit_hidro          # leitores hidro/FPHA
cargo test -p labdessem-cli    --test audit_hidro_model    # modelo hidro
cargo test -p labdessem-model  --test audit_nucleo         # FCF
cargo test -p labdessem-simulation --lib audit_            # folgas de limites (NU-R05)

# rede ponta a ponta: usar o binário de release deixa a suíte em ~10 s
cargo build --release
LABDESSEM_BIN=$PWD/target/release/labdessem cargo test -p labdessem-cli --test audit_rede

# sondas que imprimem os números desta seção
LABDESSEM_BIN=$PWD/target/release/labdessem cargo test -p labdessem-cli --test audit_rede \
  -- --ignored --nocapture --test-threads=1
```

**Depois de cada correção**, o teste do defeito correspondente deve passar a **passar**, e o controle deve continuar passando. Dois testes precisam ser **trocados**, não só "passar":

- o `convert_single_plane` do FPHA, que hoje trava o FP-02;
- o `audit_rd_r01_control_lockfile_pins_good_lp_1_15_0`, que muda se a versão do `good_lp` mudar (decisão D1).

---

## 13. Decisões que dependem de você

Vários achados não têm uma correção "óbvia": a resposta certa depende de qual metodologia o projeto quer seguir. Esta seção junta essas decisões. Cada uma traz as opções, o que se ganha e o que se perde, e a recomendação do revisor, quando houve.

### D1. Como corrigir o sinal dos duais `≥` (RD-01)

| Opção | Ganho | Perda e risco |
|---|---|---|
| (a) Atualizar o good_lp para 1.15.3 e travar com `=1.15.3` | Usa a correção oficial. Todos os duais `≥` ficam certos de uma vez. | Muda o sinal de **todos** os duais `≥` (hoje só o `flow_cut_lower` é lido, então o impacto é só no CMB). Exige rodar a suíte inteira. |
| (b) Negar o dual das linhas `≥` em `highs.rs` e travar com `=1.15.0` | Mudança local. | Precisa lembrar de desfazer se um dia atualizar o good_lp. |

**Em qualquer caso**, acrescentar o teste unitário da sonda (min x, x ≥ 3 ⇒ dual = +1) e um teste de invariância do CMO à orientação De/Para das linhas.

### D2. Como corrigir o α duplicado da FPHA computed (FP-02, junto com FP-08)

| Opção | Ganho | Perda e risco |
|---|---|---|
| (a) `correction_factor = 1.0` na conversão | Uma linha. | O γS fica sem α, divergindo do Libs. |
| (b) **Tirar o `scale_planes` do pipeline e manter cf = α** | Igual ao Libs e ao formato do `OPER_FPHA`. O α passa a valer também para o γS. | Exige ajustar o cálculo de desvio do pipeline e o cabeçalho do CSV de saída. |

**Recomendação do revisor: (b).** Nos dois casos, o teste `convert_single_plane` precisa ser reescrito.

### D3. Semântica do UCH hidráulico (HI-05 / FP-03 e HI-04 / FP-04)

| Opção | Ganho | Perda e risco |
|---|---|---|
| (a) **Seguir o DESSEM ≥ 22.1**: Q da usina livre do comprometimento | É o modelo oficial atual. Não cria inviabilidade. | A saída por unidade continua incoerente. É preciso documentar e agregar Q por usina na saída. |
| (b) Seguir o Libs / DESSEM 22.0.2: q_u ≤ Qmax_u·on_u | Coerência física por unidade. | O DESSEM abandonou esse modelo porque ele pode ser inviável quando a água precisa passar pela turbina. |
| (c) (b) com variável de "vertimento pela turbina" | Coerente e viável. | Mais complexo. |

**Recomendação do revisor FPHA: (a), registrada em ADR.** Para produtividade constante (HI-04), decidir também se entra um acoplamento por unidade g_u ≤ ρ·q_u (simétrico ao modo FPHA) ou se Q e G por unidade deixam de ser impressos.

### D4. Contrato da FCF (NU-R02, NU-R01, NU-R03)

1. **Volume absoluto ou volume útil (V − Vmin)?** O código usa útil; a documentação e o produtor (cobre) usam absoluto.
2. **Unidade dos cortes:** R$ ou mil R$? O exemplo parece estar em mil R$.
3. **Associação corte ↔ usina:** manter por posição (documentando) ou passar a validar por código?

### D5. Base de demanda com rede (RD-04)

Com `rede=1`, a demanda do submercado deve ser:

- a soma das cargas de barra (como no DESSEM, MM eq. 7.3);
- ou o valor de `OPER_SBM`, com validação de que as somas batem?

Em qualquer caso, o dado do exemplo precisa ser corrigido.

### D6. Déficit (RD-05 / MX-05)

| Opção | Comentário |
|---|---|
| (a) Manter sem déficit e melhorar o diagnóstico de inviabilidade | Coerente com o DESSEM com rede. |
| (b) Déficit só com `rede=0` | |
| (c) Déficit sempre, com custo alto | Garante que todo caso produz resultado. Em contrapartida, mascara erros de dado. |

### D7. Par de submercados não declarado (RD-07)

| Opção | Comentário |
|---|---|
| Ilimitado e gratuito (hoje) | Precisa ser documentado. |
| Proibido (limite 0) | |
| Erro de leitura | |

### D8. Desvio de água (HI-07)

Entre as duas opções, ou as duas juntas:

- coluna de capacidade de desvio no `CAD_UHE`, como o documento promete;
- penalidade de desvio igual à do vertimento.

### D9. Volume usado na FPHA (FP-06)

Fim do período (como hoje, igual ao MM 9.33) ou médio (Libs, e o que o EngMax já usa pelo ADR-006)? O revisor recomenda o **médio** e pede ADR.

### D10. Custo residual de desligamento (TH-05)

Seguir a eq. 10.22 do DESSEM literalmente (só unidades acionadas no horizonte com Ton não cumprido) ou manter a escolha atual (commit `cd9e6c1`), economicamente coerente, e documentá-la?

### D11. Modo 1 com rede congestionada (MX-10)

Aceitar como limitação documentada (e recomendar o modo 2 com rede apertada) ou fazer o MILP ser re-resolvido com os cortes que ele viola?

### D12. Validação de afluências (HI-01)

Além de rejeitar código desconhecido, conferir também o `Nome` entre `OPER_VAZAO` e `CAD_UHE`?

### D13. `CAD_FPHA_CONFIG.csv` (FP-05)

Implementar os campos (número de planos, janela de volume, tolerância) ou retirar o arquivo da documentação?

---

## 14. O que foi verificado e está **correto**

Um relatório de auditoria tende a dar a impressão de que "tudo está quebrado". Não é o caso. Abaixo está o que mais de um agente conferiu, por leitura **e** por execução, e encontrou certo. Isso delimita onde os problemas **não** estão.

### 14.1 Fluxo leitura → montagem → solver → saída

| Item | Como foi verificado |
|---|---|
| O objetivo impresso corresponde exatamente ao despacho impresso | Recomposição independente nas 224 execuções: erro ≤ 2,2e-4 R$, inclusive com TON residual, FCF e folga de rede. |
| Os fluxos impressos correspondem à geração e às cargas impressas | PTDF própria em Python, 112 execuções: erro ≤ 5e-5 MW. |
| Balanço de energia por submercado e período | 224 execuções: erro ≤ 5e-7 MW. |
| Balanço hídrico com topologia de cascata e desvio | 224 execuções, erro ≤ 1e-6 hm³. Também com período de 1,5 h, recalculado à mão. O termo de **bombeamento** entra no check, mas o exemplo não tem elevatórias, então ele sempre valeu zero: o bombeamento foi verificado **só por leitura de código**. |
| Conversão m³/s ↔ hm³ em todas as colunas | 224 execuções |
| Custos CVU × g × Δt, penalidades de vertimento e turbinamento, intercâmbio mínimo | parcelas do O01 |
| Partida, parada e trajetórias térmicas, tempos mínimos, condição inicial (exceto TH-03 e TH-09) | check T02, 96 execuções |
| Envelope FPHA e acoplamento por unidade no modo FPHA | checks H10/H10b, 96 execuções |
| Σq ≤ EngMax | check H11, 224 execuções |
| Cálculo de α da FCF (com a convenção V − Vmin que o código usa) | check F01 e conta do revisor do núcleo |
| Custo residual de TON (quando não colide com o custo de partida) | experimento `tonres_cmo0`: +12.000,00 R$ exatos |

### 14.2 Indexação e leitura da solução

| Item | Quem conferiu |
|---|---|
| Offsets de **colunas** usados pelo acessor (geração, turbinamento, volume etc.) | revisores do núcleo e hidro, mais a matriz (as saídas fecham) |
| Offsets de **linhas** `demand_balance` e `hydro_balance`, os únicos consumidos | revisores do núcleo, hidro e rede; checks M01 e H17 |
| Ordem `hydro_plant_entries` = ordem de `system.hydro_plants`, então o PiBalHidr vai para a usina certa | revisor hidro |
| Mapeamento dos duais dos cortes de fluxo (os cortes são as últimas linhas da matriz) | revisor de rede |
| `first_slack_column` para as folgas | revisor do núcleo |
| `apply_commitment_fixes`, que fixa o comprometimento do MILP no LP-FIXED e no LP-CALC-CMO | revisor do núcleo |
| Variáveis de UC no LP-CALC-CMO são contínuas e fixadas, de modo que os duais não são zerados pelo caminho MILP | revisor de rede |

### 14.3 Preços

| Item | Observação |
|---|---|
| CMO sem rede = dual do balanço de demanda ÷ Δt | sinal e escala corretos (check M01) |
| Valor da água (PiBalHidr) | dual de uma igualdade, com sinal coerente: positivo quando a água extra é vertida com penalidade, e igual ao coeficiente do corte ativo com FCF |
| CMO negativo de −0,09712 com UCT=1 | **legítimo**: a trajetória força 1120 MW contra demanda de 1079,88 MW, e Fic1 exporta a sobra |
| CMB com congestionamento em linha no limite **superior** | correto. Só o limite inferior está invertido (RD-01). |

### 14.4 Configuração e modos

| Item | Observação |
|---|---|
| `opcao_execucao` inválido (0 ou 3), `FPHA` desconhecido, tipos errados (string, booleano, número com ponto, negativo) | rejeitados com mensagem |
| Terminação do laço LP-FIXED | garantida: no máximo |linhas| × T voltas |
| LP do modo 1 sem UC térmico | correto e igual ao DESSEM (Tab. 27.2) |
| Partida e parada binárias | escolha segura (NU-14 é só documentação) |
| Monotonicidade entre configurações vizinhas | nenhuma violação em 752 pares |
| Suíte de testes existente | `cargo test` passa em todos os crates exercitados pelos revisores |

---

## 15. Uma ordem sugerida para atacar os problemas

Esta é **só uma sugestão**, para quando você decidir corrigir. Nada foi implementado. O critério foi: (1) impacto em resultado principal e silencioso primeiro, (2) correções pequenas que destravam outras, (3) o que exige decisão sua por último.

| Ordem | O quê | Por quê | Tamanho estimado |
|---|---|---|---|
| 1 | **HI-01**: validar códigos no leitor de afluências e corrigir `OPER_VAZAO` do exemplo | Corrige o dado de referência e protege decks futuros. Sem isso, toda comparação de despacho hidráulico fica contaminada. | pequeno |
| 2 | **TH-01 / NU-07 / RD-R02**: `+=` na conversão, unicidade de nomes, termo com nome inexistente vira erro | Fecha uma classe inteira de erros silenciosos na conversão. | pequeno |
| 3 | **RD-01 + RD-R01** (depois da decisão D1), com teste de sinal e de invariância De/Para | Preço errado em rede congestionada. | pequeno no código; o cuidado é com a versão |
| 4 | **RD-02, RD-03, RD-06**: escala, período e swing do cmobar | Mesmo arquivo de saída, mesma revisão. | pequeno |
| 5 | **HI-03**: revalidar o `System` depois de injetar a FPHA computed | Uma linha que evita energia gratuita. | pequeno |
| 6 | **FP-02** (depois da D2), trocando o teste que trava o erro | Envelope FPHA errado. | médio |
| 7 | **TH-03**: fixar w[r] = 1 na rampa inicial de desligamento | Geração forçada espúria. | médio |
| 8 | **NU-11**: validação do `study_config.json` | Evita que um erro de digitação mude o modelo. | pequeno |
| 9 | **RD-04** (depois da D5) e o dado do exemplo | Duas bases de demanda. | pequeno a médio |
| 10 | **NU-R02 / NU-R01** (depois da D4) e regenerar o `OPER_FCF.bin` do exemplo com cortes reais | Hoje a FCF é inerte no exemplo. Com uma FCF real, o referencial muda decisões. | médio |
| 11 | **TH-02, HI-02**: StatusOn | Saídas secundárias. | pequeno |
| 12 | Itens MÉDIA restantes e decisões D3, D6–D13 | | variável |
| 13 | **NU-10**: derivar offsets do modelo construído | Defeito latente. Melhor resolver antes que alguém precise de um dual novo. | médio |
| 14 | Cobertura: casos de exemplo para `precomputed`, rede congestionada, TON efetivo, FPHA com EngMax | Os testes de prova desta auditoria já são um começo. | médio |

---

## Apêndice A — Equivalência de IDs

Cada agente numerou seus achados de forma independente. Esta tabela liga os sinônimos.

| Defeito | Leitor | Matriz | Núcleo | Outro |
|---|---|---|---|---|
| Sinal do dual `≥` (CMB/CMO) | RD-01 | MX-01 | NU-02 | RD-R01 (versão) |
| CMB sem Δt, período 0-based, sem swing | RD-02, RD-03, RD-06 | MX-02 | NU-05 | |
| Afluência por código | HI-01 | MX-03 | NU-03 | HI-R03 (mesma família) |
| Demanda SBM × barras | RD-04 | MX-04 | | |
| Sem déficit | RD-05 | MX-05 | | EM-01 (consequência) |
| `precomputed` sem dados no exemplo | | MX-06 | | |
| StatusOn hidráulico | HI-02 | MX-07 | NU-06 | |
| StatusOn térmico | TH-02 | MX-08 | NU-06 | |
| Produtividade por usina | HI-04 | MX-09 | NU-15 | FP-04 |
| Modo 1 e rede | | MX-10 | NU-R06 | |
| Validação de config | | MX-11 | NU-11 | |
| EngMax entre etapas | | MX-12 | | EM-02 |
| Violacao sem tolerância | RD-10 | MX-13 | | |
| FCF (doc e sinal) | | MX-14 | NU-R02, NU-R04 | |
| Offsets de linhas | TH-10, HI-09, FP-10, RD-13 | MX-15 | NU-10 | |
| TON sem efeito no exemplo | | MX-16 | | |
| Mensagem "folgas de rede" | | MX-17 | | |
| Partida apagada pelo residual | TH-01 | | NU-01 | |
| FPHA computed sem planos | HI-03 | | NU-04 | FP-01 |
| Nomes duplicados | TH-R01, HI-08 | | NU-07 | NU-R05 (folgas) |
| Backend: faixa e linha livre | RD-14 | | NU-08 | |
| UCH e turbinamento de unidade desligada | HI-05 | | | FP-03 |
| Barra da 1ª unidade | TH-07, HI-06 | | | RD-15 |

---

## Apêndice B — Onde está cada coisa

Tudo o que a auditoria produziu fica **fora** do repositório, em `~/.cache/labdessem-audit/`:

| Caminho | Conteúdo |
|---|---|
| `findings/termicas.md`, `hidro.md`, `fpha_engmax.md`, `rede.md`, `nucleo.md` | relatórios dos 5 leitores (fase 1) |
| `findings/review_*.md` | relatórios dos 5 revisores (fase 3), com receitas de reprodução |
| `findings/matriz.md` | relatório da matriz de 384 configurações (fase 2) |
| `findings/testes_termicas.patch`, `testes_hidro_fpha.patch`, `testes_rede.patch`, `testes_nucleo.patch` | os 173 testes de prova (fase 4), prontos para `git apply` |
| `test_logs/*.log` | saída completa de cada suíte de teste na rodada final, que é a fonte das mensagens citadas na seção 12 |
| `run_audit_tests.sh`, `tools/parse_tests.py` | script que roda as 9 suítes em sequência e extrator de veredito por teste |
| `testerA/`, `testerB/`, `testerC/`, `base/` | clones isolados do repositório onde os testes foram escritos |
| `testerA_runs/`, `testerB_runs/`, `testerC_runs/`, `testerD_runs/`, `review_nu/` | casos modificados e saídas das execuções E2E |
| `target-*` | diretórios de build do cargo, cerca de 6 GB cada (26 GB no total) |
| `matrix_scripts/` | verificador independente `check_outputs.py` (40 checks), `aggregate.py` e `run_extra.py`, recuperados das transcrições |
| `report/` | as partes deste relatório |

**Parte do material das fases 1 a 3 se perdeu.** Ele ficava no `/tmp` (tmpfs), que foi apagado quando a sessão caiu no meio da auditoria.

- **Relatórios:** recuperados integralmente das transcrições dos agentes.
- **Verificador da matriz:** também recuperado. Para conferir qualquer execução:

  ```bash
  python3 -I ~/.cache/labdessem-audit/matrix_scripts/check_outputs.py <pasta_do_caso_com_RESULTADOS>
  ```

  Rodado sobre uma execução nova do caso base, ele reproduz os mesmos FAIL da matriz: H04, H09b, H12, T05, M04, M05 e M06. Pode faltar nele uma edição tardia, que não foi encontrada na transcrição.
- **Perdidos:** `run_matrix.py`, `summarize_runs.py`, os scripts de diagnóstico do CMB e os casos modificados dos revisores de térmicas, hidro, FPHA e rede. As **receitas** de reprodução estão nos relatórios, e os testes de prova da fase 4 foram refeitos em disco durável.

### Como aplicar os testes de prova no seu repositório (opcional)

Os quatro patches só **criam** arquivos de teste. A única exceção é o `testes_nucleo.patch`, que acrescenta duas linhas (`#[cfg(test)] mod audit_tests;`) em `crates/labdessem-simulation/src/stages/mod.rs`. Eles não tocam os mesmos arquivos, e cada um foi conferido com `git apply --check` sobre o `3d72ec6`.

```bash
cd /home/carlosribeiro/git/labdessem_rust
git switch -c audit/provas            # num branch separado
git apply ~/.cache/labdessem-audit/findings/testes_termicas.patch
git apply ~/.cache/labdessem-audit/findings/testes_hidro_fpha.patch
git apply ~/.cache/labdessem-audit/findings/testes_rede.patch
git apply ~/.cache/labdessem-audit/findings/testes_nucleo.patch

cargo build --release                 # os testes E2E da rede ficam muito mais rápidos com o binário de release
LABDESSEM_BIN=$PWD/target/release/labdessem cargo test --workspace --no-fail-fast audit_
# esperado hoje: 87 testes de defeito FALHAM, 75 controles PASSAM (seção 12.2)
```

Os testes E2E da rede gravam casos temporários em `target/tmp/audit_runs`. Para outro lugar, defina `AUDIT_RUNS_DIR`.

### Limpeza

Quando terminar a revisão, você pode liberar cerca de 26 GB com:

```bash
rm -rf ~/.cache/labdessem-audit/target-*
```

Isso mantém os relatórios. Para apagar tudo, use `rm -rf ~/.cache/labdessem-audit`.
