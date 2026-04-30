# Mapa de Conexoes do Codigo

## Objetivo

Este material foi escrito para servir como um mapa detalhado de como o codigo do `labdessem_rust` se conecta internamente. A ideia nao e apenas listar arquivos, mas mostrar:

- onde cada conceito nasce;
- como ele atravessa as camadas do sistema;
- em que momento vira variavel, restricao, custo ou relatorio;
- quais arquivos precisam ser alterados quando voce quer modificar uma parte especifica do modelo.

Este documento complementa o `manual_desenvolvimento.md`. O manual explica como implementar mudancas. Este mapa explica como o sistema esta costurado hoje.

## Visao geral

O projeto esta organizado em crates com responsabilidades bem separadas:

- `crates/labdessem-cli`
  ponto de entrada do executavel;
- `crates/labdessem-io`
  leitura dos arquivos de entrada e construcao do estudo;
- `crates/labdessem-core`
  definicao das estruturas de dominio e validacao;
- `crates/labdessem-model`
  montagem matematica do problema: indices, variaveis, restricoes e funcao objetivo;
- `crates/labdessem-solver`
  traducao do modelo para o solver e coleta dos resultados;
- `crates/labdessem-simulation`
  orquestracao das etapas de solucao, iteracoes, rede, cortes e arquivos de saida.

O fluxo principal e este:

```text
main.rs
  -> read_study_from_config(...)
      -> leitura dos CSVs
      -> construcao do System
  -> run_simulation(...)
      -> cria Model
      -> resolve com Solver
      -> faz iteracoes, LP/MILP/LP-FIXED/LP-CALC-CMO quando aplicavel
  -> write_results_csvs(...)
      -> gera os arquivos de saida
```

## Fluxo ponta a ponta

### 1. Entrada pelo CLI

Arquivo principal:

- [main.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-cli/src/main.rs)

Papeis principais:

- localizar e ler o `study_config.json`;
- chamar a leitura do estudo;
- decidir a estrategia de execucao;
- chamar a simulacao;
- gravar os resultados em CSV.

Fluxo logico:

```text
main
  -> StudyConfig::from_json / leitura de configuracao
  -> read_study_from_config(...)
  -> run_simulation(system, network_enabled, execution_option)
  -> write_results_csvs(...)
```

Aqui o sistema ainda nao conhece variaveis nem restricoes. O que existe e apenas:

- configuracao do caso;
- caminhos dos arquivos;
- escolhas de execucao;
- `System` carregado em memoria.

### 2. Leitura dos dados e montagem do estudo

Arquivo principal:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)

Papeis principais:

- ler arquivos `CAD_*` e `OPER_*`;
- interpretar colunas, unidades e flags;
- construir objetos de dominio;
- devolver um `System`.

Esse arquivo e o ponto em que o mundo externo vira estrutura interna.

Ele faz, entre outras coisas:

- le usinas termicas;
- le usinas hidraulicas;
- le renovaveis;
- le elevatorias;
- le demandas, vazoes, limites, FPHA, custos residuais etc.;
- monta mapas temporais por periodo;
- faz conversoes de unidade quando necessario.

Em termos de arquitetura, `study.rs` e a ponte entre:

```text
CSV / JSON de entrada
    ->
estruturas de dominio em `labdessem-core`
```

Se algum conceito novo entra por arquivo, normalmente ele aparece primeiro aqui.

## Camada de dominio

### 3. Estruturas centrais do modelo

Arquivos importantes:

- [system.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/system.rs)
- [hydro.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/hydro.rs)
- [thermal.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/thermal.rs)
- [renewable.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/renewable.rs)
- [error.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/error.rs)
- [ids.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/ids.rs)

#### `System`

O `System` e a fotografia completa do caso em memoria. Ele agrega:

- submercados;
- barras;
- linhas;
- usinas termicas;
- usinas hidraulicas;
- renovaveis;
- elevatorias;
- series operativas por periodo;
- limites;
- parametros de execucao.

Tudo o que o modelo precisa sai daqui.

#### Estruturas de dominio especificas

Cada arquivo do `core` representa um pedaço do sistema:

- `thermal.rs`: unidades termicas, rampas, ton/toff, condicao inicial;
- `hydro.rs`: usinas e conjuntos hidraulicos, reservatorio, FPHA, vazoes, desvios, bombeamento, penalidades;
- `renewable.rs`: renovaveis e geracao programada;
- `ids.rs`: tipos fortes para ids, evitando mistura acidental entre entidades;
- `error.rs`: erros de consistencia e validacao.

O `core` nao monta restricao nem resolve nada. Ele so define o significado dos dados.

## Camada matematica

### 4. Entrada no modelo

Arquivo principal:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/lib.rs)

Papel:

- receber o `System`;
- escolher o `SolveMode`;
- montar `Indexing`, `Variables`, `ConstraintSet` e `Objective`.

Fluxo interno:

```text
Model::from_system(system, solve_mode)
  -> Indexing::for_system(...)
  -> Variables::for_system(...)
  -> ConstraintSet::for_system(...)
  -> Objective::for_system(...)
```

Esse e o ponto onde o caso deixa de ser apenas dados de entrada e passa a ser um problema de otimizacao.

### 5. Indexacao

Arquivo:

- [indexing.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/indexing.rs)

Papel:

- transformar o `System` em listas de combinacoes que o modelo vai percorrer.

Exemplos:

- pares `(termica, unidade, periodo)`;
- pares `(hidraulica, conjunto, unidade, periodo)`;
- pares `(usina hidraulica, periodo)` para volume e vertimento;
- pares `(renovavel, periodo)`;
- pares `(submercado origem, submercado destino, periodo)`.

O `Indexing` e fundamental porque:

- organiza a dimensao das variaveis;
- evita recalcular cruzamentos toda hora;
- define o espaco de iteracao usado por variaveis, restricoes e relatorios.

Uma boa regra mental e:

```text
System diz "o que existe"
Indexing diz "em quais combinacoes o modelo vai trabalhar"
```

### 6. Variaveis de decisao

Arquivo:

- [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs)

Papel:

- declarar as familias de variaveis do problema.

As principais familias hoje sao:

- termicas
  - `thermal_generation`
  - `thermal_commitment`
  - `thermal_startup`
  - `thermal_shutdown`
- hidraulicas
  - `hydro_generation`
  - `hydro_turbining`
  - `hydro_spillage`
  - `hydro_diversion`
  - `hydro_volume`
  - `hydro_commitment`
  - `hydro_startup`
  - `hydro_shutdown`
- renovaveis
  - `renewable_generation`
- sistema
  - `deficit`
  - `interchange`
- flexibilidade e inviabilidades
  - `network_flow_slack`
  - `operational_limit_slack`
- elevatorias
  - `pumping`

Cada familia define:

- dominio da variavel;
- limites inferiores e superiores basicos;
- indexacao usada.

O arquivo `variables.rs` nao decide custo nem restricao. Ele apenas diz:

```text
"essas variaveis existem no modelo"
```

### 7. Restricoes

Arquivo:

- [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs)

Papel:

- transformar a formulacao matematica em restricoes lineares.

As restricoes sao agrupadas por tema. Entre as mais importantes:

- atendimento a demanda;
- balanco hidrico;
- limites de geracao renovavel;
- limites de geracao termica;
- limites de turbinamento;
- rampas e trajetorias termicas;
- ton/toff;
- commitment hidraulico;
- FPHA;
- intercambios;
- limites operacionais genericos;
- rede e folgas associadas, quando aplicavel.

Uma forma util de enxergar `constraints.rs` e:

```text
variables.rs diz quais pecas existem
constraints.rs diz como essas pecas podem se combinar
```

### 8. Funcao objetivo

Arquivo:

- [objective.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/objective.rs)

Papel:

- montar o custo total minimizado pelo problema.

A funcao objetivo atual reune diferentes parcelas, por exemplo:

- CVU termico;
- custos de partida e desligamento;
- deficit;
- penalidade de vertimento;
- penalidade de turbinamento;
- custos residuais de TON, quando habilitados;
- penalidades de folgas e inviabilidades.

Em termos de conexao arquitetural:

- os coeficientes economicos vem do `System`;
- as variaveis vem de `Variables`;
- a soma linear final e montada aqui.

## Camada de solver

### 9. Traducao para o solver

Arquivo:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-solver/src/lib.rs)

Papel:

- pegar o `Model` abstrato;
- criar o modelo concreto do `good_lp`;
- chamar o solver;
- coletar valores primais e duais.

Fluxo simplificado:

```text
Model
  -> construir variaveis do solver
  -> adicionar restricoes
  -> adicionar objetivo
  -> resolver
  -> coletar resultados
```

Saida principal:

- `SolveSummary`
  - valor da funcao objetivo;
  - valores das variaveis;
  - duais das restricoes, quando disponiveis.

Esse crate nao conhece significado de negocio. Ele sabe apenas traduzir o modelo para o solver.

## Camada de simulacao

### 10. Orquestracao da execucao

Arquivo:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

Esse e o arquivo mais orquestrador do sistema.

Ele conecta:

- leitura pronta do `System`;
- montagem do `Model`;
- estrategia de execucao;
- rede e cortes;
- re-solucao por etapa;
- geracao de relatorios.

As entradas principais dele sao:

- `System`
- `network_enabled`
- `execution_option`

As estrategias de execucao hoje sao:

- opcao 1
  - `LP -> MILP -> LP-FIXED -> LP-CALC-CMO`
- opcao 2
  - `MILP unico`

O `simulation` e onde se decide:

- quando rodar LP;
- quando rodar MILP;
- quando fixar inteiras;
- quando calcular CMO;
- quando gerar duais;
- quando avaliar cortes de rede;
- como escrever os CSVs finais.

## Como um conceito atravessa o sistema

Esta secao mostra o caminho completo de alguns conceitos importantes.

### 11. Exemplo 1: renovavel

#### Nascimento

Arquivos:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)
- [renewable.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/renewable.rs)

O arquivo de entrada define a usina renovavel e a geracao programada por periodo.

#### Dominio

O `core` guarda:

- identidade;
- submercado;
- barra, se aplicavel;
- disponibilidade programada.

#### Indexacao

Em [indexing.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/indexing.rs), a renovavel entra como pares:

```text
(renewable_plant, period)
```

#### Variavel

Em [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs):

```text
renewable_generation
```

#### Restricao

Em [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs), a regra atual e:

```text
0 <= geracao renovavel <= geracao programada
```

#### Balanco de demanda

Tambem em `constraints.rs`, a geracao renovavel entra no atendimento da demanda do submercado.

#### Saida

Em [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs), a familia aparece no:

- `resultado_renovaveis.csv`

Esse e um bom exemplo de conceito completo:

```text
CSV -> System -> Indexing -> Variable -> Constraint -> Solve -> CSV de saida
```

### 12. Exemplo 2: volume hidraulico

#### Nascimento

Arquivos:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)
- [hydro.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/hydro.rs)

O volume entra a partir de:

- volume inicial;
- volume minimo;
- volume maximo;
- informacoes operativas por periodo.

#### Variavel

Em [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs):

```text
hydro_volume
```

Essa variavel e por usina e por periodo.

#### Restricoes conectadas

Em [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs), o volume participa de:

- balanco hidrico;
- limites de volume;
- FPHA;
- limites operacionais genericos quando a variavel e `VOL`.

#### Saida

Em [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs), o volume aparece em:

- `VolumeHM3`
- `VolumeUtilHM3`
- `VolumeUtilMaxHM3`
- `VolumeUtilPct`

Esse e um exemplo importante porque mostra que uma mesma variavel pode:

- influenciar restricoes fisicas;
- influenciar restricoes de producao;
- alimentar relatorios derivados.

### 13. Exemplo 3: termica com commitment

#### Dominio

Arquivos:

- [thermal.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/thermal.rs)
- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)

Aqui entram:

- `pmin`, `pmax`;
- `ton`, `toff`;
- estado inicial;
- `tinic`;
- `time_in_ramp`;
- trajetorias de acionamento e desligamento.

#### Variaveis

Em [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs):

- `thermal_generation`
- `thermal_commitment`
- `thermal_startup`
- `thermal_shutdown`

#### Restricoes

Em [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs):

- ligacao entre estado e geracao;
- rampas e trajetorias;
- ton/toff;
- transicoes `u`, `y`, `w`;
- condicoes iniciais.

#### Objetivo

Em [objective.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/objective.rs):

- CVU;
- custo de partida;
- custo de desligamento;
- custo residual de permanencia ligada fora do horizonte, quando habilitado.

#### Saida

Em [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs):

- `resultado_termicas.csv`

Esse exemplo mostra um conceito que toca todas as camadas:

```text
entrada
-> dominio
-> variaveis binarias e continuas
-> restricoes de logica
-> custo
-> relatorio
```

### 14. Exemplo 4: FPHA

#### Dominio

Arquivo:

- [hydro.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/hydro.rs)

Os cortes da FPHA pertencem ao dominio hidraulico da usina.

#### Leitura

Arquivo:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)

Aqui sao carregados:

- segmentos;
- fatores de correcao;
- coeficientes;
- lados direitos.

#### Restricoes

Arquivo:

- [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs)

A FPHA entra:

- como cortes superiores de geracao agregada por usina;
- como acoplamento auxiliar entre geracao e turbinamento.

#### Saida derivada

Arquivo:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

Usa a FPHA para calcular:

- `GeracaoMaxPontoMW`

Esse e um ponto importante da arquitetura:

```text
nem tudo que usa a FPHA e restricao do solver;
parte da FPHA tambem e usada para calculos de relatorio
```

### 15. Exemplo 5: limite operacional generico

#### Leitura

Arquivo:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)

Aqui entram os registros de `OPER_REST_LIM`.

#### Dominio

Os limites sao armazenados dentro do `System` com:

- variavel alvo;
- escopo;
- limite inferior;
- limite superior;
- periodos;
- entidade associada.

#### Restricao

Arquivo:

- [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs)

O codigo transforma o registro lido em combinacoes lineares sobre variaveis como:

- `VOL`
- `GER`
- `TURB`
- `VERT`
- `DEFLU`
- `QDES`
- `QBOM`

#### Folga

Quando permitido, a inviabilidade vai para:

- `operational_limit_slack`

#### Saida

Arquivo:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

O relatorio de inviabilidade imprime:

- variavel;
- unidade;
- violacao inferior;
- violacao superior.

E um excelente exemplo de cadeia completa:

```text
registro generico de entrada
-> interpretacao por tipo
-> restricao linear
-> slack
-> relatorio de violacao
```

## Rede e submercados

### 16. Onde a rede entra

Arquivo principal:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

Hoje a rede esta muito mais presente na simulacao do que no `core` abstrato do modelo.

Ela influencia:

- montagem de fluxo por barra;
- cortes e verificacoes de violacao;
- iteracao ate nao haver novas violacoes relevantes;
- calculo final do LP-CALC-CMO.

Quando a rede esta desativada:

- o sistema nao precisa se preocupar com restricoes eletricas;
- a logica pode operar diretamente por submercado.

Quando a rede esta ativada:

- a barra volta a ser importante;
- o despacho precisa respeitar os fluxos;
- o LP-CALC-CMO so e executado apos estabilizacao sem novas violacoes.

## Duais e CMO

### 17. Onde surgem os duais

Arquivo do solver:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-solver/src/lib.rs)

O solver devolve os duais das restricoes lineares quando a etapa resolvida permite isso.

### 18. Como isso chega na saida

Arquivo:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

Hoje os duais sao usados principalmente em:

- `resultado_cmosist.csv`
  - `PiDemanda`
- `resultado_hidreletricas.csv`
  - `PiBalHidr`

Isso so fica consistente porque existe a etapa:

- `LP-CALC-CMO`

Nela, as inteiras ja estao fixadas, o problema e linear e o solver pode entregar duais economicamente interpretaveis.

## Arquivos que costumam andar juntos

### 19. Se voce mexer em leitura de dados

Arquivos mais provaveis:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)
- [hydro.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/hydro.rs)
- [thermal.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/thermal.rs)
- [renewable.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/renewable.rs)
- [system.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/system.rs)

### 20. Se voce criar uma nova variavel

Arquivos mais provaveis:

- [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs)
- [indexing.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/indexing.rs)
- [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs)
- [objective.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/objective.rs), se tiver custo
- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs), se for imprimir na saida

### 21. Se voce criar uma nova restricao

Arquivos mais provaveis:

- [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs)
- [indexing.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/indexing.rs), se precisar de nova malha de iteracao
- [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs), se a restricao depender de nova variavel

### 22. Se voce criar nova saida

Arquivo mais provavel:

- [lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

Se precisar de valor derivado:

- pode envolver tambem `constraints.rs`, `objective.rs` ou os dados do `System`.

## Mapa mental resumido

Uma forma pratica de lembrar a arquitetura e esta:

### `labdessem-cli`

- inicia o programa;
- le a configuracao;
- dispara a simulacao.

### `labdessem-io`

- le os arquivos;
- interpreta as colunas;
- constroi o `System`.

### `labdessem-core`

- define o significado dos dados;
- valida consistencia;
- representa o dominio.

### `labdessem-model`

- traduz o dominio para matematica:
  - indices;
  - variaveis;
  - restricoes;
  - objetivo.

### `labdessem-solver`

- transforma o modelo abstrato em problema resolvido pelo solver;
- devolve primais e duais.

### `labdessem-simulation`

- decide como resolver;
- executa as etapas;
- gerencia rede e iteracoes;
- grava as saidas.

## Conclusao

Se voce quiser entender rapidamente onde algo vive, esta regra costuma funcionar bem:

- se for estrutura ou dado do mundo real, procure no `core`;
- se vier de CSV, procure primeiro no `io`;
- se virar variavel, restricao ou custo, procure no `model`;
- se envolver ordem de resolucao, rede, relatorio ou exportacao, procure no `simulation`;
- se envolver o solver em si, procure no `solver`;
- se envolver apenas o fluxo de programa e configuracao inicial, procure no `cli`.

Se quiser continuar esse material, os proximos aprofundamentos naturais seriam:

- um diagrama so para termicas;
- um diagrama so para hidraulicas;
- um diagrama so para rede e LP-CALC-CMO;
- um mapa "arquivo por arquivo" de `constraints.rs`.
